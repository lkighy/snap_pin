use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, State};

use super::mvp;
use crate::capture::launcher;
use crate::settings::dto::AppSettingsDto;
use crate::settings::store;
use crate::shell_state::ShellState;
use crate::tray;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub boot_summary: String,
    pub model_summary: String,
    pub history_summary: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MvpRunResponse {
    pub events: Vec<String>,
    pub history_summary: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureResponse {
    pub events: Vec<String>,
}

#[tauri::command]
pub fn app_status(state: State<'_, Mutex<ShellState>>) -> Result<AppStatus, String> {
    log::info!("tauri command app_status started");
    let state = state.lock().map_err(|_| "shell state lock poisoned")?;

    let status = AppStatus {
        boot_summary: state.boot_summary(),
        model_summary: state.model_summary(),
        history_summary: state.history_summary(),
    };
    log::info!("tauri command app_status completed");
    Ok(status)
}

#[tauri::command]
pub fn get_settings(state: State<'_, Mutex<ShellState>>) -> Result<AppSettingsDto, String> {
    log::info!("tauri command get_settings started");
    let state = state.lock().map_err(|_| "shell state lock poisoned")?;
    let settings = AppSettingsDto::from(state.settings());
    log::info!("tauri command get_settings completed");
    Ok(settings)
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<'_, Mutex<ShellState>>,
    settings: AppSettingsDto,
) -> Result<AppSettingsDto, String> {
    log::info!(
        "tauri command save_settings started language={} capture_hotkey={}",
        settings.interface.language,
        settings.hotkeys.capture
    );
    let settings = shared_models::Settings::from(settings);
    let saved = {
        let mut state = state.lock().map_err(|_| "shell state lock poisoned")?;
        state.update_settings(settings.clone());
        AppSettingsDto::from(state.settings())
    };

    store::save(&app, &saved)?;
    tray::refresh(&app, &saved.interface.language).map_err(|error| error.to_string())?;
    launcher::register_capture_hotkey_for_settings(&app, &settings)?;
    if let Err(error) = launcher::ensure_overlay_resident(&app) {
        log::error!("failed to ensure resident overlay after settings save: {error}");
    }
    log::info!("tauri command save_settings completed");
    Ok(saved)
}

#[tauri::command]
pub fn run_mvp_flow(state: State<'_, Mutex<ShellState>>) -> Result<MvpRunResponse, String> {
    log::info!("tauri command run_mvp_flow started");
    let mut state = state.lock().map_err(|_| "shell state lock poisoned")?;
    let events = mvp::run_mvp_capture_ocr_translate(&mut state)
        .into_iter()
        .map(|event| format!("{event:?}"))
        .collect();

    log::info!("tauri command run_mvp_flow completed");
    Ok(MvpRunResponse {
        events,
        history_summary: state.history_summary(),
    })
}

#[tauri::command]
pub fn start_capture(
    app: AppHandle,
    state: State<'_, Mutex<ShellState>>,
) -> Result<CaptureResponse, String> {
    log::info!("tauri command start_capture started");
    let mut state = state.lock().map_err(|_| "shell state lock poisoned")?;
    let events = mvp::start_capture(&mut state)
        .into_iter()
        .map(|event| format!("{event:?}"))
        .collect();
    drop(state);

    launcher::launch_capture_overlay(&app)?;

    log::info!("tauri command start_capture completed");
    Ok(CaptureResponse { events })
}
