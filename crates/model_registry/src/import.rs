use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use shared_models::{ModelDomain, ModelFile, ModelManifest, ModelSource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelImportError {
    pub code: String,
    pub message: String,
}

impl ModelImportError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ManifestToml {
    id: String,
    name: String,
    family: String,
    version: String,
    backend: String,
    #[serde(default)]
    precision: Option<String>,
    #[serde(default)]
    language: Vec<String>,
    #[serde(default)]
    low_spec_friendly: bool,
    #[serde(default = "default_domain")]
    domain: String,
    files: ManifestFilesToml,
    #[serde(default)]
    checksums: ManifestChecksumsToml,
}

#[derive(Debug, Deserialize)]
struct ManifestFilesToml {
    det: Option<String>,
    rec: Option<String>,
    keys: Option<String>,
    cls: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ManifestChecksumsToml {
    det: Option<String>,
    rec: Option<String>,
    keys: Option<String>,
    cls: Option<String>,
}

fn default_domain() -> String {
    "ocr".to_owned()
}

pub fn import_manifest_file(path: impl AsRef<Path>) -> Result<ModelManifest, ModelImportError> {
    let path = path.as_ref();
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let contents = fs::read_to_string(path).map_err(|error| {
        ModelImportError::new(
            "model_manifest_read_failed",
            format!(
                "failed to read model manifest '{}': {error}",
                path.display()
            ),
        )
    })?;

    import_manifest_toml(root, &contents)
}

pub fn import_manifest_toml(
    root: impl AsRef<Path>,
    contents: &str,
) -> Result<ModelManifest, ModelImportError> {
    let root = root.as_ref();
    let manifest: ManifestToml = toml::from_str(contents).map_err(|error| {
        ModelImportError::new(
            "model_manifest_parse_failed",
            format!("failed to parse model manifest: {error}"),
        )
    })?;

    if manifest.domain != "ocr" {
        return Err(ModelImportError::new(
            "model_manifest_domain_unsupported",
            format!("unsupported model domain '{}'", manifest.domain),
        ));
    }

    let det = required_file(&manifest.files.det, "det")?;
    let rec = required_file(&manifest.files.rec, "rec")?;
    let keys = required_file(&manifest.files.keys, "keys")?;
    let cls = manifest.files.cls.clone().unwrap_or_default();

    verify_file(root, "det", &det, manifest.checksums.det.as_deref())?;
    verify_file(root, "rec", &rec, manifest.checksums.rec.as_deref())?;
    verify_file(root, "keys", &keys, manifest.checksums.keys.as_deref())?;
    if !cls.is_empty() {
        verify_file(root, "cls", &cls, manifest.checksums.cls.as_deref())?;
    }

    let files = vec![
        ModelFile::required("det", det),
        ModelFile::required("rec", rec),
        ModelFile::required("keys", keys),
        ModelFile::optional("cls", cls),
    ];

    Ok(ModelManifest {
        id: manifest.id,
        name: manifest.name,
        domain: ModelDomain::Ocr,
        family: manifest.family,
        backend: manifest.backend,
        version: manifest.version,
        source_languages: manifest.language,
        target_languages: Vec::new(),
        quantization: manifest.precision,
        low_spec_friendly: manifest.low_spec_friendly,
        multilingual: true,
        source: ModelSource::LocalPath(root.to_string_lossy().into_owned()),
        files,
    })
}

fn required_file(value: &Option<String>, role: &str) -> Result<String, ModelImportError> {
    let path = value.as_deref().unwrap_or_default().trim();
    if path.is_empty() {
        return Err(ModelImportError::new(
            "model_manifest_file_missing",
            format!("model manifest requires a '{role}' file"),
        ));
    }

    Ok(path.to_owned())
}

fn verify_file(
    root: &Path,
    role: &str,
    relative_path: &str,
    expected_sha256: Option<&str>,
) -> Result<(), ModelImportError> {
    let path = root.join(relative_path);
    if !path.exists() {
        return Err(ModelImportError::new(
            "model_file_missing",
            format!(
                "required model file '{}' for role '{}' does not exist",
                path.display(),
                role
            ),
        ));
    }

    if let Some(expected_sha256) = expected_sha256 {
        verify_sha256(&path, role, expected_sha256)?;
    }

    Ok(())
}

fn verify_sha256(
    path: &PathBuf,
    role: &str,
    expected_sha256: &str,
) -> Result<(), ModelImportError> {
    let bytes = fs::read(path).map_err(|error| {
        ModelImportError::new(
            "model_file_read_failed",
            format!("failed to read model file '{}': {error}", path.display()),
        )
    })?;
    let actual = format!("{:x}", Sha256::digest(&bytes));

    if actual != expected_sha256 {
        return Err(ModelImportError::new(
            "model_checksum_mismatch",
            format!(
                "model file '{}' for role '{}' failed sha256 check",
                path.display(),
                role
            ),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::import_manifest_toml;

    #[test]
    fn imports_ocr_manifest_toml() {
        let root =
            std::env::temp_dir().join(format!("snap-pin-ocr-manifest-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("det.mnn"), [1]).unwrap();
        fs::write(root.join("rec.mnn"), [2]).unwrap();
        fs::write(root.join("ppocr_keys_v5.txt"), [3]).unwrap();

        let manifest = import_manifest_toml(
            &root,
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

        assert_eq!(manifest.id, "ppocr-v5-mobile-mnn");
        assert_eq!(manifest.files.len(), 4);

        let _ = fs::remove_dir_all(root);
    }
}
