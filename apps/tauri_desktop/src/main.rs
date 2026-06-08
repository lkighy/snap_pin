#![allow(dead_code)]

mod capture;
mod commands;
mod ipc;
mod logging;
mod settings;
mod shell_state;
mod tray;

use std::sync::Mutex;
use std::thread;
use std::time::Instant;

use perf_trace::{PerfSpan, log_elapsed};
use shell_state::ShellState;
use tauri::{Manager, WindowEvent};

fn main() {
    let app_start = Instant::now();
    logging::init();
    log_elapsed("desktop_app_start_to_logging_ready", app_start);

    if std::env::args().any(|arg| arg == "--mvp-cli") {
        log::info!("starting desktop shell in mvp cli mode");
        run_cli_mvp();
        return;
    }

    log::info!("starting desktop shell");
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::tauri::app_status,
            commands::tauri::platform_capabilities,
            commands::tauri::get_settings,
            commands::tauri::list_models,
            commands::tauri::model_storage_info,
            commands::tauri::choose_ocr_model_storage_dir,
            commands::tauri::set_ocr_model_storage_dir,
            commands::tauri::open_ocr_model_storage_dir,
            commands::tauri::save_settings,
            commands::tauri::run_mvp_flow,
            commands::tauri::drain_events,
            commands::tauri::import_model,
            commands::tauri::download_builtin_ocr_model,
            commands::tauri::start_builtin_ocr_model_download,
            commands::tauri::model_download_status,
            commands::tauri::cancel_model_download,
            commands::tauri::start_capture
        ])
        .setup(|app| {
            let setup_span = PerfSpan::new("tauri_setup_total");
            log::info!("tauri setup started");
            let app_handle = app.handle().clone();
            let platform_start = Instant::now();
            let platform = platform_runtime::create_platform_arc();
            log_elapsed("tauri_setup_create_platform", platform_start);
            let shell_state_start = Instant::now();
            let state = ShellState::from_store_with_platform(&app_handle, platform);
            log_elapsed("tauri_setup_load_shell_state", shell_state_start);
            app.manage(Mutex::new(state));
            app.manage(Mutex::new(commands::tauri::ModelDownloadRuntime::default()));
            app.manage(Mutex::new(None::<Box<dyn platform_api::HotkeyToken>>));
            app.manage(Mutex::new(None::<capture::launcher::PinHotkeyListener>));
            app.manage(Mutex::new(
                capture::launcher::CaptureOverlayRuntime::default(),
            ));
            let tray_start = Instant::now();
            tray::install(app)?;
            log_elapsed("tauri_setup_install_tray", tray_start);
            let hide_window_start = Instant::now();
            if let Some(window) = app.get_webview_window("main") {
                if let Err(error) = window.hide() {
                    log::error!("failed to hide main window during startup: {error}");
                } else {
                    log::info!("main window hidden during startup");
                }
            } else {
                log::error!("main window not found during startup");
            }
            log_elapsed("tauri_setup_hide_main_window", hide_window_start);
            let hotkey_start = Instant::now();
            if let Err(error) = capture::launcher::register_global_hotkeys(&app_handle) {
                log::error!("failed to register global hotkeys: {error}");
            }
            log_elapsed("tauri_setup_register_global_hotkeys", hotkey_start);
            let overlay_start = Instant::now();
            warm_up_overlay_resident(app_handle.clone());
            log_elapsed("tauri_setup_schedule_overlay_warmup", overlay_start);
            log::info!("tauri setup completed");
            setup_span.finish();
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

fn warm_up_overlay_resident(app_handle: tauri::AppHandle) {
    thread::spawn(move || {
        let span = PerfSpan::new("overlay_resident_background_warmup");
        log::info!("resident overlay background warm-up started");
        if let Err(error) = capture::launcher::ensure_overlay_resident(&app_handle) {
            log::error!("resident overlay background warm-up failed: {error}");
        } else {
            log::info!("resident overlay background warm-up completed");
        }
        span.finish();
    });
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
