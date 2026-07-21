mod apollo;
mod autostart;
mod displays;
mod games;
mod power;
mod server;
mod session;
#[cfg(test)]
#[path = "tests/support.rs"]
mod test_support;

use server::{run_server, RunnerEvent};
use session::{watch_sessions, ApolloBaseUrl};
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::{mpsc, Mutex};

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
        .setup(|_app| {
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
                    rt.block_on(run_server(notify_tx, session_state, ApolloBaseUrl(apollo::DEFAULT_APOLLO_BASE_URL.to_string())));
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
