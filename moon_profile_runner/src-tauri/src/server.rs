// Servidor HTTP embutido, falado pelo plugin Decky (moon_profile_decky) e
// por runner.py (Deck) - registra/lista jogos do host e cuida do fim de
// sessao (ver session.rs) sem depender do Apollo, que entra em modo
// "placebo" depois do auto-detach do stream_game e nunca mais reporta o
// app como parado (ver docs/prd.md Fase 5). O SO nao mente sobre o
// processo, mesmo quando o Apollo mente.
//
// Sem autenticacao - servidor aberto na rede local (decisao explicita:
// numa LAN domestica ja confiavel, o atrito de colar um token na config
// do Deck nao compensa o ganho de seguranca).

use axum::{extract::Extension, routing::get, routing::post, Json, Router};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tokio::sync::mpsc;

use crate::games::{list_host_games, HostGame};
use crate::session::{close_session_now, register_session, ApolloBaseUrl, SessionState};

// Eventos que server.rs/session.rs sinalizam pra lib.rs (que TEM o
// AppHandle de verdade) disparar notificacoes de desktop - os handlers
// axum em si nao conhecem Tauri/AppHandle nenhum. Decoupled assim de
// proposito: usar AppHandle direto aqui exigiria testes com
// tauri::test::mock_builder (generico sobre o Runtime, muito mais
// complexo pra testar um pulso) - um canal simples e' trivial de testar
// (so' cria um par e ignora o receiver).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerEvent {
    GamesSynced,
    SessionClosed,
}
pub type EventNotifier = mpsc::UnboundedSender<RunnerEvent>;

// "AppId=<id>" tem que ser seguido de um NAO-digito (ou fim de string) -
// um contains() puro faria "AppId=900004" bater como prefixo de
// "AppId=9000044" (falso positivo com outro AppId que so' compartilha o
// prefixo numerico). Coberto por
// `does_not_match_a_different_app_id_with_a_shared_prefix` abaixo.
fn cmd_arg_matches_app_id(arg: &str, needle: &str) -> bool {
    let Some(pos) = arg.find(needle) else {
        return false;
    };
    match arg[pos + needle.len()..].chars().next() {
        Some(c) => !c.is_ascii_digit(),
        None => true,
    }
}

// Mesma convencao de identificar o processo certo que main.py:_build_prep_cmd
// ja usa no "pkill -f AppId=<id>" do undo - so que aqui e' LEITURA, nao kill.
// pub(crate) porque session.rs (o watchdog de fechamento autonomo) usa a
// mesma checagem, sem precisar passar por HTTP.
pub(crate) fn is_app_id_running(app_id: &str) -> bool {
    let needle = format!("AppId={app_id}");
    let mut sys = System::new();
    // refresh_processes() (o metodo de conveniencia) usa um ProcessRefreshKind
    // padrao que NAO inclui cmd (so memoria/cpu/disco/exe) - cmd() sempre
    // voltava vazio sem isso, entao o match nunca batia (bug real,
    // confirmado rodando de verdade: processo existia, cmdline batia, e
    // mesmo assim "running: false"). refresh_processes_specifics com
    // with_cmd(Always) e' o jeito certo de pedir esse dado. Regressao real
    // ja encontrada uma vez, nao repetir.
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::new().with_cmd(UpdateKind::Always),
    );

    sys.processes().values().any(|process| {
        process
            .cmd()
            .iter()
            .any(|arg| cmd_arg_matches_app_id(&arg.to_string_lossy(), &needle))
    })
}

// "quando receber uma chamada de sincronia" - dispara o pulso ANTES de
// montar a resposta, no exato momento em que o Deck de fato pediu a lista.
async fn games(Extension(notify): Extension<EventNotifier>) -> Json<Vec<HostGame>> {
    println!("[server] GET /games (sincronizacao pedida pelo Deck)");
    let _ = notify.send(RunnerEvent::GamesSynced);
    Json(list_host_games().await)
}

pub fn app(notify: EventNotifier, session_state: SessionState, apollo_base_url: ApolloBaseUrl) -> Router {
    Router::new()
        .route("/games", get(games))
        .route("/session/register", post(register_session))
        .route("/session/close", post(close_session_now))
        .layer(Extension(notify))
        .layer(Extension(session_state))
        .layer(Extension(apollo_base_url))
}

