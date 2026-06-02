use shared_models::{ImageData, OcrJob, OcrResult};

use crate::{OcrEngineError, OcrModelBundle};

#[cfg(feature = "local-ocr-rs")]
use crate::decode_image;
#[cfg(feature = "local-ocr-rs")]
use shared_models::{OcrTextBlock, Point, Rect, Size};

#[cfg(feature = "local-ocr-rs")]
pub fn recognize(
    bundle: &OcrModelBundle,
    job: &OcrJob,
    image: &ImageData,
) -> Result<OcrResult, OcrEngineError> {
    let input = decode_image(image)?;
    let engine = ocr_rs::OcrEngine::new(&bundle.det, &bundle.rec, &bundle.keys, None)
        .map_err(|error| OcrEngineError::new("local_ocr_engine_load_failed", error.to_string()))?;
    let raw_results = engine
        .recognize(&input)
        .map_err(|error| OcrEngineError::new("local_ocr_failed", error.to_string()))?;

    let blocks = raw_results
        .into_iter()
        .map(|result| {
            let bounds = Rect::new(
                Point::new(
                    result.bbox.rect.left() as f32,
                    result.bbox.rect.top() as f32,
                ),
                Size::new(
                    result.bbox.rect.width() as f32,
                    result.bbox.rect.height() as f32,
                ),
            );

            OcrTextBlock {
                text: result.text,
                bounds,
                confidence: Some(result.confidence),
                language: job.language_hint.clone(),
            }
        })
        .collect::<Vec<_>>();

    Ok(normalized_result(job, image, blocks))
}

#[cfg(not(feature = "local-ocr-rs"))]
pub fn recognize(
    _bundle: &OcrModelBundle,
    _job: &OcrJob,
    _image: &ImageData,
) -> Result<OcrResult, OcrEngineError> {
    Err(OcrEngineError::new(
        "local_ocr_runtime_disabled",
        "local OCR runtime is not compiled; enable the 'local-ocr-rs' feature to use MNN OCR",
    ))
}

pub fn runtime_status() -> &'static str {
    if cfg!(feature = "local-ocr-rs") {
        "local-ocr-rs-enabled"
    } else {
        "local-ocr-rs-disabled"
    }
}

#[cfg(feature = "local-ocr-rs")]
fn normalized_result(job: &OcrJob, image: &ImageData, blocks: Vec<OcrTextBlock>) -> OcrResult {
    let plain_text = blocks
        .iter()
        .map(|block| block.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    OcrResult {
        job_id: job.id.clone(),
        image_id: image.id.clone(),
        blocks,
        plain_text,
    }
}
