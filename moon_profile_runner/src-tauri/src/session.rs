// Fechamento autonomo de sessao: runner.py (Deck) registra a sessao ao
// configurar o Apollo (app_id + credenciais, EM MEMORIA, nunca gravadas
// em disco no host) logo antes de dar exec no Moonlight - dai um
// watchdog em background (mesma runtime tokio do servidor HTTP, ver
// lib.rs) fica de olho no processo via sysinfo (server.rs:
// is_app_id_running) e, quando detecta que o jogo fechou, chama o Apollo
// sozinho (login + /api/apps/close, que dispara o "undo" do prep-cmd em
// ordem reversa - kscreen-doctor + pkill do jogo), sem precisar do Deck
// pedir nada. Cobre os dois fluxos de lancamento (botao antigo e atalho
// novo por jogo), ja que os dois passam pelo mesmo runner.py.
//
// "Fechar conexao" manual (QuickAccessContent.tsx) tambem bate aqui
// primeiro (POST /session/close) - so cai pro caminho antigo (Deck
// falando com o Apollo direto, ver main.py:stop_stream) se o Runner nao
// tiver sessao registrada ou estiver inalcancavel (ele e' opcional).

use axum::extract::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    // Payload OPACO pra reconfigurar o app "SteamGame" com prep-cmd
    // vazio antes do fechamento autonomo - evita que o Apollo rode de
    // novo (mais devagar, com os pkill/sleep-20 do array original) um
    // trabalho que o watchdog ja fez sozinho via restore_commands acima
    // (ver _quick_close_payload em runner.py).
    pub quick_close_payload: Value,
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
    #[serde(default)]
    restore_commands: Vec<String>,
    #[serde(default)]
    quick_close_payload: Value,
}

#[derive(Serialize)]
pub struct CloseResponse {
    ok: bool,
    error: Option<String>,
}

pub async fn register_session(
    Extension(state): Extension<SessionState>,
    Json(req): Json<RegisterSessionRequest>,
) -> Json<CloseResponse> {
    println!("[{}] [session] registrada: app_id={}", timestamp(), req.app_id);
    let mut guard = state.lock().await;
    *guard = Some(ActiveSession {
        app_id: req.app_id,
        username: req.username,
        password: req.password,
        restore_commands: req.restore_commands,
        quick_close_payload: req.quick_close_payload,
    });
    Json(CloseResponse { ok: true, error: None })
}

