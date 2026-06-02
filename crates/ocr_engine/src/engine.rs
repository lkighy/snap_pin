use shared_models::{ImageData, ModelManifest, OcrJob, OcrProvider, OcrProviderProfile, OcrResult};

use crate::OcrEngineError;

pub trait OcrEngine {
    fn provider(&self) -> OcrProvider;
    fn configure_provider_profiles(&mut self, _profiles: &[OcrProviderProfile]) {}
    fn load_model(&mut self, model: &ModelManifest) -> Result<(), OcrEngineError>;
    fn recognize(&self, job: &OcrJob, image: &ImageData) -> Result<OcrResult, OcrEngineError>;
}

pub trait LocalOcrEngine {
    fn provider(&self) -> OcrProvider;
    fn load_model(&mut self, model: &ModelManifest) -> Result<(), OcrEngineError>;
    fn recognize_local(&self, job: &OcrJob, image: &ImageData)
    -> Result<OcrResult, OcrEngineError>;
}

pub trait ExternalOcrClient {
    fn provider(&self) -> OcrProvider;
    fn recognize_remote(
        &self,
        profile: &OcrProviderProfile,
        job: &OcrJob,
        image: &ImageData,
    ) -> Result<OcrResult, OcrEngineError>;
}
