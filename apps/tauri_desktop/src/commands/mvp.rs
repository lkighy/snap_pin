use crate::shell_state::ShellState;
use shared_models::{
    CoreCommand, CoreEvent, ImageData, ImageFormat, ImageId, ImageMetadata, LanguageCode,
    OcrResult, OcrTextBlock, Point, Rect, Settings, Size, TranslateLocalBackend, TranslateProvider,
    TranslationRequest, TranslationResult,
};

pub fn start_capture(state: &mut ShellState) -> Vec<CoreEvent> {
    state.dispatch(CoreCommand::StartCapture)
}

pub fn cancel_capture(state: &mut ShellState) -> Vec<CoreEvent> {
    state.dispatch(CoreCommand::CancelCapture)
}

pub fn update_settings(state: &mut ShellState, settings: Settings) -> Vec<CoreEvent> {
    state.dispatch(CoreCommand::UpdateSettings(settings))
}

pub fn translate_selection(state: &mut ShellState, request: TranslationRequest) -> Vec<CoreEvent> {
    state.dispatch(CoreCommand::Translate { request })
}

pub fn run_mvp_capture_ocr_translate(state: &mut ShellState) -> Vec<CoreEvent> {
    let region = Rect::new(Point::new(32.0, 48.0), Size::new(640.0, 360.0));
    let image = ImageData {
        id: ImageId::new("mvp-capture-001"),
        metadata: ImageMetadata {
            id: ImageId::new("mvp-capture-001"),
            pixel_size: region.size,
            format: ImageFormat::Rgba8,
            monitor_name: Some("mock-monitor".to_owned()),
        },
        bytes: vec![255; (region.size.width * region.size.height * 4.0) as usize],
    };

    let mut events = state.dispatch(CoreCommand::StartCapture);
    events.extend(state.dispatch(CoreCommand::CompleteCapture {
        image: image.clone(),
        region,
    }));

    let job_id = "ocr-mvp-capture-001".to_owned();
    let request_id = format!("translate-{job_id}");
    let source_text = "Mock OCR text from mvp-capture-001 using mvp-mock-ocr".to_owned();
    let target_language = state.settings().translate.target_language.clone();
    let ocr_result = OcrResult {
        job_id: job_id.clone(),
        image_id: image.id.clone(),
        blocks: vec![OcrTextBlock {
            text: source_text.clone(),
            bounds: region,
            confidence: Some(0.99),
            language: Some("en".to_owned()),
        }],
        plain_text: source_text.clone(),
    };
    let translation_result = TranslationResult {
        request_id: request_id.clone(),
        source_text: source_text.clone(),
        translated_text: format!("[en -> {target_language} via mvp-mock-translate] {source_text}"),
        source_language: Some(LanguageCode::new("en")),
        target_language: LanguageCode::new(target_language),
        provider: TranslateProvider::Local(TranslateLocalBackend::CTranslate2),
    };

    state.record_mvp_results(ocr_result.clone(), translation_result.clone());
    events.push(CoreEvent::OcrQueued { job_id });
    events.push(CoreEvent::OcrCompleted { result: ocr_result });
    events.push(CoreEvent::TranslationQueued { request_id });
    events.push(CoreEvent::TranslationCompleted {
        result: translation_result,
    });

    events
}
