// Ciclo de vida da sessao, 100% controlado pelo Runner - o Apollo NAO tem
// mais prep-cmd nenhum (nem "do" nem "undo"), decisao explicita pra
// deixa-lo mais simples ("plug and play": so' precisa saber conectar e
// rodar o "cmd") e dar ao Deck controle total sobre a tela do host. Quem
// liga a tela no lancamento (display_commands) e desliga no fechamento
// (restore_commands) e' sempre este modulo, rodando os comandos direto
// via shell.
//
// runner.py (Deck) registra a sessao ao configurar o Apollo (app_id +
// credenciais, EM MEMORIA, nunca gravadas em disco no host, mais os
// comandos de tela pre-calculados a partir do perfil) - o registro em si
// ja' roda os display_commands de forma SINCRONA (so' responde depois),
// entao o Runner deixou de ser opcional: sem ele, a tela nunca troca.
//
// Depois de registrada, um watchdog em background (mesma runtime tokio
// do servidor HTTP, ver lib.rs) fica de olho no processo via sysinfo
// (server.rs: is_app_id_running) e, quando detecta que o jogo fechou
// sozinho, restaura a tela e avisa o Apollo pra derrubar a conexao - sem
// precisar do Deck pedir nada. Cobre os dois fluxos de lancamento (botao
// antigo e atalho novo por jogo), ja que os dois passam pelo mesmo
// runner.py.
//
// "Fechar conexao" manual (QuickAccessContent.tsx -> POST /session/close)
// mata o jogo (se ainda estiver vivo, com SIGTERM + espera adaptativa +
// SIGKILL se preciso) e restaura a tela - diferente do watchdog, que so'
// age depois de confirmar que o processo ja morreu sozinho.
//
// ORDEM importa pra experiencia do usuario: em AMBOS os fluxos, o Apollo
// e' avisado (o que derruba o stream/desconecta o Moonlight no Deck,
// tirando a tela de streaming) ANTES de matar o jogo/restaurar a tela -
// nao depois. Kill do jogo (que pode levar ate' 20s de periodo de graca
// no fechamento manual) e os comandos de restauracao rodam DEPOIS, em
// background (tokio::spawn, sem bloquear a resposta) - o usuario ve o
// Deck sair da tela de streaming na hora, o resto acontece independente
// no host, sem ele esperando.

use axum::extract::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::apollo;
use crate::server::{is_app_id_running, timestamp, EventNotifier, RunnerEvent};

#[derive(Clone)]
pub struct ActiveSession {
    pub app_id: String,
    pub username: String,
    pub password: String,
    // Comandos de restauracao de tela (kscreen-doctor + fechar Big
    // Picture), pre-calculados por runner.py a partir do perfil - o
    // Runner so' os executa, sem interpretar o significado de cada um
    // (ver build_restore_commands em moonprofile_core.py).
    pub restore_commands: Vec<String>,
    // Bug real encontrado no device: a primeira checagem do watchdog pode
    // acontecer ANTES do jogo terminar de abrir (Steam demora um tempo
    // variavel pra spawnar o processo "reaper ... AppId=<id>" - Proton,
    // shader cache, etc) - sem esse campo, "ainda nao apareceu" e "ja
    // fechou" ficam indistinguiveis, e o watchdog fechava um jogo que
    // estava so' CARREGANDO. So' comeca a considerar "fechou" depois de
    // ter visto o processo rodando de verdade pelo menos uma vez.
    pub confirmed_running: bool,
}

pub type SessionState = Arc<Mutex<Option<ActiveSession>>>;

// Newtype (nao so' String) pra poder ser distinguido de outras Extension<String>
// no mesmo Router - axum identifica extensions pelo TYPE, nao por nome.
#[derive(Clone)]
pub struct ApolloBaseUrl(pub String);

#[derive(Deserialize)]
pub struct RegisterSessionRequest {
    app_id: String,
    username: String,
    password: String,
    // Comandos de LIGAR a tela (kscreen-doctor: enable/mode/hdr/disable
    // dos outros outputs) - rodados AGORA MESMO, antes de responder (ver
    // register_session), pra garantir que a tela ja esta' no estado certo
    // quando o runner.py (Deck) prosseguir pro exec do Moonlight.
    #[serde(default)]
    display_commands: Vec<String>,
    #[serde(default)]
    restore_commands: Vec<String>,
}

#[derive(Serialize)]
pub struct CloseResponse {
    ok: bool,
    error: Option<String>,
}

