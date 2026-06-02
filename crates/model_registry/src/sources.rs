#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPackageSource {
    pub model_id: &'static str,
    pub source_id: &'static str,
    pub source_name: &'static str,
    pub homepage: &'static str,
    pub files: Vec<ModelPackageFileSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPackageFileSource {
    pub role: &'static str,
    pub package_file_name: &'static str,
    pub local_file_name: &'static str,
    pub sha256: Option<&'static str>,
}

pub fn builtin_ocr_package_sources() -> Vec<ModelPackageSource> {
    vec![
        ModelPackageSource {
            model_id: "ppocr-v5-mobile-mnn",
            source_id: "ocr-rs-paddleocr-mnn",
            source_name: "ocr-rs PaddleOCR MNN model package",
            homepage: "https://github.com/zibo-chen/rust-paddle-ocr",
            files: vec![
                ModelPackageFileSource {
                    role: "det",
                    package_file_name: "PP-OCRv5_mobile_det.mnn",
                    local_file_name: "det.mnn",
                    sha256: None,
                },
                ModelPackageFileSource {
                    role: "rec",
                    package_file_name: "PP-OCRv5_mobile_rec.mnn",
                    local_file_name: "rec.mnn",
                    sha256: None,
                },
                ModelPackageFileSource {
                    role: "keys",
                    package_file_name: "ppocr_keys_v5.txt",
                    local_file_name: "ppocr_keys_v5.txt",
                    sha256: None,
                },
            ],
        },
        ModelPackageSource {
            model_id: "ppocr-v5-mobile-fp16-mnn",
            source_id: "ocr-rs-paddleocr-mnn",
            source_name: "ocr-rs PaddleOCR MNN model package",
            homepage: "https://github.com/zibo-chen/rust-paddle-ocr",
            files: vec![
                ModelPackageFileSource {
                    role: "det",
                    package_file_name: "PP-OCRv5_mobile_det_fp16.mnn",
                    local_file_name: "det.mnn",
                    sha256: None,
                },
                ModelPackageFileSource {
                    role: "rec",
                    package_file_name: "PP-OCRv5_mobile_rec_fp16.mnn",
                    local_file_name: "rec.mnn",
                    sha256: None,
                },
                ModelPackageFileSource {
                    role: "keys",
                    package_file_name: "ppocr_keys_v5.txt",
                    local_file_name: "ppocr_keys_v5.txt",
                    sha256: None,
                },
            ],
        },
        ModelPackageSource {
            model_id: "ppocr-v4-mobile-mnn",
            source_id: "ocr-rs-paddleocr-mnn",
            source_name: "ocr-rs PaddleOCR MNN model package",
            homepage: "https://github.com/zibo-chen/rust-paddle-ocr",
            files: vec![
                ModelPackageFileSource {
                    role: "det",
                    package_file_name: "ch_PP-OCRv4_det_infer.mnn",
                    local_file_name: "det.mnn",
                    sha256: None,
                },
                ModelPackageFileSource {
                    role: "rec",
                    package_file_name: "ch_PP-OCRv4_rec_infer.mnn",
                    local_file_name: "rec.mnn",
                    sha256: None,
                },
                ModelPackageFileSource {
                    role: "keys",
                    package_file_name: "ppocr_keys_v4.txt",
                    local_file_name: "ppocr_keys_v4.txt",
                    sha256: None,
                },
            ],
        },
    ]
}

pub fn find_builtin_ocr_package_source(model_id: &str) -> Option<ModelPackageSource> {
    builtin_ocr_package_sources()
        .into_iter()
        .find(|source| source.model_id == model_id)
}
