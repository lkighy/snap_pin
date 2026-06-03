mod download;
mod import;
mod sources;
mod storage;

use std::path::Path;

pub use download::*;
pub use import::*;
pub use sources::*;
pub use storage::*;

use shared_models::{
    ModelDomain, ModelFile, ModelManifest, ModelSource, OcrLocalBackend, TranslateLocalBackend,
};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ModelRegistry {
    models: Vec<ModelManifest>,
}

impl ModelRegistry {
    pub fn with_builtin_defaults() -> Self {
        let mut registry = Self::default();
        registry.register(default_ocr_model());
        registry.register(lightweight_ocr_model());
        registry.register(compatible_ocr_model());
        registry.register(default_translation_model());
        registry
    }

    pub fn register(&mut self, manifest: ModelManifest) {
        if let Some(existing) = self.models.iter_mut().find(|model| model.id == manifest.id) {
            *existing = manifest;
        } else {
            self.models.push(manifest);
        }
    }

    pub fn import_manifest_file(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<&ModelManifest, ModelImportError> {
        let manifest = import_manifest_file(path)?;
        let model_id = manifest.id.clone();
        self.register(manifest);
        Ok(self
            .find(&model_id)
            .expect("registered model must be available"))
    }

    pub fn list(&self) -> &[ModelManifest] {
        &self.models
    }

    pub fn list_mut(&mut self) -> &mut Vec<ModelManifest> {
        &mut self.models
    }

    pub fn find(&self, id: &str) -> Option<&ModelManifest> {
        self.models.iter().find(|model| model.id == id)
    }

    pub fn recommended_ocr(&self) -> Option<&ModelManifest> {
        self.find("ppocr-v5-mobile-mnn").or_else(|| {
            self.models
                .iter()
                .find(|model| model.domain == ModelDomain::Ocr && model.low_spec_friendly)
        })
    }

    pub fn recommended_translation(
        &self,
        source_language: Option<&str>,
        target_language: &str,
    ) -> Option<&ModelManifest> {
        self.models
            .iter()
            .filter(|model| model.domain == ModelDomain::Translation)
            .filter(|model| model.supports_language_pair(source_language, target_language))
            .max_by_key(|model| model.low_spec_friendly)
    }
}

pub fn default_ocr_model() -> ModelManifest {
    ModelManifest {
        id: "ppocr-v5-mobile-mnn".to_owned(),
        name: "PP-OCRv5 Mobile MNN".to_owned(),
        domain: ModelDomain::Ocr,
        family: "ppocr".to_owned(),
        backend: backend_name(&OcrLocalBackend::Mnn),
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
            ModelFile::optional("cls", "cls.mnn"),
        ],
    }
}

pub fn lightweight_ocr_model() -> ModelManifest {
    ModelManifest {
        id: "ppocr-v5-mobile-fp16-mnn".to_owned(),
        name: "PP-OCRv5 Mobile FP16 MNN".to_owned(),
        domain: ModelDomain::Ocr,
        family: "ppocr".to_owned(),
        backend: backend_name(&OcrLocalBackend::Mnn),
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
            ModelFile::optional("cls", ""),
        ],
    }
}

pub fn compatible_ocr_model() -> ModelManifest {
    ModelManifest {
        id: "ppocr-v4-mobile-mnn".to_owned(),
        name: "PP-OCRv4 Mobile MNN".to_owned(),
        domain: ModelDomain::Ocr,
        family: "ppocr".to_owned(),
        backend: backend_name(&OcrLocalBackend::Mnn),
        version: "v4".to_owned(),
        source_languages: vec!["zh".to_owned(), "en".to_owned()],
        target_languages: Vec::new(),
        quantization: Some("fp16".to_owned()),
        low_spec_friendly: false,
        multilingual: true,
        source: ModelSource::BuiltIn,
        files: vec![
            ModelFile::required("det", "det.mnn"),
            ModelFile::required("rec", "rec.mnn"),
            ModelFile::required("keys", "ppocr_keys_v4.txt"),
            ModelFile::optional("cls", ""),
        ],
    }
}

pub fn default_translation_model() -> ModelManifest {
    ModelManifest {
        id: "opus-mt-en-zh-ct2-int8".to_owned(),
        name: "OPUS-MT English to Chinese CTranslate2 int8".to_owned(),
        domain: ModelDomain::Translation,
        family: "opus-mt".to_owned(),
        backend: translate_backend_name(&TranslateLocalBackend::CTranslate2),
        version: "marian".to_owned(),
        source_languages: vec!["en".to_owned()],
        target_languages: vec!["zh-CN".to_owned(), "zh".to_owned()],
        quantization: Some("int8".to_owned()),
        low_spec_friendly: true,
        multilingual: false,
        source: ModelSource::BuiltIn,
        files: vec![
            ModelFile::required("model", "model.bin"),
            ModelFile::required("config", "config.json"),
            ModelFile::required("source_tokenizer", "source.spm"),
            ModelFile::required("target_tokenizer", "target.spm"),
            ModelFile::optional("vocabulary", "shared_vocabulary.json"),
        ],
    }
}

fn backend_name(backend: &OcrLocalBackend) -> String {
    match backend {
        OcrLocalBackend::Mnn => "mnn",
        OcrLocalBackend::OnnxRuntime => "onnxruntime",
        OcrLocalBackend::PaddleRuntime => "paddle",
        OcrLocalBackend::Custom(value) => value,
    }
    .to_owned()
}

fn translate_backend_name(backend: &TranslateLocalBackend) -> String {
    match backend {
        TranslateLocalBackend::CTranslate2 => "ctranslate2",
        TranslateLocalBackend::Custom(value) => value,
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::ModelRegistry;

    #[test]
    fn includes_default_models() {
        let registry = ModelRegistry::with_builtin_defaults();

        assert!(registry.find("ppocr-v5-mobile-mnn").is_some());
        assert!(registry.find("ppocr-v5-mobile-fp16-mnn").is_some());
        assert!(registry.find("ppocr-v4-mobile-mnn").is_some());
        assert!(registry.find("opus-mt-en-zh-ct2-int8").is_some());
        assert!(registry.recommended_ocr().is_some());
        assert!(
            registry
                .recommended_translation(Some("en"), "zh-CN")
                .is_some()
        );
    }
}
