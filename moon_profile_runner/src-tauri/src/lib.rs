mod apollo;
mod displays;
mod games;
mod server;
mod session;
#[cfg(test)]
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
            // Servidor HTTP + watchdog de sessao rodam numa thread + runtime
            // tokio proprias, separado do event loop do Tauri (que fica no
            // thread principal cuidando de tray/janela). Sem token/pareamento -
            // servidor aberto na rede local (decisao explicita: LAN
            // domestica ja e' confiavel o suficiente, e colar token na
            // config do Deck toda vez e' atrito sem ganho real aqui).
            //
            // Os handlers axum (server.rs/session.rs) nao conhecem
            // Tauri/AppHandle - so' mandam um evento nesse canal quando algo
            // acontece (sincronia de jogos, sessao fechada sozinha). Quem
            // escuta e dispara a notificacao de desktop e' esta task, que
            // roda na MESMA runtime do servidor mas tem o AppHandle de
            // verdade (clonado do _app do setup()).
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
                        .expect("falha ao criar a runtime tokio do servidor");
                    rt.spawn(async move {
                        while let Some(event) = notify_rx.recv().await {
                            let (title, body) = match event {
                                RunnerEvent::GamesSynced => {
                                    ("MoonProfile Runner", "Sincronizando jogos com o Deck...")
                                }
                                RunnerEvent::SessionClosed => {
                                    ("MoonProfile Runner", "Sessao encerrada automaticamente (jogo fechado)")
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

            let show_i = MenuItem::with_id(_app, "show", "Abrir MoonProfile Runner", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(_app, "quit", "Sair", true, None::<&str>)?;
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