// Fechamento IMEDIATO (manual, "Fechar conexao") - nao espera o watchdog
// detectar nada, usa a sessao registrada (se existir) pra chamar o Apollo
// agora mesmo.
pub async fn close_session_now(
    Extension(state): Extension<SessionState>,
    Extension(ApolloBaseUrl(base_url)): Extension<ApolloBaseUrl>,
    Extension(notifier): Extension<EventNotifier>,
) -> Json<CloseResponse> {
    let session = { state.lock().await.clone().map(|s| (s.username, s.password)) };

    let Some((username, password)) = session else {
        println!("[{}] [session] fechamento manual pedido, mas nao ha sessao registrada", timestamp());
        return Json(CloseResponse {
            ok: false,
            error: Some("Nenhuma sessao registrada no Runner".to_string()),
        });
    };

    println!("[{}] [session] fechamento manual pedido - chamando o Apollo", timestamp());
    match apollo::close_session_at(&base_url, &username, &password).await {
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

// Roda cada comando de restauracao (kscreen-doctor/fechar Big Picture,
// pre-calculados por runner.py) direto via shell, sequencialmente - o
// mesmo formato de string unica que o Apollo ja usa nos prep-cmd
// (ver moonprofile_core.py:build_restore_commands). Best-effort: um
// comando falhando (ex: output ja desligado) nao impede os proximos de
// rodar - loga e segue, mesma filosofia de fail-open do resto do projeto.
async fn run_restore_commands(commands: &[String]) {
    for cmd in commands {
        println!("[{}] [session] watchdog: rodando comando de restauracao: {cmd}", timestamp());
        match tokio::process::Command::new("sh").arg("-c").arg(cmd).status().await {
            Ok(status) if !status.success() => {
                println!("[{}] [session] watchdog: comando de restauracao saiu com {status}: {cmd}", timestamp());
            }
            Err(error) => {
                println!("[{}] [session] watchdog: falha ao rodar comando de restauracao ({error}): {cmd}", timestamp());
            }
            Ok(_) => {}
        }
    }
}

// Roda pra sempre numa task em background (ver lib.rs) - a cada
// intervalo, checa se a sessao registrada ainda esta rodando de verdade
// (sysinfo, o SO nao mente mesmo quando o Apollo mente). Se morreu,
// restaura a tela IMEDIATAMENTE (direto via shell, sem esperar o Apollo -
// o array de undo original do Apollo tem um "sleep 20" de proposito, pra
// dar tempo de um jogo AINDA VIVO se fechar sozinho, o que nao se aplica
// aqui: o processo ja foi confirmado morto) e so' depois avisa o Apollo
// (reconfigurado com prep-cmd vazio via quick_close_payload, pra ele nao
// repetir esse trabalho mais devagar) - fechamento em ~1s em vez de
// ~25-30s. Extraida como funcao separada (nao so' o loop inline) pra
// poder testar UMA passada sem precisar de sleep/timing de verdade no
// teste.
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
    println!("[{}] [session] watchdog: checando app_id={}, rodando={running}", timestamp(), session.app_id);
    if running {
        return false;
    }

    println!(
        "[{}] [session] watchdog: app_id={} nao esta mais rodando, restaurando a tela e fechando no Apollo",
        timestamp(),
        session.app_id
    );
    run_restore_commands(&session.restore_commands).await;

    if session.quick_close_payload != Value::Null {
        if let Err(error) = apollo::save_app_at(base_url, &session.username, &session.password, &session.quick_close_payload).await {
            println!("[{}] [session] watchdog: falha ao neutralizar o undo do Apollo ({error}) - fechando mesmo assim", timestamp());
        }
    }

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
    use serde_json::json;
    use tokio::sync::mpsc;
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn empty_state() -> SessionState {
        Arc::new(Mutex::new(None))
    }

    // Sessao sem restore_commands/quick_close_payload - pros testes que
    // nao se importam com essa parte (ja cobertos separadamente abaixo).
    fn plain_session(app_id: &str, username: &str, password: &str) -> ActiveSession {
        ActiveSession {
            app_id: app_id.to_string(),
            username: username.to_string(),
            password: password.to_string(),
            restore_commands: Vec::new(),
            quick_close_payload: Value::Null,
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
    async fn register_session_stores_the_session_in_state() {
        let state = empty_state();
        let req = RegisterSessionRequest {
            app_id: "123".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
            restore_commands: vec!["echo restaurando".to_string()],
            quick_close_payload: json!({"prep-cmd": []}),
        };

        let response = register_session(Extension(state.clone()), Json(req)).await;

        assert!(response.0.ok);
        let guard = state.lock().await;
        assert_eq!(guard.as_ref().unwrap().app_id, "123");
        assert_eq!(guard.as_ref().unwrap().restore_commands, vec!["echo restaurando".to_string()]);
        assert_eq!(guard.as_ref().unwrap().quick_close_payload, json!({"prep-cmd": []}));
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
    async fn close_session_now_closes_immediately_regardless_of_process_state() {
        // fechamento MANUAL - fecha mesmo que o processo ainda esteja
        // rodando (diferente do watchdog, que so' fecha quando detecta que
        // ja morreu sozinho).
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
        drop(fake);
    }

    #[tokio::test]
    async fn watchdog_runs_the_restore_commands_before_closing() {
        // Comando com efeito colateral observavel (escreve num arquivo
        // temporario) em vez de so' confiar no retorno da funcao - testa
        // que o shell de verdade roda o comando, nao so' que a funcao nao
        // deu erro (mesma filosofia de testar comportamento real do resto
        // do projeto).
        let marker_file = tempfile::NamedTempFile::new().unwrap();
        let marker_path = marker_file.path().to_str().unwrap().to_string();

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
        *state.lock().await = Some(ActiveSession {
            app_id: "900014".to_string(), // nenhum processo com esse AppId - "ja fechou"
            username: "user".to_string(),
            password: "pass".to_string(),
            restore_commands: vec![format!("echo restaurado > {marker_path}")],
            quick_close_payload: Value::Null,
        });
        let (tx, _rx) = mpsc::unbounded_channel();

        let closed = check_and_maybe_close_session(&state, &mock_server.uri(), &tx).await;

        assert!(closed);
        let contents = std::fs::read_to_string(&marker_path).unwrap();
        assert_eq!(contents.trim(), "restaurado");
    }

    #[tokio::test]
    async fn watchdog_neutralizes_apollo_undo_before_closing_when_a_quick_close_payload_is_set() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/login"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;
        // so' aceita o POST /api/apps se o body bater com o payload que
        // registramos - confirma que o watchdog manda o payload de
        // verdade, nao um generico.
        Mock::given(method("POST"))
            .and(path("/api/apps"))
            .and(body_partial_json(json!({"prep-cmd": []})))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/apps/close"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let state = empty_state();
        *state.lock().await = Some(ActiveSession {
            app_id: "900015".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
            restore_commands: Vec::new(),
            quick_close_payload: json!({"name": "SteamGame", "prep-cmd": []}),
        });
        let (tx, _rx) = mpsc::unbounded_channel();

        let closed = check_and_maybe_close_session(&state, &mock_server.uri(), &tx).await;

        // Se o watchdog nao tivesse mandado o POST /api/apps esperado, o
        // wiremock nao teria mock pra bater e o request falharia - closed
        // so' vira true se a sequencia toda (restore -> save_app -> close)
        // funcionou.
        assert!(closed);
    }

    #[tokio::test]
    async fn watchdog_skips_apollo_reconfiguration_when_no_quick_close_payload_was_registered() {
        // Sessoes registradas sem quick_close_payload (Value::Null, o
        // default) nao devem tentar reconfigurar o Apollo - so' fecham
        // direto. expect(0) faz o wiremock derrubar o teste se
        // POST /api/apps for chamado mesmo assim.
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/login"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/apps"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&mock_server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/apps/close"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let state = empty_state();
        *state.lock().await = Some(plain_session("900016", "user", "pass"));
        let (tx, _rx) = mpsc::unbounded_channel();

        let closed = check_and_maybe_close_session(&state, &mock_server.uri(), &tx).await;

        assert!(closed);
    }
}