// Roda um comando de shell (string unica, mesmo formato que o Apollo
// usava no prep-cmd) - best-effort: uma falha aqui so' e' logada, nao
// interrompe os comandos seguintes (ex: kscreen-doctor tentando desligar
// um output que ja esta' desligado, nao e' fatal).
async fn run_shell_command(cmd: &str) {
    match tokio::process::Command::new("sh").arg("-c").arg(cmd).status().await {
        Ok(status) if !status.success() => {
            println!("[{}] [session] comando saiu com {status}: {cmd}", timestamp());
        }
        Err(error) => {
            println!("[{}] [session] falha ao rodar comando ({error}): {cmd}", timestamp());
        }
        Ok(_) => {}
    }
}

async fn run_shell_commands(commands: &[String]) {
    for cmd in commands {
        println!("[{}] [session] rodando: {cmd}", timestamp());
        run_shell_command(cmd).await;
    }
}

// So' usado pelo fechamento MANUAL - o watchdog nunca chama isso, porque
// so' age depois de confirmar que o processo JA morreu sozinho (nada pra
// matar). Aqui o jogo pode genuinamente ainda estar rodando, entao pede
// SIGTERM e espera ATE 20s - mas poll a cada 1s e sai assim que o
// processo morrer, em vez de sempre esperar os 20s inteiros como o
// array de undo antigo do Apollo fazia. So' manda SIGKILL se o periodo de
// graca inteiro passar sem o processo sair sozinho.
async fn kill_game_process(app_id: &str) {
    if !is_app_id_running(app_id) {
        println!("[{}] [session] app_id={app_id} ja nao esta rodando, nada pra matar", timestamp());
        return;
    }

    println!("[{}] [session] mandando SIGTERM (AppId={app_id})", timestamp());
    run_shell_command(&format!("pkill -TERM -f AppId={app_id}")).await;

    let grace_period = Duration::from_secs(20);
    let start = tokio::time::Instant::now();
    while is_app_id_running(app_id) {
        if start.elapsed() >= grace_period {
            println!(
                "[{}] [session] app_id={app_id} nao saiu sozinho no periodo de graca - forcando com SIGKILL",
                timestamp()
            );
            run_shell_command(&format!("pkill -KILL -f AppId={app_id}")).await;
            return;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    println!("[{}] [session] app_id={app_id} saiu sozinho depois do SIGTERM", timestamp());
}

// Registra a sessao - roda os display_commands DE FORMA SINCRONA antes
// de responder (por isso runner.py so' prossegue pro exec do Moonlight
// depois que esta chamada retorna: garante que a tela ja esta' no
// estado certo quando o stream comecar).
pub async fn register_session(
    Extension(state): Extension<SessionState>,
    Json(req): Json<RegisterSessionRequest>,
) -> Json<CloseResponse> {
    println!("[{}] [session] registrando app_id={} - ligando a tela...", timestamp(), req.app_id);
    run_shell_commands(&req.display_commands).await;
    println!("[{}] [session] tela ligada, sessao registrada: app_id={}", timestamp(), req.app_id);

    let mut guard = state.lock().await;
    *guard = Some(ActiveSession {
        app_id: req.app_id,
        username: req.username,
        password: req.password,
        restore_commands: req.restore_commands,
        confirmed_running: false,
    });
    Json(CloseResponse { ok: true, error: None })
}

// Fechamento IMEDIATO (manual, "Fechar conexao") - avisa o Apollo PRIMEIRO
// (derruba a conexao/stream no Deck na hora) e so' depois mata o jogo (se
// ainda estiver vivo - SIGTERM + espera adaptativa + SIGKILL, ver
// kill_game_process) e restaura a tela, em background, sem bloquear a
// resposta.
pub async fn close_session_now(
    Extension(state): Extension<SessionState>,
    Extension(ApolloBaseUrl(base_url)): Extension<ApolloBaseUrl>,
    Extension(notifier): Extension<EventNotifier>,
) -> Json<CloseResponse> {
    let session = state.lock().await.clone();

    let Some(session) = session else {
        println!("[{}] [session] fechamento manual pedido, mas nao ha sessao registrada", timestamp());
        return Json(CloseResponse {
            ok: false,
            error: Some("Nenhuma sessao registrada no Runner".to_string()),
        });
    };

    println!(
        "[{}] [session] fechamento manual pedido - avisando o Apollo agora (kill/restauro rodam depois, em background)",
        timestamp()
    );
    match apollo::close_session_at(&base_url, &session.username, &session.password).await {
        Ok(()) => {
            println!("[{}] [session] Apollo fechou a sessao com sucesso (fechamento manual)", timestamp());
            *state.lock().await = None;
            let _ = notifier.send(RunnerEvent::SessionClosed);

            // O Deck ja recebeu a confirmacao (o stream ja foi
            // derrubado) - matar o jogo (se ainda vivo) e restaurar a
            // tela nao precisam bloquear essa resposta.
            let app_id = session.app_id.clone();
            let restore_commands = session.restore_commands.clone();
            tokio::spawn(async move {
                kill_game_process(&app_id).await;
                run_shell_commands(&restore_commands).await;
            });

            Json(CloseResponse { ok: true, error: None })
        }
        Err(error) => {
            println!("[{}] [session] Apollo falhou ao fechar (fechamento manual): {error}", timestamp());
            Json(CloseResponse { ok: false, error: Some(error) })
        }
    }
}

// Roda pra sempre numa task em background (ver lib.rs) - a cada
// intervalo, checa se a sessao registrada ainda esta rodando de verdade
// (sysinfo, o SO nao mente mesmo quando o Apollo mente). Se morreu, avisa
// o Apollo IMEDIATAMENTE (derruba a conexao no Deck na hora - nao ha'
// prep-cmd nenhum no Apollo pra esperar) e so' depois restaura a tela, em
// background, sem bloquear essa etapa. Extraida como funcao separada (nao
// so' o loop inline) pra poder testar UMA passada sem precisar de
// sleep/timing de verdade no teste.
//
// Se o Apollo responder com erro (ex: credenciais erradas, ou host fora
// do ar momentaneamente) na etapa final de close, a sessao fica
// registrada e a proxima passada tenta de novo - nao desiste depois de N
// tentativas. Aceitavel pro escopo atual (LAN domestica, sem tentativas
// maliciosas de esgotar login); se algum dia virar problema real,
// adicionar um limite aqui. Repetir so' a etapa de close (nao os
// restore_commands de novo) evita rodar kscreen-doctor duas vezes por
// causa de uma falha momentanea so' na parte do Apollo.
pub async fn check_and_maybe_close_session(state: &SessionState, base_url: &str, notifier: &EventNotifier) -> bool {
    let session = state.lock().await.clone();

    let Some(session) = session else {
        return false;
    };

    let running = is_app_id_running(&session.app_id);
    println!(
        "[{}] [session] watchdog: checando app_id={}, rodando={running}, confirmado_antes={}",
        timestamp(),
        session.app_id,
        session.confirmed_running
    );

    if running {
        if !session.confirmed_running {
            // primeira vez que vemos o processo de verdade - marca antes
            // de sair, pra proxima passada ja saber que uma queda agora
            // e' fechamento de verdade, nao so' o jogo ainda carregando.
            if let Some(s) = state.lock().await.as_mut() {
                s.confirmed_running = true;
            }
        }
        return false;
    }

    if !session.confirmed_running {
        // Bug real encontrado no device: a primeira checagem do watchdog
        // pode acontecer ANTES do jogo terminar de abrir (Steam demora um
        // tempo variavel pra spawnar o processo com "AppId=" no cmdline -
        // Proton, shader cache, etc). Sem essa checagem, "ainda nao
        // apareceu" e "ja fechou" ficam indistinguiveis - o watchdog
        // fechava (e desfazia a tela) um jogo que estava so' carregando.
        // So' considera "fechou" depois de ter visto rodando de verdade
        // pelo menos uma vez.
        println!(
            "[{}] [session] watchdog: app_id={} ainda nao foi visto rodando (jogo carregando?) - nao fecha ainda",
            timestamp(),
            session.app_id
        );
        return false;
    }

    println!(
        "[{}] [session] watchdog: app_id={} nao esta mais rodando, avisando o Apollo agora (restauro roda depois, em background)",
        timestamp(),
        session.app_id
    );

    match apollo::close_session_at(base_url, &session.username, &session.password).await {
        Ok(()) => {
            println!("[{}] [session] watchdog: Apollo fechou a sessao com sucesso", timestamp());
            *state.lock().await = None;
            let _ = notifier.send(RunnerEvent::SessionClosed);

            // O Deck ja recebeu a desconexao - restaurar a tela nao
            // precisa bloquear isso, roda independente em background.
            let restore_commands = session.restore_commands.clone();
            tokio::spawn(async move {
                run_shell_commands(&restore_commands).await;
            });

            true
        }
        Err(error) => {
            println!(
                "[{}] [session] watchdog: Apollo falhou ao fechar ({error}) - tenta de novo no proximo tick",
                timestamp()
            );
            false
        }
    }
}

pub async fn watch_sessions(state: SessionState, base_url: String, notifier: EventNotifier) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        check_and_maybe_close_session(&state, &base_url, &notifier).await;
    }
}

#[cfg(test)]
#[path = "tests/session.rs"]
mod tests;
