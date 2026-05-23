use shared_models::{
    ImageData, ModelManifest, OcrJob, OcrLocalBackend, OcrProvider, OcrResult, OcrTextBlock, Point,
    Rect, Size,
};

use crate::{OcrEngine, OcrEngineError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockOcrEngine {
    provider: OcrProvider,
    loaded_model_id: Option<String>,
}

impl Default for MockOcrEngine {
    fn default() -> Self {
        Self {
            provider: OcrProvider::Local(OcrLocalBackend::Mnn),
            loaded_model_id: None,
        }
    }
}

impl OcrEngine for MockOcrEngine {
    fn provider(&self) -> OcrProvider {
        self.provider.clone()
    }

    fn load_model(&mut self, model: &ModelManifest) -> Result<(), OcrEngineError> {
        self.loaded_model_id = Some(model.id.clone());
        Ok(())
    }

    fn recognize(&self, job: &OcrJob, image: &ImageData) -> Result<OcrResult, OcrEngineError> {
        let bounds = job.source_rect.unwrap_or(Rect::new(
            Point::ZERO,
            Size::new(
                image.metadata.pixel_size.width.max(1.0),
                image.metadata.pixel_size.height.max(1.0),
            ),
        ));

        let model = self
            .loaded_model_id
            .as_deref()
            .or(job.model_id.as_deref())
            .unwrap_or("unloaded-model");

        let plain_text = format!("Mock OCR text from {} using {}", image.id.0, model);

        Ok(OcrResult {
            job_id: job.id.clone(),
            image_id: image.id.clone(),
            blocks: vec![OcrTextBlock {
                text: plain_text.clone(),
                bounds,
                confidence: Some(0.99),
                language: job.language_hint.clone().or_else(|| Some("en".to_owned())),
            }],
            plain_text,
        })
    }
}
