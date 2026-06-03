use std::{
    fs,
    path::{Path, PathBuf},
};

use model_registry::{DEFAULT_OCR_MODELS_DIR, ModelStorage};
use serde::{Deserialize, Serialize};
use shared_models::{ModelDomain, ModelManifest, ModelSource};
use tauri::{AppHandle, Manager};

const MODELS_FILE: &str = "models.json";
const MODEL_STORAGE_FILE: &str = "model_storage.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelStorageConfig {
    ocr_models_dir: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStorageInfo {
    pub default_ocr_models_dir: String,
    pub current_ocr_models_dir: String,
    pub using_default: bool,
}

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
    let root = ocr_models_dir(app)?;
    Ok(ModelStorage::new(root))
}

pub fn storage_info(app: &AppHandle) -> Result<ModelStorageInfo, String> {
    let default_dir = default_ocr_models_dir(app)?;
    let current_dir = ocr_models_dir(app)?;
    Ok(ModelStorageInfo {
        default_ocr_models_dir: path_string(&default_dir),
        current_ocr_models_dir: path_string(&current_dir),
        using_default: same_path(&default_dir, &current_dir),
    })
}

pub fn set_ocr_models_dir(
    app: &AppHandle,
    models: &mut [ModelManifest],
    target_dir: impl AsRef<Path>,
) -> Result<ModelStorageInfo, String> {
    let target_dir = normalize_target_dir(target_dir.as_ref())?;
    let current_dir = ocr_models_dir(app)?;
    if same_path(&current_dir, &target_dir) {
        save_storage_config(app, &target_dir)?;
        return storage_info(app);
    }
    if path_starts_with(&target_dir, &current_dir) {
        return Err(
            "model_storage_invalid_target: new model location must not be inside the current location"
                .to_owned(),
        );
    }

    fs::create_dir_all(&target_dir).map_err(|error| {
        format!(
            "model_storage_create_failed: failed to create '{}': {error}",
            target_dir.display()
        )
    })?;

    for model in models.iter_mut() {
        if model.domain != ModelDomain::Ocr {
            continue;
        }
        let ModelSource::LocalPath(root) = &model.source else {
            continue;
        };
        let old_root = PathBuf::from(root);
        if !old_root.exists() {
            let next_root = target_dir.join(safe_model_id(&model.id));
            model.source = ModelSource::LocalPath(path_string(&next_root));
            continue;
        }

        let next_root = target_dir.join(safe_model_id(&model.id));
        if same_path(&old_root, &next_root) {
            model.source = ModelSource::LocalPath(path_string(&next_root));
            continue;
        }
        if next_root.exists() {
            fs::remove_dir_all(&next_root).map_err(|error| {
                format!(
                    "model_storage_migrate_failed: failed to replace '{}': {error}",
                    next_root.display()
                )
            })?;
        }
        if let Some(parent) = next_root.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "model_storage_create_failed: failed to create '{}': {error}",
                    parent.display()
                )
            })?;
        }

        match fs::rename(&old_root, &next_root) {
            Ok(()) => {}
            Err(_) => {
                copy_dir_all(&old_root, &next_root)?;
                fs::remove_dir_all(&old_root).map_err(|error| {
                    format!(
                        "model_storage_cleanup_failed: failed to remove old model directory '{}': {error}",
                        old_root.display()
                    )
                })?;
            }
        }
        model.source = ModelSource::LocalPath(path_string(&next_root));
    }

    save_storage_config(app, &target_dir)?;
    save(app, models)?;
    storage_info(app)
}

pub(crate) fn models_path(app: &AppHandle) -> tauri::Result<PathBuf> {
    Ok(app.path().app_config_dir()?.join(MODELS_FILE))
}

fn ocr_models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if let Some(config) = load_storage_config(app) {
        let path = PathBuf::from(config.ocr_models_dir.trim());
        if !path.as_os_str().is_empty() {
            return Ok(path);
        }
    }
    default_ocr_models_dir(app)
}

fn default_ocr_models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join(DEFAULT_OCR_MODELS_DIR))
}

fn storage_config_path(app: &AppHandle) -> tauri::Result<PathBuf> {
    Ok(app.path().app_config_dir()?.join(MODEL_STORAGE_FILE))
}

fn load_storage_config(app: &AppHandle) -> Option<ModelStorageConfig> {
    let path = storage_config_path(app).ok()?;
    let contents = fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

fn save_storage_config(app: &AppHandle, ocr_models_dir: &Path) -> Result<(), String> {
    let path = storage_config_path(app).map_err(|error| error.to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let config = ModelStorageConfig {
        ocr_models_dir: path_string(ocr_models_dir),
    };
    let contents = serde_json::to_string_pretty(&config).map_err(|error| error.to_string())?;
    fs::write(path, contents).map_err(|error| error.to_string())
}

fn normalize_target_dir(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("model_storage_invalid_target: model location must not be empty".to_owned());
    }
    Ok(path.to_path_buf())
}

fn copy_dir_all(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|error| {
        format!(
            "model_storage_migrate_failed: failed to create '{}': {error}",
            target.display()
        )
    })?;
    for entry in fs::read_dir(source).map_err(|error| {
        format!(
            "model_storage_migrate_failed: failed to read '{}': {error}",
            source.display()
        )
    })? {
        let entry = entry.map_err(|error| {
            format!("model_storage_migrate_failed: failed to read directory entry: {error}")
        })?;
        let file_type = entry.file_type().map_err(|error| {
            format!("model_storage_migrate_failed: failed to inspect directory entry: {error}")
        })?;
        let next_target = target.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &next_target)?;
        } else {
            fs::copy(entry.path(), &next_target).map_err(|error| {
                format!(
                    "model_storage_migrate_failed: failed to copy '{}' to '{}': {error}",
                    entry.path().display(),
                    next_target.display()
                )
            })?;
        }
    }
    Ok(())
}

fn same_path(left: &Path, right: &Path) -> bool {
    normalize_for_compare(left) == normalize_for_compare(right)
}

fn path_starts_with(path: &Path, parent: &Path) -> bool {
    let path = normalize_for_compare(path);
    let parent = normalize_for_compare(parent);
    path != parent && path.starts_with(parent)
}

fn normalize_for_compare(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn safe_model_id(model_id: &str) -> String {
    model_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
