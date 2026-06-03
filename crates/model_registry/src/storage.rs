use std::fs;
use std::path::{Component, Path, PathBuf};

use shared_models::{ModelManifest, ModelSource};

use crate::{ModelImportError, import_manifest_file};

pub const DEFAULT_OCR_MODELS_DIR: &str = "models/ocr";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelStorage {
    root: PathBuf,
}

impl ModelStorage {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn import_manifest_file(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<ModelManifest, ModelImportError> {
        let path = path.as_ref();
        let source_root = path.parent().unwrap_or_else(|| Path::new("."));
        let mut manifest = import_manifest_file(path)?;
        let model_dir = self.model_dir(&manifest.id);

        fs::create_dir_all(&model_dir).map_err(|error| {
            ModelImportError::new(
                "model_storage_create_failed",
                format!(
                    "failed to create model storage directory '{}': {error}",
                    model_dir.display()
                ),
            )
        })?;

        copy_manifest(path, &model_dir)?;
        for file in manifest
            .files
            .iter()
            .filter(|file| file.required || !file.path.trim().is_empty())
        {
            let relative_path = safe_relative_path(&file.path)?;
            copy_model_file(source_root, &model_dir, &relative_path)?;
        }

        manifest.source = ModelSource::LocalPath(model_dir.to_string_lossy().into_owned());
        Ok(manifest)
    }

    pub(crate) fn model_dir(&self, model_id: &str) -> PathBuf {
        self.root.join(safe_model_id(model_id))
    }
}

fn copy_manifest(path: &Path, model_dir: &Path) -> Result<(), ModelImportError> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("manifest.toml");
    let target = model_dir.join(file_name);
    fs::copy(path, &target).map_err(|error| {
        ModelImportError::new(
            "model_manifest_copy_failed",
            format!(
                "failed to copy model manifest to '{}': {error}",
                target.display()
            ),
        )
    })?;
    Ok(())
}

fn copy_model_file(
    source_root: &Path,
    model_dir: &Path,
    relative_path: &Path,
) -> Result<(), ModelImportError> {
    let source = source_root.join(relative_path);
    let target = model_dir.join(relative_path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ModelImportError::new(
                "model_storage_create_failed",
                format!(
                    "failed to create model storage directory '{}': {error}",
                    parent.display()
                ),
            )
        })?;
    }

    fs::copy(&source, &target).map_err(|error| {
        ModelImportError::new(
            "model_file_copy_failed",
            format!(
                "failed to copy model file '{}' to '{}': {error}",
                source.display(),
                target.display()
            ),
        )
    })?;
    Ok(())
}

fn safe_relative_path(path: &str) -> Result<PathBuf, ModelImportError> {
    let path = Path::new(path);
    if path.as_os_str().is_empty() {
        return Err(ModelImportError::new(
            "model_file_path_invalid",
            "model file path must not be empty",
        ));
    }

    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => safe.push(part),
            _ => {
                return Err(ModelImportError::new(
                    "model_file_path_invalid",
                    "model file paths must be relative and stay inside the model package",
                ));
            }
        }
    }
    Ok(safe)
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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{DEFAULT_OCR_MODELS_DIR, ModelStorage};

    #[test]
    fn default_ocr_models_dir_is_stable() {
        assert_eq!(DEFAULT_OCR_MODELS_DIR, "models/ocr");
    }

    #[test]
    fn imports_manifest_into_storage_root() {
        let root = std::env::temp_dir().join(format!(
            "snap-pin-model-storage-source-{}",
            std::process::id()
        ));
        let storage_root = std::env::temp_dir().join(format!(
            "snap-pin-model-storage-target-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("det.mnn"), [1]).unwrap();
        fs::write(root.join("rec.mnn"), [2]).unwrap();
        fs::write(root.join("ppocr_keys_v5.txt"), [3]).unwrap();
        fs::write(
            root.join("manifest.toml"),
            r#"
id = "ppocr-v5-mobile-mnn"
name = "PP-OCRv5 Mobile MNN"
family = "ppocr"
version = "v5"
backend = "mnn"
precision = "fp16"
language = ["zh", "en"]

[files]
det = "det.mnn"
rec = "rec.mnn"
keys = "ppocr_keys_v5.txt"
"#,
        )
        .unwrap();

        let storage = ModelStorage::new(&storage_root);
        let manifest = storage
            .import_manifest_file(root.join("manifest.toml"))
            .unwrap();

        let model_root = storage_root.join("ppocr-v5-mobile-mnn");
        assert!(model_root.join("manifest.toml").exists());
        assert!(model_root.join("det.mnn").exists());
        assert!(model_root.join("rec.mnn").exists());
        assert!(model_root.join("ppocr_keys_v5.txt").exists());
        assert_eq!(
            manifest.source,
            shared_models::ModelSource::LocalPath(model_root.to_string_lossy().into_owned())
        );

        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(storage_root);
    }
}
