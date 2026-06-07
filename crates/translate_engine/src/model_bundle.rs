use std::path::{Path, PathBuf};

use shared_models::{ModelDomain, ModelFile, ModelManifest, ModelSource};

use crate::TranslateEngineError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationModelBundle {
    pub manifest: ModelManifest,
    pub model: PathBuf,
    pub config: PathBuf,
    pub source_tokenizer: PathBuf,
    pub target_tokenizer: PathBuf,
    pub vocabulary: Option<PathBuf>,
}

impl TranslationModelBundle {
    pub fn from_manifest(manifest: &ModelManifest) -> Result<Self, TranslateEngineError> {
        validate_translation_manifest(manifest)?;

        let root = match &manifest.source {
            ModelSource::LocalPath(path) => PathBuf::from(path),
            ModelSource::BuiltIn => {
                return Err(TranslateEngineError::new(
                    "translation_model_not_installed",
                    format!(
                        "translation model '{}' is a built-in manifest only; download or import the local CTranslate2 model files first",
                        manifest.id
                    ),
                ));
            }
            ModelSource::Download { url, .. } => {
                return Err(TranslateEngineError::new(
                    "translation_model_not_installed",
                    format!(
                        "translation model '{}' has not been downloaded from '{}'",
                        manifest.id, url
                    ),
                ));
            }
        };

        let model = resolve_required_file(manifest, &root, "model")?;
        let config = resolve_required_file(manifest, &root, "config")?;
        let source_tokenizer = resolve_required_file(manifest, &root, "source_tokenizer")?;
        let target_tokenizer = resolve_required_file(manifest, &root, "target_tokenizer")?;
        let vocabulary = resolve_optional_file(manifest, &root, "vocabulary")?;

        Ok(Self {
            manifest: manifest.clone(),
            model,
            config,
            source_tokenizer,
            target_tokenizer,
            vocabulary,
        })
    }

    pub fn supports_language_pair(
        &self,
        source_language: Option<&str>,
        target_language: &str,
    ) -> bool {
        self.manifest
            .supports_language_pair(source_language, target_language)
    }

    pub fn default_source_language(&self) -> Option<&str> {
        single_non_empty_language(&self.manifest.source_languages)
    }
}

fn single_non_empty_language(languages: &[String]) -> Option<&str> {
    let mut values = languages
        .iter()
        .map(|language| language.trim())
        .filter(|language| !language.is_empty());
    let first = values.next()?;
    values.next().is_none().then_some(first)
}

pub fn validate_translation_manifest(manifest: &ModelManifest) -> Result<(), TranslateEngineError> {
    if manifest.domain != ModelDomain::Translation {
        return Err(TranslateEngineError::new(
            "invalid_model_domain",
            format!("model '{}' is not a translation model", manifest.id),
        ));
    }

    if manifest.backend != "ctranslate2" {
        return Err(TranslateEngineError::new(
            "model_backend_mismatch",
            format!(
                "translation model '{}' uses backend '{}' but the local MVP requires 'ctranslate2'",
                manifest.id, manifest.backend
            ),
        ));
    }

    require_role(manifest, "model")?;
    require_role(manifest, "config")?;
    require_role(manifest, "source_tokenizer")?;
    require_role(manifest, "target_tokenizer")?;
    validate_required_local_files(manifest)?;

    Ok(())
}

fn validate_required_local_files(manifest: &ModelManifest) -> Result<(), TranslateEngineError> {
    let ModelSource::LocalPath(root) = &manifest.source else {
        return Ok(());
    };
    let root = Path::new(root);

    for file in manifest.files.iter().filter(|file| file.required) {
        let path = root.join(&file.path);
        if !path.exists() {
            return Err(TranslateEngineError::new(
                "model_file_missing",
                format!(
                    "required translation model file '{}' for role '{}' is missing",
                    path.display(),
                    file.role
                ),
            ));
        }
    }

    Ok(())
}

fn require_role<'a>(
    manifest: &'a ModelManifest,
    role: &str,
) -> Result<&'a ModelFile, TranslateEngineError> {
    manifest
        .files
        .iter()
        .find(|file| file.role == role && file.required && !file.path.trim().is_empty())
        .ok_or_else(|| {
            TranslateEngineError::new(
                "model_file_missing",
                format!(
                    "translation model '{}' requires a '{}' file",
                    manifest.id, role
                ),
            )
        })
}

fn resolve_required_file(
    manifest: &ModelManifest,
    root: &Path,
    role: &str,
) -> Result<PathBuf, TranslateEngineError> {
    let file = require_role(manifest, role)?;
    Ok(root.join(&file.path))
}

fn resolve_optional_file(
    manifest: &ModelManifest,
    root: &Path,
    role: &str,
) -> Result<Option<PathBuf>, TranslateEngineError> {
    Ok(manifest
        .files
        .iter()
        .find(|file| file.role == role && !file.path.trim().is_empty())
        .map(|file| root.join(&file.path)))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use shared_models::{ModelDomain, ModelFile, ModelManifest, ModelSource};

    use super::{TranslationModelBundle, validate_translation_manifest};

    fn manifest(source: ModelSource) -> ModelManifest {
        ModelManifest {
            id: "opus-mt-en-zh-ct2-int8".to_owned(),
            name: "OPUS-MT English to Chinese CTranslate2 int8".to_owned(),
            domain: ModelDomain::Translation,
            family: "opus-mt".to_owned(),
            backend: "ctranslate2".to_owned(),
            version: "marian".to_owned(),
            source_languages: vec!["en".to_owned()],
            target_languages: vec!["zh-CN".to_owned()],
            quantization: Some("int8".to_owned()),
            low_spec_friendly: true,
            multilingual: false,
            source,
            files: vec![
                ModelFile::required("model", "model.bin"),
                ModelFile::required("config", "config.json"),
                ModelFile::required("source_tokenizer", "source.spm"),
                ModelFile::required("target_tokenizer", "target.spm"),
            ],
        }
    }

    #[test]
    fn rejects_builtin_manifest_without_local_files() {
        let error =
            TranslationModelBundle::from_manifest(&manifest(ModelSource::BuiltIn)).unwrap_err();

        assert_eq!(error.code, "translation_model_not_installed");
    }

    #[test]
    fn validates_local_translation_manifest() {
        let root = std::env::temp_dir().join(format!(
            "snap-pin-translation-bundle-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("model.bin"), [1]).unwrap();
        fs::write(root.join("config.json"), [2]).unwrap();
        fs::write(root.join("source.spm"), [3]).unwrap();
        fs::write(root.join("target.spm"), [4]).unwrap();

        let manifest = manifest(ModelSource::LocalPath(root.to_string_lossy().into_owned()));

        assert!(validate_translation_manifest(&manifest).is_ok());
        assert!(TranslationModelBundle::from_manifest(&manifest).is_ok());

        let _ = fs::remove_dir_all(root);
    }
}
