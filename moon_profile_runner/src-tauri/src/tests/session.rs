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
async fn closes_via_apollo_before_restore_commands_finish_running() {
    // Prova a ordem nova no watchdog: o Apollo e' avisado (a funcao
    // retorna "closed") ANTES dos restore_commands terminarem de
    // rodar - eles seguem em background (tokio::spawn), sem atrasar
    // a desconexao do Deck.
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
        app_id: "900019".to_string(), // nenhum processo - "ja fechou"
        username: "user".to_string(),
        password: "pass".to_string(),
        restore_commands: vec![format!("sleep 0.3 && echo restaurado > {marker_path}")],
        confirmed_running: true,
    });
    let (tx, _rx) = mpsc::unbounded_channel();

    let closed = check_and_maybe_close_session(&state, &mock_server.uri(), &tx).await;

    assert!(closed);
    // logo apos retornar, o comando (que tem um sleep de 0.3s
    // embutido) ainda nao deveria ter terminado - prova que ele roda
    // DEPOIS, em background, nao antes de considerar a sessao fechada.
    assert!(std::fs::read_to_string(&marker_path).unwrap().is_empty());

    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if !std::fs::read_to_string(&marker_path).unwrap().is_empty() {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline, "restore_commands nao rodou a tempo em background");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
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

    // kill_game_process roda em BACKGROUND agora (tokio::spawn, nao
    // bloqueia a resposta acima) - poll com timeout em vez de checar
    // na hora, senao da' corrida com a task spawnada.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while is_app_id_running("900013") {
        assert!(tokio::time::Instant::now() < deadline, "processo fake nao morreu a tempo");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    drop(fake);
}

#[tokio::test]
async fn close_session_now_returns_before_the_kill_grace_period_would_finish() {
    // Prova a ordem nova: o Apollo e' avisado (e a resposta volta)
    // ANTES do kill/restauro terminarem - nao depois. Processo que
    // ignora SIGTERM (so' morre com SIGKILL) - se close_session_now
    // esperasse o periodo de graca inteiro (20s) antes de responder,
    // esse teste estouraria o timeout abaixo.
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

    let app_id = "900018";
    let marker = format!("AppId={app_id}");
    let mut fake = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(r#"trap "" TERM; exec -a "{marker}" sleep 30"#))
        .spawn()
        .expect("falha ao spawnar processo fake pro teste");
    tokio::time::sleep(Duration::from_millis(200)).await;

    let state = empty_state();
    *state.lock().await = Some(plain_session(app_id, "user", "pass"));
    let (tx, _rx) = mpsc::unbounded_channel();

    let start = tokio::time::Instant::now();
    let response = close_session_now(
        Extension(state),
        Extension(ApolloBaseUrl(mock_server.uri())),
        Extension(tx),
    )
    .await;

    assert!(response.0.ok);
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "close_session_now nao deveria esperar o periodo de graca do kill"
    );

    let _ = fake.kill(); // SIGKILL de verdade - limpeza, o trap so' ignora TERM
    let _ = fake.wait();
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
