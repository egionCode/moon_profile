mod apollo;
mod autostart;
mod clients;
mod displays;
mod games;
mod ping;
mod power;
mod server;
mod session;
#[cfg(test)]
#[path = "tests/support.rs"]
mod test_support;

use clients::{ClientsFilePath, ClientsState, KnownClient};
use server::{run_server, RunnerEvent};
use session::{watch_sessions, ApolloBaseUrl};
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, State, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::{mpsc, Mutex};

// "Clients" section of the Runner's own window (main.js): the list of
// Decks that have ever connected (see clients.rs), sorted for a stable
// display order.
// Tauri requires async commands that borrow an argument (State<'_, ..>
// here) to return a Result - this one never actually fails, Ok is the
// only variant ever produced.
#[tauri::command]
async fn list_known_clients(clients_state: State<'_, ClientsState>) -> Result<Vec<KnownClient>, ()> {
    let guard = clients_state.0.lock().await;
    let mut list: Vec<KnownClient> = guard.values().cloned().collect();
    list.sort_by(|a, b| a.ip.cmp(&b.ip));
    Ok(list)
}

// Called from the window's own 3s polling loop (main.js), NOT from the
// always-on HTTP server side - so an unreachable Deck only ever slows
// down this window, never anything the Deck itself is waiting on. The
// outer timeout guards spawn_blocking (which runs the real `ping`
// subprocess) against hanging longer than the -W 1 flag alone would
// suggest, e.g. DNS resolution weirdness.
#[tauri::command]
async fn ping_client(ip: String) -> Option<f64> {
    match tokio::time::timeout(ping::PING_COMMAND_TIMEOUT, tokio::task::spawn_blocking(move || ping::ping_once(&ip))).await {
        Ok(Ok(latency)) => latency,
        _ => None,
    }
}

fn open_or_focus_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("MoonProfile Runner")
        .inner_size(480.0, 420.0)
        .build();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![list_known_clients, ping_client])
        .setup(|_app| {
            // Loaded once at startup (see clients.rs) so Decks seen on a
            // previous run still show up (as unreachable) even before
            // they reconnect - persisted under the app's own data dir,
            // resolved here because this is where the real AppHandle
            // lives; clients.rs itself only ever touches a plain Path,
            // same decoupling as session_state/notify below.
            let clients_dir = _app.path().app_data_dir().expect("failed to resolve the app data dir");
            std::fs::create_dir_all(&clients_dir).expect("failed to create the app data dir");
            let clients_file_path = clients_dir.join("clients.json");
            let clients_state = ClientsState(Arc::new(Mutex::new(clients::load_known_clients(&clients_file_path))));
            _app.manage(clients_state.clone());

            // Best-effort: register ourselves for autostart on future logins
            // the first time someone runs the app (see autostart.rs) - the
            // AUR package can't do this from its own install scriptlet
            // (pacman runs as root, not the logged-in user's systemd --user
            // instance), but we're already inside the right session here.
            // Spawned on its own thread (not called inline) so a slow or
            // hung `systemctl --user` (e.g. session D-Bus not fully up yet
            // right after login) can never delay the tray icon or HTTP
            // server from appearing - this is pure bookkeeping, nothing
            // else depends on it finishing first.
            std::thread::spawn(autostart::ensure_enabled);

            // The HTTP server + session watchdog run on their own thread +
            // tokio runtime, separate from Tauri's event loop (which stays
            // on the main thread handling the tray/window). No token/pairing
            // - server open on the local network (explicit decision: a home
            // LAN is already trustworthy enough, and pasting a token into
            // the Deck's config every time is friction with no real gain
            // here).
            //
            // The axum handlers (server.rs/session.rs) don't know about
            // Tauri/AppHandle at all - they just send an event on this
            // channel when something happens (games synced, session closed
            // on its own). This task is the one listening and firing the
            // desktop notification, running on the SAME runtime as the
            // server but with a real AppHandle (cloned from setup()'s
            // _app).
            let notification_handle = _app.handle().clone();
            let (notify_tx, mut notify_rx) = mpsc::unbounded_channel();
            let session_state = Arc::new(Mutex::new(None));

            std::thread::spawn({
                let notify_tx = notify_tx.clone();
                let session_state = session_state.clone();
                let clients_state = clients_state.clone();
                let clients_file_path = ClientsFilePath(clients_file_path.clone());
                move || {
                    let rt = tokio::runtime::Builder::new_multi_thread()
                        .enable_all()
                        .build()
                        .expect("failed to create the server's tokio runtime");
                    rt.spawn(async move {
                        while let Some(event) = notify_rx.recv().await {
                            let (title, body) = match event {
                                RunnerEvent::GamesSynced => {
                                    ("MoonProfile Runner", "Syncing games with the Deck...")
                                }
                                RunnerEvent::SessionClosed => {
                                    ("MoonProfile Runner", "Session closed automatically (game exited)")
                                }
                            };
                            let _ = notification_handle.notification().builder().title(title).body(body).show();
                        }
                    });
                    rt.spawn(watch_sessions(
                        session_state.clone(),
                        apollo::DEFAULT_APOLLO_BASE_URL.to_string(),
                        notify_tx.clone(),
                    ));
                    rt.block_on(run_server(
                        notify_tx,
                        session_state,
                        ApolloBaseUrl(apollo::DEFAULT_APOLLO_BASE_URL.to_string()),
                        clients_state,
                        clients_file_path,
                    ));
                }
            });

            let show_i = MenuItem::with_id(_app, "show", "Open MoonProfile Runner", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(_app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(_app, &[&show_i, &quit_i])?;

            TrayIconBuilder::new()
                .icon(_app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "show" => open_or_focus_window(app),
                    _ => {}
                })
                .build(_app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
