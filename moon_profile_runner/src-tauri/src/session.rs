// Session lifecycle, 100% controlled by the Runner - Apollo no longer
// has any prep-cmd at all (neither "do" nor "undo"), an explicit
// decision to keep it simpler ("plug and play": it only needs to know
// how to connect and run the "cmd") and to give the Deck full control
// over the host's screen. Whoever turns the screen on at launch
// (display_commands) and off at close (restore_commands) is always
// this module, running the commands directly via shell.
//
// runner.py (Deck) registers the session while configuring Apollo
// (app_id + credentials, IN MEMORY, never written to disk on the host,
// plus the screen commands precomputed from the profile) - the
// registration itself already runs the display_commands SYNCHRONOUSLY
// (it only responds afterwards), so the Runner stopped being optional:
// without it, the screen never switches.
//
// Once registered, a background watchdog (same tokio runtime as the
// HTTP server, see lib.rs) keeps an eye on the process via sysinfo
// (server.rs: is_app_id_running) and, when it detects the game closed
// on its own, restores the screen and tells Apollo to drop the
// connection, without the Deck needing to ask for anything. This covers
// both launch flows (the old button and the new per-game shortcut),
// since both go through the same runner.py.
//
// Manual "close connection" (QuickAccessContent.tsx -> POST
// /session/close) kills the game (if still alive, with SIGTERM +
// adaptive wait + SIGKILL if needed) and restores the screen, unlike
// the watchdog, which only acts after confirming the process has
// already died on its own.
//
// ORDER matters for the user experience: in BOTH flows, Apollo is
// notified (which drops the stream/disconnects Moonlight on the Deck,
// taking it off the streaming screen) BEFORE killing the game/restoring
// the screen, not after. Killing the game (which can take up to 20s of
// grace period on manual close) and the restore commands run
// AFTERWARDS, in the background (tokio::spawn, without blocking the
// response) - the user sees the Deck leave the streaming screen right
// away, the rest happens independently on the host, without them
// waiting for it.

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
    // Screen restore commands (kscreen-doctor + closing Big Picture),
    // precomputed by runner.py from the profile - the Runner just
    // executes them, without interpreting what each one means (see
    // build_restore_commands in moonprofile_core.py).
    pub restore_commands: Vec<String>,
    // Real bug found on-device: the watchdog's first check can happen
    // BEFORE the game finishes opening (Steam takes a variable amount of
    // time to spawn the "reaper ... AppId=<id>" process - Proton, shader
    // cache, etc) - without this field, "hasn't shown up yet" and
    // "already closed" are indistinguishable, and the watchdog would
    // close a game that was just LOADING. Only starts considering it
    // "closed" after having seen the process actually running at least
    // once.
    pub confirmed_running: bool,
}

pub type SessionState = Arc<Mutex<Option<ActiveSession>>>;

// Newtype (not just a String) so it can be distinguished from other
// Extension<String> on the same Router - axum identifies extensions by
// TYPE, not by name.
#[derive(Clone)]
pub struct ApolloBaseUrl(pub String);

