use shared_models::{ImageData, ModelManifest, OcrJob, OcrProvider, OcrResult};

use crate::OcrEngineError;

pub trait OcrEngine {
    fn provider(&self) -> OcrProvider;
    fn load_model(&mut self, model: &ModelManifest) -> Result<(), OcrEngineError>;
    fn recognize(&self, job: &OcrJob, image: &ImageData) -> Result<OcrResult, OcrEngineError>;
}
