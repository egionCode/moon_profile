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
// SIGKILL se preciso) antes de restaurar a tela e avisar o Apollo -
// diferente do watchdog, que so' age depois de confirmar que o processo
// ja morreu sozinho.

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

// Fechamento IMEDIATO (manual, "Fechar conexao") - mata o jogo se ainda
// estiver vivo (SIGTERM + espera adaptativa + SIGKILL, ver
// kill_game_process), restaura a tela, e so' entao avisa o Apollo pra
// derrubar a conexao.
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

    println!("[{}] [session] fechamento manual pedido - matando o jogo (se vivo) e restaurando a tela", timestamp());
    kill_game_process(&session.app_id).await;
    run_shell_commands(&session.restore_commands).await;

    match apollo::close_session_at(&base_url, &session.username, &session.password).await {
        Ok(()) => {
            println!("[{}] [session] Apollo fechou a sessao com sucesso (fechamento manual)", timestamp());
            *state.lock().await = None;
            let _ = notifier.send(RunnerEvent::SessionClosed);
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
// (sysinfo, o SO nao mente mesmo quando o Apollo mente). Se morreu,
// restaura a tela e avisa o Apollo - fechamento em ~1s, ja que nao ha'
// processo pra matar (o watchdog so' age DEPOIS de confirmar a morte) nem
// prep-cmd nenhum no Apollo pra esperar. Extraida como funcao separada
// (nao so' o loop inline) pra poder testar UMA passada sem precisar de
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
        "[{}] [session] watchdog: app_id={} nao esta mais rodando, restaurando a tela e fechando no Apollo",
        timestamp(),
        session.app_id
    );
    run_shell_commands(&session.restore_commands).await;

    match apollo::close_session_at(base_url, &session.username, &session.password).await {
        Ok(()) => {
            println!("[{}] [session] watchdog: Apollo fechou a sessao com sucesso", timestamp());
            *state.lock().await = None;
            let _ = notifier.send(RunnerEvent::SessionClosed);
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
mod tests {
    use super::*;
    use crate::test_support::FakeGameProcess;
    use tokio::sync::mpsc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn empty_state() -> SessionState {
        Arc::new(Mutex::new(None))
    }

    // Sessao sem restore_commands, ja com confirmed_running=true (simula
    // um jogo que ja foi visto rodando antes) - pros testes que nao se
    // importam com a corrida de inicializacao (coberta separadamente
    // abaixo).
    fn plain_session(app_id: &str, username: &str, password: &str) -> ActiveSession {
        ActiveSession {
            app_id: app_id.to_string(),
            username: username.to_string(),
            password: password.to_string(),
            restore_commands: Vec::new(),
            confirmed_running: true,
        }
    }

    #[tokio::test]
    async fn does_nothing_when_no_session_is_registered() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let closed = check_and_maybe_close_session(&empty_state(), "http://127.0.0.1:0", &tx).await;

        assert!(!closed);
    }

    #[tokio::test]
    async fn keeps_the_session_while_the_process_is_still_running() {
        let fake = FakeGameProcess::spawn("900010");
        tokio::time::sleep(Duration::from_millis(200)).await;

        let state = empty_state();
        *state.lock().await = Some(plain_session("900010", "user", "pass"));
        let (tx, _rx) = mpsc::unbounded_channel();

        let closed = check_and_maybe_close_session(&state, "http://127.0.0.1:0", &tx).await;

        assert!(!closed);
        assert!(state.lock().await.is_some());
        drop(fake);
    }

    // Bug real encontrado no device: a primeira checagem do watchdog
    // aconteceu so' 4.2s depois do registro, achando "nao esta rodando" -
    // o jogo (Ghostwire: Tokyo) ainda estava CARREGANDO, o processo com
    // "AppId=" no cmdline (reaper do Steam) ainda nao tinha aparecido.
    // Sem a checagem de confirmed_running, isso fechava (e desfazia a
    // tela) um jogo que nunca chegou a fechar de verdade.
    #[tokio::test]
    async fn does_not_close_a_session_that_was_never_seen_running_yet() {
        let state = empty_state();
        *state.lock().await = Some(ActiveSession {
            app_id: "900020".to_string(), // nenhum processo - simula "ainda carregando"
            username: "user".to_string(),
            password: "pass".to_string(),
            restore_commands: Vec::new(),
            confirmed_running: false,
        });
        let (tx, _rx) = mpsc::unbounded_channel();

        let closed = check_and_maybe_close_session(&state, "http://127.0.0.1:0", &tx).await;

        assert!(!closed);
        let guard = state.lock().await;
        assert!(guard.is_some(), "sessao nao deveria ter sido limpa - jogo pode so' estar carregando");
        assert!(!guard.as_ref().unwrap().confirmed_running);
    }

    #[tokio::test]
    async fn marks_confirmed_running_the_first_time_the_process_is_seen_alive() {
        let fake = FakeGameProcess::spawn("900021");
        tokio::time::sleep(Duration::from_millis(200)).await;

        let state = empty_state();
        *state.lock().await = Some(ActiveSession {
            app_id: "900021".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
            restore_commands: Vec::new(),
            confirmed_running: false,
        });
        let (tx, _rx) = mpsc::unbounded_channel();

        let closed = check_and_maybe_close_session(&state, "http://127.0.0.1:0", &tx).await;

        assert!(!closed);
        assert!(state.lock().await.as_ref().unwrap().confirmed_running);
        drop(fake);
    }

    #[tokio::test]
    async fn closes_once_confirmed_running_and_then_the_process_disappears() {
        // simula o ciclo de verdade: watchdog ve o jogo vivo primeiro
        // (confirmed_running vira true), so' DEPOIS ele fecha de fato.
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/login"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/apps/close"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let fake = FakeGameProcess::spawn("900022");
        tokio::time::sleep(Duration::from_millis(200)).await;

        let state = empty_state();
        *state.lock().await = Some(ActiveSession {
            app_id: "900022".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
            restore_commands: Vec::new(),
            confirmed_running: false,
        });
        let (tx, mut rx) = mpsc::unbounded_channel();

        let closed_while_alive = check_and_maybe_close_session(&state, &mock_server.uri(), &tx).await;
        assert!(!closed_while_alive);
        assert!(state.lock().await.as_ref().unwrap().confirmed_running);

        drop(fake);
        tokio::time::sleep(Duration::from_millis(200)).await;

        let closed_after_exit = check_and_maybe_close_session(&state, &mock_server.uri(), &tx).await;

        assert!(closed_after_exit);
        assert!(state.lock().await.is_none());
        assert_eq!(rx.try_recv(), Ok(RunnerEvent::SessionClosed));
    }

    #[tokio::test]
    async fn closes_via_apollo_and_clears_state_once_the_process_has_exited() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/login"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/apps/close"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let state = empty_state();
        // nenhum processo com esse AppId - "ja fechou"
        *state.lock().await = Some(plain_session("900011", "user", "pass"));
        let (tx, mut rx) = mpsc::unbounded_channel();

        let closed = check_and_maybe_close_session(&state, &mock_server.uri(), &tx).await;

        assert!(closed);
        assert!(state.lock().await.is_none());
        assert_eq!(rx.try_recv(), Ok(RunnerEvent::SessionClosed));
    }

    #[tokio::test]
    async fn keeps_the_session_registered_when_apollo_call_fails() {
        // credenciais erradas (ou Apollo fora do ar) - tenta de novo no
        // proximo tick em vez de desistir e perder a sessao.
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/login"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock_server)
            .await;

        let state = empty_state();
        *state.lock().await = Some(plain_session("900012", "user", "wrong"));
        let (tx, _rx) = mpsc::unbounded_channel();

        let closed = check_and_maybe_close_session(&state, &mock_server.uri(), &tx).await;

        assert!(!closed);
        assert!(state.lock().await.is_some());
    }

    #[tokio::test]
    async fn register_session_runs_display_commands_before_storing_the_session() {
        // Efeito colateral observavel (escreve num arquivo temporario) em
        // vez de so' confiar no retorno - confirma que o shell de verdade
        // roda o comando de tela, nao so' que a funcao nao deu erro.
        let marker_file = tempfile::NamedTempFile::new().unwrap();
        let marker_path = marker_file.path().to_str().unwrap().to_string();

        let state = empty_state();
        let req = RegisterSessionRequest {
            app_id: "123".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
            display_commands: vec![format!("echo ligado > {marker_path}")],
            restore_commands: vec!["echo restaurando".to_string()],
        };

        let response = register_session(Extension(state.clone()), Json(req)).await;

        assert!(response.0.ok);
        let contents = std::fs::read_to_string(&marker_path).unwrap();
        assert_eq!(contents.trim(), "ligado");

        let guard = state.lock().await;
        assert_eq!(guard.as_ref().unwrap().app_id, "123");
        assert_eq!(guard.as_ref().unwrap().restore_commands, vec!["echo restaurando".to_string()]);
        assert!(!guard.as_ref().unwrap().confirmed_running);
    }

    #[tokio::test]
    async fn close_session_now_reports_no_session_when_none_is_registered() {
        let state = empty_state();
        let (tx, _rx) = mpsc::unbounded_channel();

        let response = close_session_now(
            Extension(state),
            Extension(ApolloBaseUrl("http://127.0.0.1:0".to_string())),
            Extension(tx),
        )
        .await;

        assert!(!response.0.ok);
        assert_eq!(response.0.error, Some("Nenhuma sessao registrada no Runner".to_string()));
    }

    #[tokio::test]
    async fn close_session_now_kills_the_game_when_it_is_still_running() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/login"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/apps/close"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let fake = FakeGameProcess::spawn("900013");
        tokio::time::sleep(Duration::from_millis(200)).await;

        let state = empty_state();
        *state.lock().await = Some(plain_session("900013", "user", "pass"));
        let (tx, mut rx) = mpsc::unbounded_channel();

        let response = close_session_now(
            Extension(state.clone()),
            Extension(ApolloBaseUrl(mock_server.uri())),
            Extension(tx),
        )
        .await;

        assert!(response.0.ok);
        assert!(state.lock().await.is_none());
        assert_eq!(rx.try_recv(), Ok(RunnerEvent::SessionClosed));
        // pkill -TERM (mandado por kill_game_process) deve ter derrubado
        // o processo fake de verdade - confirma comportamento real, nao
        // so' que a funcao retornou sem erro.
        assert!(!is_app_id_running("900013"));
        drop(fake);
    }

    #[tokio::test]
    async fn close_session_now_skips_killing_when_the_game_already_exited() {
        // Sem processo fake nenhum - "AppId=900017" nunca existiu.
        // kill_game_process deve perceber isso e nao tentar nenhum pkill
        // (nao ha' teste direto pro pkill em si aqui, so' que o fluxo
        // completa normalmente sem travar).
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/login"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/apps/close"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let state = empty_state();
        *state.lock().await = Some(plain_session("900017", "user", "pass"));
        let (tx, mut rx) = mpsc::unbounded_channel();

        let response = close_session_now(
            Extension(state.clone()),
            Extension(ApolloBaseUrl(mock_server.uri())),
            Extension(tx),
        )
        .await;

        assert!(response.0.ok);
        assert_eq!(rx.try_recv(), Ok(RunnerEvent::SessionClosed));
    }
}
