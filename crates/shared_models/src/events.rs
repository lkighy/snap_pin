use crate::{
    ImageData, ImageId, ModelManifest, OcrJob, OcrResult, Rect, Settings, TranslationRequest,
    TranslationResult,
};

#[derive(Debug, Clone, PartialEq)]
pub enum CoreCommand {
    StartCapture,
    CancelCapture,
    CompleteCapture {
        image: ImageData,
        region: Rect,
    },
    PinImage {
        image_id: ImageId,
        bounds: Rect,
    },
    RunOcr {
        job: OcrJob,
    },
    CancelOcr {
        job_id: String,
    },
    Translate {
        request: TranslationRequest,
    },
    RunOcrAndTranslate {
        job: OcrJob,
        target_language: String,
    },
    DrainEvents,
    ImportModel {
        manifest_path: String,
    },
    UpdateSettings(Settings),
    RegisterModel(ModelManifest),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CoreEvent {
    CaptureStarted,
    CaptureCanceled,
    CaptureFinished { image_id: ImageId, region: Rect },
    ImagePinned { image_id: ImageId },
    OcrQueued { job_id: String },
    OcrCanceled { job_id: String },
    OcrCompleted { result: OcrResult },
    TranslationQueued { request_id: String },
    TranslationCompleted { result: TranslationResult },
    ModelRegistered { model_id: String },
    SettingsUpdated,
    Error { code: String, message: String },
}
