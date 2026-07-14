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
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::apollo;
use crate::server::{is_app_id_running, EventNotifier, RunnerEvent};

#[derive(Clone)]
pub struct ActiveSession {
    pub app_id: String,
    pub username: String,
    pub password: String,
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
    let mut guard = state.lock().await;
    *guard = Some(ActiveSession {
        app_id: req.app_id,
        username: req.username,
        password: req.password,
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
        return Json(CloseResponse {
            ok: false,
            error: Some("Nenhuma sessao registrada no Runner".to_string()),
        });
    };

    match apollo::close_session_at(&base_url, &username, &password).await {
        Ok(()) => {
            *state.lock().await = None;
            let _ = notifier.send(RunnerEvent::SessionClosed);
            Json(CloseResponse { ok: true, error: None })
        }
        Err(error) => Json(CloseResponse { ok: false, error: Some(error) }),
    }
}

// Roda pra sempre numa task em background (ver lib.rs) - a cada
// intervalo, checa se a sessao registrada ainda esta rodando de verdade
// (sysinfo, o SO nao mente mesmo quando o Apollo mente). Se morreu, fecha
// sozinho no Apollo e limpa o estado. Extraida como funcao separada (nao
// so' o loop inline) pra poder testar UMA passada sem precisar de
// sleep/timing de verdade no teste.
//
// Se o Apollo responder com erro (ex: credenciais erradas, ou host fora
// do ar momentaneamente), a sessao fica registrada e a proxima passada
// tenta de novo - nao desiste depois de N tentativas. Aceitavel pro
// escopo atual (LAN domestica, sem tentativas maliciosas de esgotar
// login); se algum dia virar problema real, adicionar um limite aqui.
pub async fn check_and_maybe_close_session(state: &SessionState, base_url: &str, notifier: &EventNotifier) -> bool {
    let session = { state.lock().await.clone().map(|s| (s.app_id, s.username, s.password)) };

    let Some((app_id, username, password)) = session else {
        return false;
    };

    if is_app_id_running(&app_id) {
        return false;
    }

    match apollo::close_session_at(base_url, &username, &password).await {
        Ok(()) => {
            *state.lock().await = None;
            let _ = notifier.send(RunnerEvent::SessionClosed);
            true
        }
        Err(_) => false,
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
        *state.lock().await = Some(ActiveSession {
            app_id: "900010".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
        });
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
        *state.lock().await = Some(ActiveSession {
            app_id: "900011".to_string(), // nenhum processo com esse AppId - "ja fechou"
            username: "user".to_string(),
            password: "pass".to_string(),
        });
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
        *state.lock().await = Some(ActiveSession {
            app_id: "900012".to_string(),
            username: "user".to_string(),
            password: "wrong".to_string(),
        });
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
        };

        let response = register_session(Extension(state.clone()), Json(req)).await;

        assert!(response.0.ok);
        let guard = state.lock().await;
        assert_eq!(guard.as_ref().unwrap().app_id, "123");
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
        *state.lock().await = Some(ActiveSession {
            app_id: "900013".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
        });
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
}
