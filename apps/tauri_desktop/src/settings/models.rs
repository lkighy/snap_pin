use std::{fs, path::PathBuf};

use model_registry::{DEFAULT_OCR_MODELS_DIR, ModelStorage};
use shared_models::{ModelManifest, ModelSource};
use tauri::{AppHandle, Manager};

const MODELS_FILE: &str = "models.json";

pub fn load(app: &AppHandle) -> Vec<ModelManifest> {
    let Ok(path) = models_path(app) else {
        return Vec::new();
    };
    log::info!("loading model registry from {}", path.display());
    let Ok(contents) = fs::read_to_string(&path) else {
        log::info!("model registry not loaded from {}", path.display());
        return Vec::new();
    };

    match serde_json::from_str(&contents) {
        Ok(models) => {
            log::info!("model registry loaded from {}", path.display());
            models
        }
        Err(error) => {
            log::error!(
                "failed to parse model registry from {}: {error}",
                path.display()
            );
            Vec::new()
        }
    }
}

pub fn save(app: &AppHandle, models: &[ModelManifest]) -> Result<(), String> {
    let path = models_path(app).map_err(|error| error.to_string())?;
    log::info!("saving model registry to {}", path.display());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let imported_models = models
        .iter()
        .filter(|model| !matches!(model.source, ModelSource::BuiltIn))
        .cloned()
        .collect::<Vec<_>>();
    let contents =
        serde_json::to_string_pretty(&imported_models).map_err(|error| error.to_string())?;
    fs::write(&path, contents).map_err(|error| error.to_string())?;
    log::info!("model registry saved to {}", path.display());
    Ok(())
}

pub fn storage(app: &AppHandle) -> Result<ModelStorage, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join(DEFAULT_OCR_MODELS_DIR);
    Ok(ModelStorage::new(root))
}

fn models_path(app: &AppHandle) -> tauri::Result<PathBuf> {
    Ok(app.path().app_config_dir()?.join(MODELS_FILE))
}
