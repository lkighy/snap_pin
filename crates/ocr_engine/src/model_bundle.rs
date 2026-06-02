use std::path::{Path, PathBuf};

use shared_models::{ModelDomain, ModelFile, ModelManifest, ModelSource};

use crate::OcrEngineError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrModelBundle {
    pub manifest: ModelManifest,
    pub det: PathBuf,
    pub rec: PathBuf,
    pub keys: PathBuf,
    pub cls: Option<PathBuf>,
}

impl OcrModelBundle {
    pub fn from_manifest(manifest: &ModelManifest) -> Result<Self, OcrEngineError> {
        validate_ocr_manifest(manifest)?;

        let root = match &manifest.source {
            ModelSource::LocalPath(path) => Some(PathBuf::from(path)),
            ModelSource::BuiltIn | ModelSource::Download { .. } => None,
        };

        let det = resolve_required_file(manifest, root.as_deref(), "det")?;
        let rec = resolve_required_file(manifest, root.as_deref(), "rec")?;
        let keys = resolve_required_file(manifest, root.as_deref(), "keys")?;
        let cls = resolve_optional_file(manifest, root.as_deref(), "cls")?;

        Ok(Self {
            manifest: manifest.clone(),
            det,
            rec,
            keys,
            cls,
        })
    }
}

pub fn validate_ocr_manifest(manifest: &ModelManifest) -> Result<(), OcrEngineError> {
    if manifest.domain != ModelDomain::Ocr {
        return Err(OcrEngineError::new(
            "invalid_model_domain",
            format!("model '{}' is not an OCR model", manifest.id),
        ));
    }

    require_role(manifest, "det")?;
    require_role(manifest, "rec")?;
    require_role(manifest, "keys")?;
    validate_backend_file_extensions(manifest)?;
    validate_dictionary_version(manifest)?;

    if let ModelSource::LocalPath(root) = &manifest.source {
        let root = Path::new(root);
        for file in manifest.files.iter().filter(|file| file.required) {
            let path = root.join(&file.path);
            if !path.exists() {
                return Err(OcrEngineError::new(
                    "model_file_missing",
                    format!(
                        "required OCR model file '{}' is missing for model '{}'",
                        file.role, manifest.id
                    ),
                ));
            }
        }
    }

    Ok(())
}

fn require_role<'a>(
    manifest: &'a ModelManifest,
    role: &str,
) -> Result<&'a ModelFile, OcrEngineError> {
    manifest
        .files
        .iter()
        .find(|file| file.role == role && file.required)
        .ok_or_else(|| {
            OcrEngineError::new(
                "model_file_missing",
                format!("OCR model '{}' requires a '{}' file", manifest.id, role),
            )
        })
}

fn resolve_required_file(
    manifest: &ModelManifest,
    root: Option<&Path>,
    role: &str,
) -> Result<PathBuf, OcrEngineError> {
    let file = require_role(manifest, role)?;
    Ok(resolve_path(root, &file.path))
}

fn resolve_optional_file(
    manifest: &ModelManifest,
    root: Option<&Path>,
    role: &str,
) -> Result<Option<PathBuf>, OcrEngineError> {
    Ok(manifest
        .files
        .iter()
        .find(|file| file.role == role && !file.path.is_empty())
        .map(|file| resolve_path(root, &file.path)))
}

fn resolve_path(root: Option<&Path>, path: &str) -> PathBuf {
    root.map_or_else(|| PathBuf::from(path), |root| root.join(path))
}

fn validate_backend_file_extensions(manifest: &ModelManifest) -> Result<(), OcrEngineError> {
    let expected_extension = match manifest.backend.as_str() {
        "mnn" => Some("mnn"),
        "onnx" | "onnxruntime" => Some("onnx"),
        "paddle" | "paddleruntime" => Some("pdmodel"),
        _ => None,
    };

    let Some(expected_extension) = expected_extension else {
        return Ok(());
    };

    for role in ["det", "rec", "cls"] {
        let Some(file) = manifest.files.iter().find(|file| file.role == role) else {
            continue;
        };
        if file.path.is_empty() {
            continue;
        }
        if Path::new(&file.path)
            .extension()
            .and_then(|extension| extension.to_str())
            != Some(expected_extension)
        {
            return Err(OcrEngineError::new(
                "model_backend_mismatch",
                format!(
                    "OCR model '{}' uses backend '{}' but file '{}' is not .{}",
                    manifest.id, manifest.backend, role, expected_extension
                ),
            ));
        }
    }

    Ok(())
}

fn validate_dictionary_version(manifest: &ModelManifest) -> Result<(), OcrEngineError> {
    let keys = require_role(manifest, "keys")?;
    if manifest.version == "v5" && !keys.path.contains("v5") {
        return Err(OcrEngineError::new(
            "model_dictionary_mismatch",
            format!(
                "OCR model '{}' uses PP-OCRv5 but dictionary '{}' does not look like v5 keys",
                manifest.id, keys.path
            ),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use shared_models::{ModelDomain, ModelFile, ModelManifest, ModelSource};

    use super::validate_ocr_manifest;

    fn manifest() -> ModelManifest {
        ModelManifest {
            id: "ppocr-v5-mobile-mnn".to_owned(),
            name: "PP-OCRv5 Mobile MNN".to_owned(),
            domain: ModelDomain::Ocr,
            family: "ppocr".to_owned(),
            backend: "mnn".to_owned(),
            version: "v5".to_owned(),
            source_languages: vec!["zh".to_owned(), "en".to_owned()],
            target_languages: Vec::new(),
            quantization: Some("fp16".to_owned()),
            low_spec_friendly: true,
            multilingual: true,
            source: ModelSource::BuiltIn,
            files: vec![
                ModelFile::required("det", "det.mnn"),
                ModelFile::required("rec", "rec.mnn"),
                ModelFile::required("keys", "ppocr_keys_v5.txt"),
            ],
        }
    }

    #[test]
    fn validates_default_ppocr_v5_mnn_manifest() {
        assert!(validate_ocr_manifest(&manifest()).is_ok());
    }

    #[test]
    fn rejects_backend_file_mismatch() {
        let mut manifest = manifest();
        manifest.files[0].path = "det.onnx".to_owned();

        let error = validate_ocr_manifest(&manifest).unwrap_err();

        assert_eq!(error.code, "model_backend_mismatch");
    }

    #[test]
    fn rejects_v5_dictionary_mismatch() {
        let mut manifest = manifest();
        manifest.files[2].path = "ppocr_keys_v4.txt".to_owned();

        let error = validate_ocr_manifest(&manifest).unwrap_err();

        assert_eq!(error.code, "model_dictionary_mismatch");
    }
}
