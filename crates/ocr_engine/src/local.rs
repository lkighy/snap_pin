use shared_models::{ImageData, ModelManifest, OcrJob, OcrLocalBackend, OcrProvider, OcrResult};

use perf_trace::{PerfSpan, log_elapsed};

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
        let span = PerfSpan::new("ocr_engine_load_model_total")
            .field("model_id", &model.id)
            .field("backend", &model.backend);
        let bundle_start = std::time::Instant::now();
        let bundle = OcrModelBundle::from_manifest(model)?;
        log_elapsed("ocr_engine_model_bundle_from_manifest", bundle_start);
        let validate_start = std::time::Instant::now();
        validate_backend_matches_provider(&self.backend, model)?;
        log_elapsed("ocr_engine_validate_backend", validate_start);
        self.loaded_bundle = Some(bundle);
        span.finish();
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

        let span = PerfSpan::new("ocr_engine_recognize_local_total")
            .field("backend", local_backend_name(&self.backend))
            .field("image_bytes", image.bytes.len())
            .field("width", image.metadata.pixel_size.width.round().max(1.0))
            .field("height", image.metadata.pixel_size.height.round().max(1.0));
        let result = ocr_rs_backend::recognize(bundle, job, image);
        if result.is_ok() {
            span.finish();
        }
        result
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
    }
}
