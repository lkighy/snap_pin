use std::{fs, path::PathBuf};

use tauri::{AppHandle, Manager};

use crate::settings::dto::AppSettingsDto;

const SETTINGS_FILE: &str = "settings.json";

pub fn load(app: &AppHandle) -> Option<AppSettingsDto> {
    let path = settings_path(app).ok()?;
    log::info!("loading settings from {}", path.display());
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) => {
            log::info!("settings file not loaded from {}: {error}", path.display());
            return None;
        }
    };

    match serde_json::from_str(&contents) {
        Ok(settings) => {
            log::info!("settings loaded from {}", path.display());
            Some(settings)
        }
        Err(error) => {
            log::error!("failed to parse settings from {}: {error}", path.display());
            None
        }
    }
}

pub fn save(app: &AppHandle, settings: &AppSettingsDto) -> Result<(), String> {
    let path = settings_path(app).map_err(|error| error.to_string())?;
    log::info!("saving settings to {}", path.display());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let contents = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    fs::write(&path, contents).map_err(|error| error.to_string())?;
    log::info!("settings saved to {}", path.display());
    Ok(())
}

fn settings_path(app: &AppHandle) -> tauri::Result<PathBuf> {
    Ok(app.path().app_config_dir()?.join(SETTINGS_FILE))
}
