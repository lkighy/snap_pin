use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::blocking::Client;
use sha2::{Digest, Sha256};

use crate::ModelImportError;

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

pub fn download_model_file(
    request: &ModelDownloadRequest,
) -> Result<ModelDownloadResult, ModelImportError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(120))
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
    let bytes = response.bytes().map_err(|error| {
        ModelImportError::new(
            "model_download_read_failed",
            format!(
                "failed to read model download body from '{}': {error}",
                request.url
            ),
        )
    })?;

    let actual_sha256 = format!("{:x}", Sha256::digest(&bytes));
    if let Some(expected_sha256) = &request.sha256 {
        if !expected_sha256.eq_ignore_ascii_case(&actual_sha256) {
            return Err(ModelImportError::new(
                "model_download_checksum_mismatch",
                "downloaded model file failed sha256 check",
            ));
        }
    }

    write_atomic(&request.target_path, &bytes)?;
    Ok(ModelDownloadResult {
        path: request.target_path.clone(),
        sha256: actual_sha256,
        bytes: bytes.len() as u64,
    })
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

    match fs::rename(&tmp_path, path) {
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
            fs::rename(&tmp_path, path).map_err(|rename_error| {
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
