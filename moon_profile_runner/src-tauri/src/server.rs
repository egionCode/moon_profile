// Embedded HTTP server, talked to by the Decky plugin (moon_profile_decky)
// and by runner.py (Deck) - registers/lists host games and handles
// session ending (see session.rs) without depending on Apollo, which
// goes into "placebo" mode after stream_game's auto-detach and never
// reports the app as stopped again (see docs/prd.md Phase 5). The OS
// doesn't lie about the process, even when Apollo lies.
//
// No authentication - server open on the local network (explicit
// decision: on an already-trusted home LAN, the friction of pasting a
// token into the Deck's config isn't worth the security gain).

use axum::{extract::Extension, routing::get, routing::post, Json, Router};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tokio::sync::mpsc;

use crate::displays::{list_displays, HostDisplay};
use crate::games::{list_host_games, HostGame};
use crate::session::{close_session_now, register_session, ApolloBaseUrl, SessionState};

// Only for the diagnostic prints (register/watchdog/close) - without
// this the console logs from "tauri dev" didn't tell us WHAT SECOND
// each thing happened (relevant for the watchdog, which runs every few
// seconds).
pub(crate) fn timestamp() -> String {
    chrono::Local::now().format("%H:%M:%S%.3f").to_string()
}

// Events that server.rs/session.rs signal to lib.rs (which HAS the real
// AppHandle) to fire desktop notifications - the axum handlers
// themselves don't know about Tauri/AppHandle at all. Decoupled like
// this on purpose: using AppHandle directly here would require tests
// with tauri::test::mock_builder (generic over the Runtime, much more
// complex to test a single signal) - a simple channel is trivial to
// test (just create a pair and ignore the receiver).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerEvent {
    GamesSynced,
    SessionClosed,
}
pub type EventNotifier = mpsc::UnboundedSender<RunnerEvent>;

// "AppId=<id>" has to be followed by a NON-digit (or end of string) - a
// plain contains() would make "AppId=900004" match as a prefix of
// "AppId=9000044" (false positive with a different AppId that just
// shares the numeric prefix). Covered by
// `does_not_match_a_different_app_id_with_a_shared_prefix` below.
fn cmd_arg_matches_app_id(arg: &str, needle: &str) -> bool {
    let Some(pos) = arg.find(needle) else {
        return false;
    };
    match arg[pos + needle.len()..].chars().next() {
        Some(c) => !c.is_ascii_digit(),
        None => true,
    }
}

// Same convention for identifying the right process that
// main.py:_build_prep_cmd already uses in the undo's
// "pkill -f AppId=<id>" - except here it's a READ, not a kill.
// pub(crate) because session.rs (the autonomous-close watchdog) uses the
// same check, without going through HTTP.
pub(crate) fn is_app_id_running(app_id: &str) -> bool {
    let needle = format!("AppId={app_id}");
    let mut sys = System::new();
    // refresh_processes() (the convenience method) uses a default
    // ProcessRefreshKind that does NOT include cmd (only memory/cpu/disk/
    // exe) - cmd() always came back empty without this, so the match
    // never hit (a real bug, confirmed by running it for real: the
    // process existed, the cmdline matched, and yet "running: false").
    // refresh_processes_specifics with with_cmd(Always) is the right way
    // to ask for this data. A real regression already hit once, don't
    // repeat it.
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

// "when a sync call is received" - fires the pulse BEFORE building the
// response, at the exact moment the Deck actually requested the list.
async fn games(Extension(notify): Extension<EventNotifier>) -> Json<Vec<HostGame>> {
    println!("[{}] [server] GET /games (sync requested by the Deck)", timestamp());
    let _ = notify.send(RunnerEvent::GamesSynced);
    Json(list_host_games().await)
}

// GET /displays - lists the host's monitors/outputs (via kscreen-doctor)
// for the Deck's UI to populate a select instead of the user typing the
// output name by hand (see moon_profile_decky/src/ProfileEditor.tsx).
async fn displays() -> Json<Vec<HostDisplay>> {
    Json(list_displays())
}

pub fn app(notify: EventNotifier, session_state: SessionState, apollo_base_url: ApolloBaseUrl) -> Router {
    Router::new()
        .route("/games", get(games))
        .route("/displays", get(displays))
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
    println!("[{}] [server] MoonProfile Runner listening on 0.0.0.0:47991", timestamp());

    axum::serve(listener, app(notify, session_state, apollo_base_url))
        .await
        .expect("runner HTTP server crashed");
}

#[cfg(test)]
#[path = "tests/server.rs"]
mod tests;
