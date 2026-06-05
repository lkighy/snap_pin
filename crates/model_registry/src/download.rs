use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::blocking::Client;
use sha2::{Digest, Sha256};

use shared_models::{ModelManifest, ModelSource};

use crate::{
    ModelImportError, ModelPackageSource, ModelStorage, compatible_ocr_model, default_ocr_model,
    default_translation_model, import_manifest_file, lightweight_ocr_model,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDownloadRequest {
    pub url: String,
    pub sha256: Option<String>,
    pub target_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDownloadResult {
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

pub fn download_model_file(
    request: &ModelDownloadRequest,
) -> Result<ModelDownloadResult, ModelImportError> {
    download_model_file_with_progress(request, |_| {}, || false)
}

pub fn download_model_file_with_progress(
    request: &ModelDownloadRequest,
    mut progress: impl FnMut(ModelDownloadProgress),
    mut should_cancel: impl FnMut() -> bool,
) -> Result<ModelDownloadResult, ModelImportError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|error| {
            ModelImportError::new(
                "model_downloader_init_failed",
                format!("failed to initialize model downloader: {error}"),
            )
        })?;

    let response = client.get(&request.url).send().map_err(|error| {
        ModelImportError::new(
            "model_download_failed",
            format!("failed to download model from '{}': {error}", request.url),
        )
    })?;
    let response = response.error_for_status().map_err(|error| {
        ModelImportError::new(
            "model_download_http_failed",
            format!(
                "model download returned an HTTP error for '{}': {error}",
                request.url
            ),
        )
    })?;
    let total_bytes = response.content_length();

    let (tmp_path, actual_sha256, bytes) = write_download_stream_tmp(
        response,
        &request.target_path,
        total_bytes,
        &mut progress,
        &mut should_cancel,
    )?;
    if let Some(expected_sha256) = &request.sha256 {
        if !expected_sha256.eq_ignore_ascii_case(&actual_sha256) {
            let _ = fs::remove_file(&tmp_path);
            return Err(ModelImportError::new(
                "model_download_checksum_mismatch",
                "downloaded model file failed sha256 check",
            ));
        }
    }

    replace_tmp_file(&tmp_path, &request.target_path)?;
    Ok(ModelDownloadResult {
        path: request.target_path.clone(),
        sha256: actual_sha256,
        bytes,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPackageDownloadResult {
    pub manifest: ModelManifest,
    pub files: Vec<ModelDownloadResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPackageDownloadProgress {
    pub file_index: usize,
    pub file_count: usize,
    pub role: String,
    pub file_name: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

impl ModelStorage {
    pub fn download_builtin_ocr_package(
        &self,
        source: &ModelPackageSource,
    ) -> Result<ModelPackageDownloadResult, ModelImportError> {
        self.download_builtin_ocr_package_with_progress(source, |_| {}, || false)
    }

    pub fn download_builtin_ocr_package_with_progress(
        &self,
        source: &ModelPackageSource,
        mut progress: impl FnMut(ModelPackageDownloadProgress),
        mut should_cancel: impl FnMut() -> bool,
    ) -> Result<ModelPackageDownloadResult, ModelImportError> {
        let model_dir = self.model_dir(&source.model_id);
        fs::create_dir_all(&model_dir).map_err(|error| {
            ModelImportError::new(
                "model_download_storage_failed",
                format!(
                    "failed to create model download directory '{}': {error}",
                    model_dir.display()
                ),
            )
        })?;

        let mut downloads = Vec::new();
        let mut checksums = BTreeMap::new();
        let file_count = source.files.len();
        for (file_index, file) in source.files.iter().enumerate() {
            let result = download_model_file_with_progress(
                &ModelDownloadRequest {
                    url: file.url.to_owned(),
                    sha256: file.sha256.map(str::to_owned),
                    target_path: model_dir.join(file.local_file_name),
                },
                |file_progress| {
                    progress(ModelPackageDownloadProgress {
                        file_index,
                        file_count,
                        role: file.role.to_owned(),
                        file_name: file.local_file_name.to_owned(),
                        downloaded_bytes: file_progress.downloaded_bytes,
                        total_bytes: file_progress.total_bytes,
                    });
                },
                &mut should_cancel,
            )?;
            checksums.insert(file.role, result.sha256.clone());
            downloads.push(result);
        }

        write_builtin_ocr_manifest(source, &model_dir, &checksums)?;
        let mut manifest = import_manifest_file(model_dir.join("manifest.toml"))?;
        manifest.source = ModelSource::LocalPath(model_dir.to_string_lossy().into_owned());

        Ok(ModelPackageDownloadResult {
            manifest,
            files: downloads,
        })
    }

    pub fn download_builtin_translation_package(
        &self,
        source: &ModelPackageSource,
    ) -> Result<ModelPackageDownloadResult, ModelImportError> {
        self.download_builtin_translation_package_with_progress(source, |_| {}, || false)
    }

    pub fn download_builtin_translation_package_with_progress(
        &self,
        source: &ModelPackageSource,
        progress: impl FnMut(ModelPackageDownloadProgress),
        should_cancel: impl FnMut() -> bool,
    ) -> Result<ModelPackageDownloadResult, ModelImportError> {
        self.download_builtin_package_with_progress(
            source,
            write_builtin_translation_manifest,
            progress,
            should_cancel,
        )
    }

    fn download_builtin_package_with_progress(
        &self,
        source: &ModelPackageSource,
        write_manifest: impl Fn(
            &ModelPackageSource,
            &Path,
            &BTreeMap<&str, String>,
        ) -> Result<(), ModelImportError>,
        mut progress: impl FnMut(ModelPackageDownloadProgress),
        mut should_cancel: impl FnMut() -> bool,
    ) -> Result<ModelPackageDownloadResult, ModelImportError> {
        let model_dir = self.model_dir(&source.model_id);
        fs::create_dir_all(&model_dir).map_err(|error| {
            ModelImportError::new(
                "model_download_storage_failed",
                format!(
                    "failed to create model download directory '{}': {error}",
                    model_dir.display()
                ),
            )
        })?;

        let mut downloads = Vec::new();
        let mut checksums = BTreeMap::new();
        let file_count = source.files.len();
        for (file_index, file) in source.files.iter().enumerate() {
            let result = download_model_file_with_progress(
                &ModelDownloadRequest {
                    url: file.url.to_owned(),
                    sha256: file.sha256.map(str::to_owned),
                    target_path: model_dir.join(file.local_file_name),
                },
                |file_progress| {
                    progress(ModelPackageDownloadProgress {
                        file_index,
                        file_count,
                        role: file.role.to_owned(),
                        file_name: file.local_file_name.to_owned(),
                        downloaded_bytes: file_progress.downloaded_bytes,
                        total_bytes: file_progress.total_bytes,
                    });
                },
                &mut should_cancel,
            )?;
            checksums.insert(file.role, result.sha256.clone());
            downloads.push(result);
        }

        write_manifest(source, &model_dir, &checksums)?;
        let mut manifest = import_manifest_file(model_dir.join("manifest.toml"))?;
        manifest.source = ModelSource::LocalPath(model_dir.to_string_lossy().into_owned());

        Ok(ModelPackageDownloadResult {
            manifest,
            files: downloads,
        })
    }
}

fn write_builtin_ocr_manifest(
    source: &ModelPackageSource,
    model_dir: &Path,
    checksums: &BTreeMap<&str, String>,
) -> Result<(), ModelImportError> {
    let model = default_ocr_model_for_source(source)?;
    let det = local_file_for_role(source, "det")?;
    let rec = local_file_for_role(source, "rec")?;
    let keys = local_file_for_role(source, "keys")?;
    let manifest = format!(
        r#"id = "{id}"
name = "{name}"
family = "{family}"
version = "{version}"
backend = "{backend}"
precision = "{precision}"
language = [{languages}]
low_spec_friendly = {low_spec_friendly}

[files]
det = "{det}"
rec = "{rec}"
keys = "{keys}"
cls = ""

[checksums]
det = "{det_sha256}"
rec = "{rec_sha256}"
keys = "{keys_sha256}"
"#,
        id = model.id,
        name = model.name,
        family = model.family,
        version = model.version,
        backend = model.backend,
        precision = model.quantization.unwrap_or_default(),
        languages = model
            .source_languages
            .iter()
            .map(|language| format!("\"{language}\""))
            .collect::<Vec<_>>()
            .join(", "),
        low_spec_friendly = model.low_spec_friendly,
        det = det,
        rec = rec,
        keys = keys,
        det_sha256 = checksum_for_role(checksums, "det")?,
        rec_sha256 = checksum_for_role(checksums, "rec")?,
        keys_sha256 = checksum_for_role(checksums, "keys")?,
    );

    write_atomic(&model_dir.join("manifest.toml"), manifest.as_bytes())
}

fn write_builtin_translation_manifest(
    source: &ModelPackageSource,
    model_dir: &Path,
    checksums: &BTreeMap<&str, String>,
) -> Result<(), ModelImportError> {
    let model = default_translation_model_for_source(source)?;
    let model_file = local_file_for_role(source, "model")?;
    let config = local_file_for_role(source, "config")?;
    let source_tokenizer = local_file_for_role(source, "source_tokenizer")?;
    let target_tokenizer = local_file_for_role(source, "target_tokenizer")?;
    let vocabulary = local_file_for_role(source, "vocabulary").unwrap_or("");
    let manifest = format!(
        r#"id = "{id}"
name = "{name}"
domain = "translation"
family = "{family}"
version = "{version}"
backend = "{backend}"
precision = "{precision}"
source_languages = [{source_languages}]
target_languages = [{target_languages}]
low_spec_friendly = {low_spec_friendly}
multilingual = {multilingual}

[files]
model = "{model_file}"
config = "{config}"
source_tokenizer = "{source_tokenizer}"
target_tokenizer = "{target_tokenizer}"
vocabulary = "{vocabulary}"

[checksums]
model = "{model_sha256}"
config = "{config_sha256}"
source_tokenizer = "{source_tokenizer_sha256}"
target_tokenizer = "{target_tokenizer_sha256}"
vocabulary = "{vocabulary_sha256}"
"#,
        id = model.id,
        name = model.name,
        family = model.family,
        version = model.version,
        backend = model.backend,
        precision = model.quantization.unwrap_or_default(),
        source_languages = toml_string_array(&model.source_languages),
        target_languages = toml_string_array(&model.target_languages),
        low_spec_friendly = model.low_spec_friendly,
        multilingual = model.multilingual,
        model_file = model_file,
        config = config,
        source_tokenizer = source_tokenizer,
        target_tokenizer = target_tokenizer,
        vocabulary = vocabulary,
        model_sha256 = checksum_for_role(checksums, "model")?,
        config_sha256 = checksum_for_role(checksums, "config")?,
        source_tokenizer_sha256 = checksum_for_role(checksums, "source_tokenizer")?,
        target_tokenizer_sha256 = checksum_for_role(checksums, "target_tokenizer")?,
        vocabulary_sha256 = checksum_for_role(checksums, "vocabulary")?,
    );

    write_atomic(&model_dir.join("manifest.toml"), manifest.as_bytes())
}

fn toml_string_array(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

fn default_ocr_model_for_source(
    source: &ModelPackageSource,
) -> Result<ModelManifest, ModelImportError> {
    match source.model_id {
        "ppocr-v5-mobile-mnn" => Ok(default_ocr_model()),
        "ppocr-v5-mobile-fp16-mnn" => Ok(lightweight_ocr_model()),
        "ppocr-v4-mobile-mnn" => Ok(compatible_ocr_model()),
        _ => Err(ModelImportError::new(
            "model_download_source_unsupported",
            format!(
                "unsupported built-in OCR model source '{}'",
                source.model_id
            ),
        )),
    }
}

fn default_translation_model_for_source(
    source: &ModelPackageSource,
) -> Result<ModelManifest, ModelImportError> {
    match source.model_id {
        "opus-mt-en-zh-ct2-int8" => Ok(default_translation_model()),
        _ => Err(ModelImportError::new(
            "model_download_source_unsupported",
            format!(
                "unsupported built-in translation model source '{}'",
                source.model_id
            ),
        )),
    }
}

fn local_file_for_role(
    source: &ModelPackageSource,
    role: &str,
) -> Result<&'static str, ModelImportError> {
    source
        .files
        .iter()
        .find(|file| file.role == role)
        .map(|file| file.local_file_name)
        .ok_or_else(|| {
            ModelImportError::new(
                "model_download_source_invalid",
                format!(
                    "model source '{}' is missing role '{role}'",
                    source.model_id
                ),
            )
        })
}

fn checksum_for_role(
    checksums: &BTreeMap<&str, String>,
    role: &str,
) -> Result<String, ModelImportError> {
    checksums.get(role).cloned().ok_or_else(|| {
        ModelImportError::new(
            "model_download_checksum_missing",
            format!("downloaded model package is missing checksum for role '{role}'"),
        )
    })
}

fn write_download_stream_tmp(
    mut response: reqwest::blocking::Response,
    path: &Path,
    total_bytes: Option<u64>,
    progress: &mut impl FnMut(ModelDownloadProgress),
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<(PathBuf, String, u64), ModelImportError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ModelImportError::new(
                "model_download_storage_failed",
                format!(
                    "failed to create model download directory '{}': {error}",
                    parent.display()
                ),
            )
        })?;
    }

    let tmp_path = path.with_extension("download.tmp");
    let mut file = fs::File::create(&tmp_path).map_err(|error| {
        ModelImportError::new(
            "model_download_storage_failed",
            format!(
                "failed to create temporary file '{}': {error}",
                tmp_path.display()
            ),
        )
    })?;

    let mut hasher = Sha256::new();
    let mut downloaded_bytes = 0;
    let mut buffer = [0_u8; 128 * 1024];
    progress(ModelDownloadProgress {
        downloaded_bytes,
        total_bytes,
    });

    loop {
        if should_cancel() {
            drop(file);
            let _ = fs::remove_file(&tmp_path);
            return Err(ModelImportError::new(
                "model_download_cancelled",
                "model download was cancelled",
            ));
        }

        let bytes_read = response.read(&mut buffer).map_err(|error| {
            let _ = fs::remove_file(&tmp_path);
            ModelImportError::new(
                "model_download_read_failed",
                format!("failed to read model download body: {error}"),
            )
        })?;
        if bytes_read == 0 {
            break;
        }

        file.write_all(&buffer[..bytes_read]).map_err(|error| {
            let _ = fs::remove_file(&tmp_path);
            ModelImportError::new(
                "model_download_storage_failed",
                format!(
                    "failed to write temporary file '{}': {error}",
                    tmp_path.display()
                ),
            )
        })?;
        hasher.update(&buffer[..bytes_read]);
        downloaded_bytes += bytes_read as u64;
        progress(ModelDownloadProgress {
            downloaded_bytes,
            total_bytes,
        });
    }

    file.sync_all().map_err(|error| {
        let _ = fs::remove_file(&tmp_path);
        ModelImportError::new(
            "model_download_storage_failed",
            format!(
                "failed to flush temporary file '{}': {error}",
                tmp_path.display()
            ),
        )
    })?;
    drop(file);

    Ok((
        tmp_path,
        format!("{:x}", hasher.finalize()),
        downloaded_bytes,
    ))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), ModelImportError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ModelImportError::new(
                "model_download_storage_failed",
                format!(
                    "failed to create model download directory '{}': {error}",
                    parent.display()
                ),
            )
        })?;
    }

    let tmp_path = path.with_extension("download.tmp");
    {
        let mut file = fs::File::create(&tmp_path).map_err(|error| {
            ModelImportError::new(
                "model_download_storage_failed",
                format!(
                    "failed to create temporary file '{}': {error}",
                    tmp_path.display()
                ),
            )
        })?;
        file.write_all(bytes).map_err(|error| {
            ModelImportError::new(
                "model_download_storage_failed",
                format!(
                    "failed to write temporary file '{}': {error}",
                    tmp_path.display()
                ),
            )
        })?;
        file.sync_all().map_err(|error| {
            ModelImportError::new(
                "model_download_storage_failed",
                format!(
                    "failed to flush temporary file '{}': {error}",
                    tmp_path.display()
                ),
            )
        })?;
    }

    replace_tmp_file(&tmp_path, path)
}