pub async fn run_server(notify: EventNotifier, session_state: SessionState, apollo_base_url: ApolloBaseUrl) {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:47991")
        .await
        .expect("failed to bind runner HTTP server to port 47991");
    println!("[server] MoonProfile Runner escutando em 0.0.0.0:47991");

    axum::serve(listener, app(notify, session_state, apollo_base_url))
        .await
        .expect("runner HTTP server crashed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeGameProcess;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use tower::ServiceExt;

    // Unitario e rapido - nao precisa de processo de verdade, so cobre a
    // logica de match pura (os casos de borda do prefixo compartilhado).
    #[test]
    fn cmd_arg_matches_app_id_cases() {
        assert!(cmd_arg_matches_app_id("AppId=42", "AppId=42"));
        assert!(cmd_arg_matches_app_id("SteamLaunch AppId=42 --", "AppId=42"));
        assert!(!cmd_arg_matches_app_id("AppId=420", "AppId=42"));
        assert!(!cmd_arg_matches_app_id("AppId=4", "AppId=42"));
        assert!(!cmd_arg_matches_app_id("nada a ver", "AppId=42"));
    }

    // IDs de teste bem distintos (900xxx) pra nao colidir por acaso com um
    // AppId de jogo de verdade rodando na maquina de dev enquanto testa.

    #[tokio::test]
    async fn is_app_id_running_false_when_no_matching_process_exists() {
        assert!(!is_app_id_running("900001"));
    }

    #[tokio::test]
    async fn is_app_id_running_true_when_a_matching_process_is_alive() {
        let fake = FakeGameProcess::spawn("900002");
        tokio::time::sleep(Duration::from_millis(200)).await;

        assert!(is_app_id_running("900002"));
        drop(fake);
    }

    #[tokio::test]
    async fn is_app_id_running_false_again_after_the_process_exits() {
        let fake = FakeGameProcess::spawn("900003");
        tokio::time::sleep(Duration::from_millis(200)).await;
        drop(fake); // mata e espera terminar antes de perguntar de novo
        tokio::time::sleep(Duration::from_millis(200)).await;

        assert!(!is_app_id_running("900003"));
    }

    #[tokio::test]
    async fn is_app_id_running_does_not_match_a_different_app_id_with_a_shared_prefix() {
        // "900004" nao deveria bater com um processo que tem "AppId=9000044"
        // (prefixo compartilhado) - contains() e' substring, entao esse
        // caso confirma que o formato "AppId=<id>" exato (sem separador
        // depois) nao gera falso positivo aqui na pratica.
        let fake = FakeGameProcess::spawn("9000044");
        tokio::time::sleep(Duration::from_millis(200)).await;

        assert!(!is_app_id_running("900004"));
        drop(fake);
    }

    fn test_app() -> Router {
        let (tx, _rx) = mpsc::unbounded_channel();
        let session_state: SessionState = Arc::new(Mutex::new(None));
        app(tx, session_state, ApolloBaseUrl("http://127.0.0.1:0".to_string()))
    }

    // So confirma que a rota esta' registrada e devolve um JSON valido - a
    // logica de parsing dos jogos em si (as fixtures de VDF, o caso de
    // "libraryfolders.vdf ausente", etc) ja' e' coberta a fundo em
    // games.rs; duplicar isso aqui so' testaria a mesma coisa duas vezes.
    #[tokio::test]
    async fn games_route_returns_a_json_array() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/games")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let _games: Vec<crate::games::HostGame> = serde_json::from_slice(&bytes).unwrap();
    }

    // "quando receber uma chamada de sincronia" - o handler manda um evento
    // pro canal a cada GET /games, que lib.rs escuta pra disparar a
    // notificacao de desktop de verdade (com AppHandle real, fora do
    // alcance deste teste - testar so' o sinal, nao a notificacao em si,
    // evita precisar de tauri::test::mock_builder generico sobre Runtime).
    #[tokio::test]
    async fn games_route_sends_a_games_synced_event() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let session_state: SessionState = Arc::new(Mutex::new(None));

        let _ = app(tx, session_state, ApolloBaseUrl("http://127.0.0.1:0".to_string()))
            .oneshot(
                Request::builder()
                    .uri("/games")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(rx.try_recv(), Ok(RunnerEvent::GamesSynced));
    }
}
