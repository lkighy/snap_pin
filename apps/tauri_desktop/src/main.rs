#![allow(dead_code)]

mod capture_launcher;
mod commands;
mod ipc_bridge;
mod logging;
mod settings_dto;
mod settings_store;
mod shell_state;
mod tauri_commands;
mod tray;

use std::sync::Mutex;

use shell_state::ShellState;
use tauri::{Manager, WindowEvent};

fn main() {
    logging::init();

    if std::env::args().any(|arg| arg == "--mvp-cli") {
        log::info!("starting desktop shell in mvp cli mode");
        run_cli_mvp();
        return;
    }

    log::info!("starting desktop shell");
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            tauri_commands::app_status,
            tauri_commands::get_settings,
            tauri_commands::save_settings,
            tauri_commands::run_mvp_flow,
            tauri_commands::start_capture
        ])
        .setup(|app| {
            log::info!("tauri setup started");
            let app_handle = app.handle().clone();
            let state = ShellState::from_store(&app_handle);
            app.manage(Mutex::new(state));
            app.manage(Mutex::new(None::<platform_win32::HotkeyListener>));
            app.manage(Mutex::new(
                capture_launcher::CaptureOverlayRuntime::default(),
            ));
            tray::install(app)?;
            if let Some(window) = app.get_webview_window("main") {
                if let Err(error) = window.hide() {
                    log::error!("failed to hide main window during startup: {error}");
                } else {
                    log::info!("main window hidden during startup");
                }
            } else {
                log::error!("main window not found during startup");
            }
            if let Err(error) = capture_launcher::register_capture_hotkey(&app_handle) {
                log::error!("failed to register capture hotkey: {error}");
            }
            if let Err(error) = capture_launcher::ensure_overlay_resident(&app_handle) {
                log::error!("failed to ensure resident overlay: {error}");
            }
            log::info!("tauri setup completed");
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    log::info!("main window close requested; hiding window");
                    api.prevent_close();
                    if let Err(error) = window.hide() {
                        log::error!("failed to hide main window: {error}");
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run snap pin tauri application");
}

fn run_cli_mvp() {
    let mut state = ShellState::default();
    log::info!("mvp cli state initialized");
    println!("{}", state.boot_summary());
    println!("{}", state.model_summary());

    for event in commands::run_mvp_capture_ocr_translate(&mut state) {
        log::info!("mvp cli event: {event:?}");
        println!("event: {event:?}");
    }

    println!("{}", state.history_summary());
    log::info!("mvp cli finished");
}
