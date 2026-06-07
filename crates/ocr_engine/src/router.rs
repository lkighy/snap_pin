use std::collections::HashMap;

use shared_models::{
    ImageData, ModelManifest, OcrExternalProvider, OcrJob, OcrLocalBackend, OcrProvider,
    OcrProviderProfile, OcrResult,
};

use crate::{
    ExternalOcrClient, HttpOcrClient, LocalOcrEngine, MockOcrEngine, OcrEngine, OcrEngineError,
    PaddleOcrLocalEngine,
};

#[derive(Clone)]
pub struct RoutedOcrEngine {
    local_mnn: PaddleOcrLocalEngine,
    local_onnx: PaddleOcrLocalEngine,
    local_paddle: PaddleOcrLocalEngine,
    mock: MockOcrEngine,
    profiles: HashMap<String, OcrProviderProfile>,
}

impl Default for RoutedOcrEngine {
    fn default() -> Self {
        Self {
            local_mnn: PaddleOcrLocalEngine::new(OcrLocalBackend::Mnn),
            local_onnx: PaddleOcrLocalEngine::new(OcrLocalBackend::OnnxRuntime),
            local_paddle: PaddleOcrLocalEngine::new(OcrLocalBackend::PaddleRuntime),
            mock: MockOcrEngine::default(),
            profiles: HashMap::new(),
        }
    }
}

impl OcrEngine for RoutedOcrEngine {
    fn provider(&self) -> OcrProvider {
        OcrProvider::Local(OcrLocalBackend::Mnn)
    }

    fn configure_provider_profiles(&mut self, profiles: &[OcrProviderProfile]) {
        self.profiles = profiles
            .iter()
            .cloned()
            .map(|profile| (profile.id.clone(), profile))
            .collect();
    }

    fn load_model(&mut self, model: &ModelManifest) -> Result<(), OcrEngineError> {
        match model.backend.as_str() {
            "mnn" => self.local_mnn.load_model(model),
            "onnxruntime" | "onnx" => self.local_onnx.load_model(model),
            "paddle" | "paddleruntime" => self.local_paddle.load_model(model),
            _ => self.mock.load_model(model),
        }
    }

    fn recognize(&self, job: &OcrJob, image: &ImageData) -> Result<OcrResult, OcrEngineError> {
        match &job.provider {
            OcrProvider::Disabled => Err(OcrEngineError::new(
                "ocr_disabled",
                "OCR is disabled for this job",
            )),
            OcrProvider::System => Err(OcrEngineError::new(
                "system_ocr_requires_platform",
                "system OCR is a platform capability and must be dispatched through platform_api",
            )),
            OcrProvider::Local(OcrLocalBackend::Mnn) => self.local_mnn.recognize_local(job, image),
            OcrProvider::Local(OcrLocalBackend::OnnxRuntime) => {
                self.local_onnx.recognize_local(job, image)
            }
            OcrProvider::Local(OcrLocalBackend::PaddleRuntime) => {
                self.local_paddle.recognize_local(job, image)
            }
            OcrProvider::Local(OcrLocalBackend::Custom(_)) => self.mock.recognize(job, image),
            OcrProvider::ExternalApi(provider) => {
                let profile = self.resolve_profile(job, provider)?;
                HttpOcrClient::new(provider.clone()).recognize_remote(profile, job, image)
            }
        }
    }
}

impl RoutedOcrEngine {
    fn resolve_profile(
        &self,
        job: &OcrJob,
        provider: &OcrExternalProvider,
    ) -> Result<&OcrProviderProfile, OcrEngineError> {
        let Some(profile_id) = job.provider_profile_id.as_deref() else {
            return Err(OcrEngineError::new(
                "ocr_profile_missing",
                "external OCR requires a provider profile id",
            ));
        };

        let Some(profile) = self.profiles.get(profile_id) else {
            return Err(OcrEngineError::new(
                "ocr_profile_not_found",
                format!("external OCR profile '{}' is not configured", profile_id),
            ));
        };

        if &profile.provider != provider {
            return Err(OcrEngineError::new(
                "ocr_profile_provider_mismatch",
                format!(
                    "external OCR profile '{}' does not match requested provider",
                    profile_id
                ),
            ));
        }

        Ok(profile)
    }
}