#[derive(Deserialize)]
pub struct RegisterSessionRequest {
    app_id: String,
    username: String,
    password: String,
    // Commands to TURN ON the screen (kscreen-doctor: enable/mode/hdr/
    // disable of the other outputs) - run RIGHT NOW, before responding
    // (see register_session), to guarantee the screen is already in the
    // right state by the time runner.py (Deck) proceeds to exec
    // Moonlight.
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

// Runs a shell command (single string, same format Apollo used to use
// in prep-cmd) - best-effort: a failure here is only logged, it doesn't
// stop the following commands (e.g. kscreen-doctor trying to turn off
// an output that's already off isn't fatal).
async fn run_shell_command(cmd: &str) {
    match tokio::process::Command::new("sh").arg("-c").arg(cmd).status().await {
        Ok(status) if !status.success() => {
            println!("[{}] [session] command exited with {status}: {cmd}", timestamp());
        }
        Err(error) => {
            println!("[{}] [session] failed to run command ({error}): {cmd}", timestamp());
        }
        Ok(_) => {}
    }
}

async fn run_shell_commands(commands: &[String]) {
    for cmd in commands {
        println!("[{}] [session] running: {cmd}", timestamp());
        run_shell_command(cmd).await;
    }
}

// Only used by MANUAL close - the watchdog never calls this, because it
// only acts after confirming the process has ALREADY died on its own
// (nothing to kill). Here the game can genuinely still be running, so
// it sends SIGTERM and waits UP TO 20s, but polls every 1s and exits as
// soon as the process dies, instead of always waiting the full 20s like
// Apollo's old undo array used to. Only sends SIGKILL if the whole
// grace period passes without the process exiting on its own.
async fn kill_game_process(app_id: &str) {
    if !is_app_id_running(app_id) {
        println!("[{}] [session] app_id={app_id} is already not running, nothing to kill", timestamp());
        return;
    }

    println!("[{}] [session] sending SIGTERM (AppId={app_id})", timestamp());
    run_shell_command(&format!("pkill -TERM -f AppId={app_id}")).await;

    let grace_period = Duration::from_secs(20);
    let start = tokio::time::Instant::now();
    while is_app_id_running(app_id) {
        if start.elapsed() >= grace_period {
            println!(
                "[{}] [session] app_id={app_id} did not exit on its own within the grace period, forcing with SIGKILL",
                timestamp()
            );
            run_shell_command(&format!("pkill -KILL -f AppId={app_id}")).await;
            return;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    println!("[{}] [session] app_id={app_id} exited on its own after SIGTERM", timestamp());
}

// Registers the session - runs the display_commands SYNCHRONOUSLY
// before responding (which is why runner.py only proceeds to exec
// Moonlight after this call returns: guarantees the screen is already
// in the right state by the time the stream starts).
pub async fn register_session(
    Extension(state): Extension<SessionState>,
    Json(req): Json<RegisterSessionRequest>,
) -> Json<CloseResponse> {
    println!("[{}] [session] registering app_id={} - turning on the screen...", timestamp(), req.app_id);
    run_shell_commands(&req.display_commands).await;
    println!("[{}] [session] screen on, session registered: app_id={}", timestamp(), req.app_id);

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

// IMMEDIATE close (manual, "Close connection") - notifies Apollo FIRST
// (drops the connection/stream on the Deck right away) and only then
// kills the game (if still alive, SIGTERM + adaptive wait + SIGKILL,
// see kill_game_process) and restores the screen, in the background,
// without blocking the response.
pub async fn close_session_now(
    Extension(state): Extension<SessionState>,
    Extension(ApolloBaseUrl(base_url)): Extension<ApolloBaseUrl>,
    Extension(notifier): Extension<EventNotifier>,
) -> Json<CloseResponse> {
    let session = state.lock().await.clone();

    let Some(session) = session else {
        println!("[{}] [session] manual close requested, but no session is registered", timestamp());
        return Json(CloseResponse {
            ok: false,
            error: Some("No session registered in the Runner".to_string()),
        });
    };

    println!(
        "[{}] [session] manual close requested - notifying Apollo now (kill/restore run afterwards, in the background)",
        timestamp()
    );
    match apollo::close_session_at(&base_url, &session.username, &session.password).await {
        Ok(()) => {
            println!("[{}] [session] Apollo closed the session successfully (manual close)", timestamp());
            *state.lock().await = None;
            let _ = notifier.send(RunnerEvent::SessionClosed);

            // The Deck has already received the confirmation (the
            // stream has already been dropped) - killing the game (if
            // still alive) and restoring the screen don't need to block
            // this response.
            let app_id = session.app_id.clone();
            let restore_commands = session.restore_commands.clone();
            tokio::spawn(async move {
                kill_game_process(&app_id).await;
                run_shell_commands(&restore_commands).await;
            });

            Json(CloseResponse { ok: true, error: None })
        }
        Err(error) => {
            println!("[{}] [session] Apollo failed to close (manual close): {error}", timestamp());
            Json(CloseResponse { ok: false, error: Some(error) })
        }
    }
}

// Runs forever in a background task (see lib.rs) - every interval,
// checks whether the registered session is actually still running
// (sysinfo, the OS doesn't lie even when Apollo does). If it died,
// notifies Apollo IMMEDIATELY (drops the connection on the Deck right
// away, there's no prep-cmd at all on Apollo to wait for) and only then
// restores the screen, in the background, without blocking this step.
// Extracted as a separate function (not just an inline loop) so a
// single pass can be tested without needing real sleeps/timing in the
// test.
//
// If Apollo responds with an error (e.g. wrong credentials, or the host
// being momentarily down) at the final close step, the session stays
// registered and the next pass tries again, it doesn't give up after N
// attempts. Acceptable for the current scope (home LAN, no malicious
// login-exhaustion attempts); if this ever becomes a real problem, add
// a limit here. Retrying only the close step (not the restore_commands
// again) avoids running kscreen-doctor twice because of a momentary
// failure only on Apollo's end.
pub async fn check_and_maybe_close_session(state: &SessionState, base_url: &str, notifier: &EventNotifier) -> bool {
    let session = state.lock().await.clone();

    let Some(session) = session else {
        return false;
    };

    let running = is_app_id_running(&session.app_id);
    println!(
        "[{}] [session] watchdog: checking app_id={}, running={running}, previously_confirmed={}",
        timestamp(),
        session.app_id,
        session.confirmed_running
    );

    if running {
        if !session.confirmed_running {
            // first time we've actually seen the process - mark it
            // before returning, so the next pass already knows that a
            // drop now is a real close, not just the game still loading.
            if let Some(s) = state.lock().await.as_mut() {
                s.confirmed_running = true;
            }
        }
        return false;
    }

    if !session.confirmed_running {
        // Real bug found on-device: the watchdog's first check can
        // happen BEFORE the game finishes opening (Steam takes a
        // variable amount of time to spawn the process with "AppId=" in
        // its cmdline, Proton, shader cache, etc). Without this check,
        // "hasn't shown up yet" and "already closed" are
        // indistinguishable, the watchdog would close (and tear down
        // the screen for) a game that was just loading. Only considers
        // it "closed" after having seen it actually running at least
        // once.
        println!(
            "[{}] [session] watchdog: app_id={} has not been seen running yet (game loading?), not closing yet",
            timestamp(),
            session.app_id
        );
        return false;
    }

    println!(
        "[{}] [session] watchdog: app_id={} is no longer running, notifying Apollo now (restore runs afterwards, in the background)",
        timestamp(),
        session.app_id
    );

    match apollo::close_session_at(base_url, &session.username, &session.password).await {
        Ok(()) => {
            println!("[{}] [session] watchdog: Apollo closed the session successfully", timestamp());
            *state.lock().await = None;
            let _ = notifier.send(RunnerEvent::SessionClosed);

            // The Deck has already received the disconnect - restoring
            // the screen doesn't need to block this, it runs
            // independently in the background.
            let restore_commands = session.restore_commands.clone();
            tokio::spawn(async move {
                run_shell_commands(&restore_commands).await;
            });

            true
        }
        Err(error) => {
            println!(
                "[{}] [session] watchdog: Apollo failed to close ({error}), trying again next tick",
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
