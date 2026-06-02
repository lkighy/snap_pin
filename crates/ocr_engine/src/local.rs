use shared_models::{ImageData, ModelManifest, OcrJob, OcrLocalBackend, OcrProvider, OcrResult};

use crate::{LocalOcrEngine, OcrEngineError, OcrModelBundle, ocr_rs_backend};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaddleOcrLocalEngine {
    backend: OcrLocalBackend,
    loaded_bundle: Option<OcrModelBundle>,
}

impl PaddleOcrLocalEngine {
    pub fn new(backend: OcrLocalBackend) -> Self {
        Self {
            backend,
            loaded_bundle: None,
        }
    }
}

impl LocalOcrEngine for PaddleOcrLocalEngine {
    fn provider(&self) -> OcrProvider {
        OcrProvider::Local(self.backend.clone())
    }

    fn load_model(&mut self, model: &ModelManifest) -> Result<(), OcrEngineError> {
        let bundle = OcrModelBundle::from_manifest(model)?;
        validate_backend_matches_provider(&self.backend, model)?;
        self.loaded_bundle = Some(bundle);
        Ok(())
    }

    fn recognize_local(
        &self,
        job: &OcrJob,
        image: &ImageData,
    ) -> Result<OcrResult, OcrEngineError> {
        let Some(bundle) = self.loaded_bundle.as_ref() else {
            return Err(OcrEngineError::new(
                "ocr_model_not_loaded",
                "local OCR requires a validated model bundle before recognition",
            ));
        };

        ocr_rs_backend::recognize(bundle, job, image)
    }
}

fn validate_backend_matches_provider(
    backend: &OcrLocalBackend,
    model: &ModelManifest,
) -> Result<(), OcrEngineError> {
    let expected = local_backend_name(backend);
    if model.backend != expected {
        return Err(OcrEngineError::new(
            "model_backend_mismatch",
            format!(
                "OCR provider '{}' cannot load model '{}' with backend '{}'",
                expected, model.id, model.backend
            ),
        ));
    }

    Ok(())
}

fn local_backend_name(backend: &OcrLocalBackend) -> &str {
    match backend {
        OcrLocalBackend::Mnn => "mnn",
        OcrLocalBackend::OnnxRuntime => "onnxruntime",
        OcrLocalBackend::PaddleRuntime => "paddle",
        OcrLocalBackend::Custom(value) => value.as_str(),
    }
}
