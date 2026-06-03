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
    pub url: &'static str,
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
                    url: "https://raw.githubusercontent.com/zibo-chen/rust-paddle-ocr/next/models/PP-OCRv5_mobile_det.mnn",
                    package_file_name: "PP-OCRv5_mobile_det.mnn",
                    local_file_name: "det.mnn",
                    sha256: Some(
                        "326f846bb5c903282e116ea089e8796b67921586726cca9457730436a79684c3",
                    ),
                },
                ModelPackageFileSource {
                    role: "rec",
                    url: "https://raw.githubusercontent.com/zibo-chen/rust-paddle-ocr/next/models/PP-OCRv5_mobile_rec.mnn",
                    package_file_name: "PP-OCRv5_mobile_rec.mnn",
                    local_file_name: "rec.mnn",
                    sha256: Some(
                        "c809800b09263a8d18c678c211e470ffc464cbb33db2e6bde0244766f3feb0db",
                    ),
                },
                ModelPackageFileSource {
                    role: "keys",
                    url: "https://raw.githubusercontent.com/zibo-chen/rust-paddle-ocr/next/models/ppocr_keys_v5.txt",
                    package_file_name: "ppocr_keys_v5.txt",
                    local_file_name: "ppocr_keys_v5.txt",
                    sha256: Some(
                        "f2ed6bb20a850ce4767fa9b4622d9b282985ab7f0ea8f8c11abd790ca6d2ff94",
                    ),
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
                    url: "https://raw.githubusercontent.com/zibo-chen/rust-paddle-ocr/next/models/PP-OCRv5_mobile_det_fp16.mnn",
                    package_file_name: "PP-OCRv5_mobile_det_fp16.mnn",
                    local_file_name: "det.mnn",
                    sha256: None,
                },
                ModelPackageFileSource {
                    role: "rec",
                    url: "https://raw.githubusercontent.com/zibo-chen/rust-paddle-ocr/next/models/PP-OCRv5_mobile_rec_fp16.mnn",
                    package_file_name: "PP-OCRv5_mobile_rec_fp16.mnn",
                    local_file_name: "rec.mnn",
                    sha256: None,
                },
                ModelPackageFileSource {
                    role: "keys",
                    url: "https://raw.githubusercontent.com/zibo-chen/rust-paddle-ocr/next/models/ppocr_keys_v5.txt",
                    package_file_name: "ppocr_keys_v5.txt",
                    local_file_name: "ppocr_keys_v5.txt",
                    sha256: Some(
                        "f2ed6bb20a850ce4767fa9b4622d9b282985ab7f0ea8f8c11abd790ca6d2ff94",
                    ),
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
                    url: "https://raw.githubusercontent.com/zibo-chen/rust-paddle-ocr/next/models/ch_PP-OCRv4_det_infer.mnn",
                    package_file_name: "ch_PP-OCRv4_det_infer.mnn",
                    local_file_name: "det.mnn",
                    sha256: None,
                },
                ModelPackageFileSource {
                    role: "rec",
                    url: "https://raw.githubusercontent.com/zibo-chen/rust-paddle-ocr/next/models/ch_PP-OCRv4_rec_infer.mnn",
                    package_file_name: "ch_PP-OCRv4_rec_infer.mnn",
                    local_file_name: "rec.mnn",
                    sha256: None,
                },
                ModelPackageFileSource {
                    role: "keys",
                    url: "https://raw.githubusercontent.com/zibo-chen/rust-paddle-ocr/next/models/ppocr_keys_v4.txt",
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
