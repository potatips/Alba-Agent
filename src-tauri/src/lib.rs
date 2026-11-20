pub mod commands;
pub mod service;

use std::path::PathBuf;
use std::sync::Arc;

use tauri::Manager;

use crate::commands::{
    approve_action, build_tauri_event_callback, fork_trajectory, get_audit_logs, get_branches,
    get_checkpoints, get_default_workspace, list_workspace_files, read_workspace_file, resume_run,
    start_run, DesktopAppState,
};
use crate::service::DesktopEngineService;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = std::env::var("ALBA_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    app.path()
                        .app_data_dir()
                        .unwrap_or_else(|_| PathBuf::from("."))
                });
            std::fs::create_dir_all(&app_data_dir).ok();

            let sock_path = app_data_dir.join("alba.sock");
            #[cfg(unix)]
            {
                use std::io::{Read, Write};
                use std::os::unix::net::{UnixListener, UnixStream};
                match UnixListener::bind(&sock_path) {
                    Ok(listener) => {
                        let handle = app.handle().clone();
                        std::thread::spawn(move || {
                            for mut stream in listener.incoming().flatten() {
                                let mut buf = [0u8; 16];
                                if let Ok(n) = stream.read(&mut buf) {
                                    if &buf[..n] == b"focus\n" || &buf[..n] == b"focus" {
                                        if let Some(w) = handle.get_webview_window("main") {
                                            let _ = w.show();
                                            let _ = w.set_focus();
                                        }
                                    }
                                }
                            }
                        });
                    }
                    Err(_) => {
                        if let Ok(mut stream) = UnixStream::connect(&sock_path) {
                            let _ = stream.write_all(b"focus\n");
                            std::process::exit(0);
                        } else {
                            let _ = std::fs::remove_file(&sock_path);
                            if let Ok(listener) = UnixListener::bind(&sock_path) {
                                let handle = app.handle().clone();
                                std::thread::spawn(move || {
                                    for mut stream in listener.incoming().flatten() {
                                        let mut buf = [0u8; 16];
                                        if let Ok(n) = stream.read(&mut buf) {
                                            if &buf[..n] == b"focus\n" || &buf[..n] == b"focus" {
                                                if let Some(w) = handle.get_webview_window("main") {
                                                    let _ = w.show();
                                                    let _ = w.set_focus();
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
            }

            let db_path = app_data_dir.join("checkpoints.db");
            let store = Arc::new(
                nexus_albaai::checkpoints::SqliteCheckpointStore::new(&db_path)
                    .expect("failed to open SQLite store"),
            );
            let workspace_root = app_data_dir.join("workspace");
            std::fs::create_dir_all(&workspace_root).ok();

            let static_dir = std::env::var("ALBA_STATIC_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    if let Ok(res_out) = app.path().resource_dir().map(|r| r.join("out")) {
                        if res_out.exists() {
                            return res_out;
                        }
                    }
                    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                    let candidate = manifest.join("../web/out");
                    if candidate.exists() {
                        candidate
                    } else {
                        app.path()
                            .resource_dir()
                            .map(|r| r.join("out"))
                            .unwrap_or(candidate)
                    }
                });

            let server_state =
                nexus_albaai::server::state::AppState::new(store.clone(), workspace_root.clone())
                    .with_credentials(Arc::new(
                        nexus_albaai::credentials::MacosKeychainStore::new("alba"),
                    ))
                    .with_static_dir(static_dir)
                    .with_allow_local_auth(true);

            let bind_addr = std::env::var("ALBA_BIND_PORT")
                .map(|p| format!("127.0.0.1:{}", p))
                .unwrap_or_else(|_| "127.0.0.1:0".to_string());

            let listener = tauri::async_runtime::block_on(async {
                tokio::net::TcpListener::bind(&bind_addr).await
            })
            .expect("failed to bind loopback");

            let port = listener.local_addr().expect("local addr").port();
            std::fs::write(app_data_dir.join("backend_port"), port.to_string()).ok();
            println!("Alba backend ready on http://127.0.0.1:{}", port);

            tauri::async_runtime::spawn(async move {
                let _ = nexus_albaai::server::serve_app(server_state, listener).await;
            });

            let callback = build_tauri_event_callback(app.handle().clone());
            let service = Arc::new(DesktopEngineService::new(Some(callback)));
            app.manage(DesktopAppState { service });

            if let Some(window) = app.get_webview_window("main") {
                let target_url = format!("http://127.0.0.1:{}", port);
                if let Ok(parsed) = target_url.parse() {
                    let _ = window.navigate(parsed);
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_default_workspace,
            list_workspace_files,
            read_workspace_file,
            start_run,
            resume_run,
            approve_action,
            get_checkpoints,
            get_branches,
            get_audit_logs,
            fork_trajectory,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Alba Agent desktop application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit = event {
                if let Ok(dir) = app_handle.path().app_data_dir() {
                    let _ = std::fs::remove_file(dir.join("alba.sock"));
                    let _ = std::fs::remove_file(dir.join("backend_port"));
                }
            }
        });
}