fn replace_tmp_file(tmp_path: &Path, path: &Path) -> Result<(), ModelImportError> {
    match fs::rename(tmp_path, path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            fs::remove_file(path).map_err(|remove_error| {
                ModelImportError::new(
                    "model_download_storage_failed",
                    format!(
                        "failed to replace existing model file '{}': {remove_error}",
                        path.display()
                    ),
                )
            })?;
            fs::rename(tmp_path, path).map_err(|rename_error| {
                ModelImportError::new(
                    "model_download_storage_failed",
                    format!(
                        "failed to move downloaded model file to '{}': {rename_error}",
                        path.display()
                    ),
                )
            })
        }
        Err(error) => Err(ModelImportError::new(
            "model_download_storage_failed",
            format!(
                "failed to move downloaded model file to '{}': {error}",
                path.display()
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use std::collections::BTreeMap;

    use sha2::Digest;

    use crate::{
        ModelSource, ModelStorage, find_builtin_ocr_package_source,
        find_builtin_translation_package_source, import_manifest_file,
    };

    use super::write_builtin_translation_manifest;

    #[test]
    #[ignore = "downloads PP-OCRv5 MNN files from the network"]
    fn downloads_builtin_ppocr_v5_package() {
        let root =
            std::env::temp_dir().join(format!("snap-pin-model-download-{}", std::process::id()));
        let source = find_builtin_ocr_package_source("ppocr-v5-mobile-mnn").unwrap();
        let storage = ModelStorage::new(&root);

        let result = storage.download_builtin_ocr_package(&source).unwrap();
        let model_root = root.join("ppocr-v5-mobile-mnn");

        assert_eq!(result.manifest.id, "ppocr-v5-mobile-mnn");
        assert!(matches!(result.manifest.source, ModelSource::LocalPath(_)));
        assert_eq!(result.files.len(), 3);
        assert!(model_root.join("manifest.toml").exists());
        assert!(model_root.join("det.mnn").exists());
        assert!(model_root.join("rec.mnn").exists());
        assert!(model_root.join("ppocr_keys_v5.txt").exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_builtin_translation_manifest() {
        let root = std::env::temp_dir().join(format!(
            "snap-pin-translation-download-manifest-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("model.bin"), [1]).unwrap();
        fs::write(root.join("config.json"), [2]).unwrap();
        fs::write(root.join("source.spm"), [3]).unwrap();
        fs::write(root.join("target.spm"), [4]).unwrap();
        fs::write(root.join("shared_vocabulary.json"), [5]).unwrap();

        let source = find_builtin_translation_package_source("opus-mt-en-zh-ct2-int8").unwrap();
        let checksums = BTreeMap::from([
            ("model", format!("{:x}", sha2::Sha256::digest([1]))),
            ("config", format!("{:x}", sha2::Sha256::digest([2]))),
            (
                "source_tokenizer",
                format!("{:x}", sha2::Sha256::digest([3])),
            ),
            (
                "target_tokenizer",
                format!("{:x}", sha2::Sha256::digest([4])),
            ),
            ("vocabulary", format!("{:x}", sha2::Sha256::digest([5]))),
        ]);

        write_builtin_translation_manifest(&source, &root, &checksums).unwrap();
        let manifest = import_manifest_file(root.join("manifest.toml")).unwrap();

        assert_eq!(manifest.id, "opus-mt-en-zh-ct2-int8");
        assert!(matches!(manifest.source, ModelSource::LocalPath(_)));
        assert_eq!(manifest.domain, shared_models::ModelDomain::Translation);
        assert_eq!(manifest.source_languages, vec!["en"]);
        assert!(manifest.target_languages.contains(&"zh-CN".to_owned()));
        assert!(manifest.target_languages.contains(&"zh".to_owned()));

        let _ = fs::remove_dir_all(root);
    }
}
