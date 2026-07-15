use super::*;
use crate::test_support::FakeGameProcess;
use tokio::sync::mpsc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn empty_state() -> SessionState {
    Arc::new(Mutex::new(None))
}

// Session with no restore_commands, already with confirmed_running=true
// (simulates a game that has already been seen running before) - for
// tests that don't care about the startup race (covered separately
// below).
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

// Real bug found on-device: the watchdog's first check happened only
// 4.2s after registration, finding "not running" - the game
// (Ghostwire: Tokyo) was still LOADING, the process with "AppId=" in
// its cmdline (Steam's reaper) hadn't shown up yet. Without the
// confirmed_running check, this closed (and tore down the screen for)
// a game that never actually finished closing.
#[tokio::test]
async fn does_not_close_a_session_that_was_never_seen_running_yet() {
    let state = empty_state();
    *state.lock().await = Some(ActiveSession {
        app_id: "900020".to_string(), // no process - simulates "still loading"
        username: "user".to_string(),
        password: "pass".to_string(),
        restore_commands: Vec::new(),
        confirmed_running: false,
    });
    let (tx, _rx) = mpsc::unbounded_channel();

    let closed = check_and_maybe_close_session(&state, "http://127.0.0.1:0", &tx).await;

    assert!(!closed);
    let guard = state.lock().await;
    assert!(guard.is_some(), "session should not have been cleared - the game might just be loading");
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
    // simulates the real cycle: the watchdog sees the game alive first
    // (confirmed_running becomes true), only THEN does it actually close.
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
    // no process with this AppId - "already closed"
    *state.lock().await = Some(plain_session("900011", "user", "pass"));
    let (tx, mut rx) = mpsc::unbounded_channel();

    let closed = check_and_maybe_close_session(&state, &mock_server.uri(), &tx).await;

    assert!(closed);
    assert!(state.lock().await.is_none());
    assert_eq!(rx.try_recv(), Ok(RunnerEvent::SessionClosed));
}

#[tokio::test]
async fn closes_via_apollo_before_restore_commands_finish_running() {
    // Proves the new order in the watchdog: Apollo is notified (the
    // function returns "closed") BEFORE the restore_commands finish
    // running - they run in the background (tokio::spawn), without
    // delaying the Deck's disconnect.
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
        app_id: "900019".to_string(), // no process - "already closed"
        username: "user".to_string(),
        password: "pass".to_string(),
        restore_commands: vec![format!("sleep 0.3 && echo restored > {marker_path}")],
        confirmed_running: true,
    });
    let (tx, _rx) = mpsc::unbounded_channel();

    let closed = check_and_maybe_close_session(&state, &mock_server.uri(), &tx).await;

    assert!(closed);
    // right after returning, the command (which has a 0.3s sleep baked
    // in) shouldn't have finished yet - proves it runs AFTERWARDS, in
    // the background, not before the session is considered closed.
    assert!(std::fs::read_to_string(&marker_path).unwrap().is_empty());

    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if !std::fs::read_to_string(&marker_path).unwrap().is_empty() {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline, "restore_commands did not run in time in the background");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn keeps_the_session_registered_when_apollo_call_fails() {
    // wrong credentials (or Apollo down) - tries again next tick instead
    // of giving up and losing the session.
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
    // Observable side effect (writes to a temp file) instead of just
    // trusting the return value - confirms the real shell runs the
    // screen command, not just that the function didn't error.
    let marker_file = tempfile::NamedTempFile::new().unwrap();
    let marker_path = marker_file.path().to_str().unwrap().to_string();

    let state = empty_state();
    let req = RegisterSessionRequest {
        app_id: "123".to_string(),
        username: "user".to_string(),
        password: "pass".to_string(),
        display_commands: vec![format!("echo on > {marker_path}")],
        restore_commands: vec!["echo restoring".to_string()],
    };

    let response = register_session(Extension(state.clone()), Json(req)).await;

    assert!(response.0.ok);
    let contents = std::fs::read_to_string(&marker_path).unwrap();
    assert_eq!(contents.trim(), "on");

    let guard = state.lock().await;
    assert_eq!(guard.as_ref().unwrap().app_id, "123");
    assert_eq!(guard.as_ref().unwrap().restore_commands, vec!["echo restoring".to_string()]);
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
    assert_eq!(response.0.error, Some("No session registered in the Runner".to_string()));
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

    // kill_game_process now runs in the BACKGROUND (tokio::spawn,
    // doesn't block the response above) - poll with a timeout instead
    // of checking right away, otherwise it races with the spawned task.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while is_app_id_running("900013") {
        assert!(tokio::time::Instant::now() < deadline, "fake process did not die in time");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    drop(fake);
}

#[tokio::test]
async fn close_session_now_returns_before_the_kill_grace_period_would_finish() {
    // Proves the new order: Apollo is notified (and the response comes
    // back) BEFORE the kill/restore finish, not after. A process that
    // ignores SIGTERM (only dies with SIGKILL) - if close_session_now
    // waited for the whole grace period (20s) before responding, this
    // test would blow past the timeout below.
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
        .expect("failed to spawn fake process for the test");
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
        "close_session_now should not wait for the kill's grace period"
    );

    let _ = fake.kill(); // a real SIGKILL - cleanup, the trap only ignores TERM
    let _ = fake.wait();
}

#[tokio::test]
async fn close_session_now_skips_killing_when_the_game_already_exited() {
    // No fake process at all - "AppId=900017" never existed.
    // kill_game_process should notice this and not attempt any pkill
    // (there's no direct test for the pkill itself here, just that the
    // whole flow completes normally without hanging).
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
