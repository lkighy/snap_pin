use shared_models::{ImageData, OcrJob, OcrResult};

use perf_trace::PerfSpan;

#[cfg(feature = "local-ocr-rs")]
use perf_trace::log_elapsed;

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
    let mut span = PerfSpan::new("ocr_rs_backend_recognize_total")
        .field("model_id", &bundle.manifest.id)
        .field("image_bytes", image.bytes.len());
    let decode_start = std::time::Instant::now();
    let input = decode_image(image)?;
    log_elapsed("ocr_rs_backend_decode_image", decode_start);
    let engine_start = std::time::Instant::now();
    let engine = ocr_rs::OcrEngine::new(&bundle.det, &bundle.rec, &bundle.keys, None)
        .map_err(|error| OcrEngineError::new("local_ocr_engine_load_failed", error.to_string()))?;
    log_elapsed("ocr_rs_backend_create_engine", engine_start);
    let recognize_start = std::time::Instant::now();
    let raw_results = engine
        .recognize(&input)
        .map_err(|error| OcrEngineError::new("local_ocr_failed", error.to_string()))?;
    log_elapsed("ocr_rs_backend_native_recognize", recognize_start);

    let normalize_start = std::time::Instant::now();
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
    span.add_field("blocks", blocks.len());
    let result = normalized_result(job, image, blocks);
    log_elapsed("ocr_rs_backend_normalize_result", normalize_start);
    span.finish();

    Ok(result)
}

#[cfg(not(feature = "local-ocr-rs"))]
pub fn recognize(
    _bundle: &OcrModelBundle,
    _job: &OcrJob,
    _image: &ImageData,
) -> Result<OcrResult, OcrEngineError> {
    let span = PerfSpan::new("ocr_rs_backend_recognize_total").field("runtime", "disabled");
    span.finish();
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
