use crate::shell_state::ShellState;
use shared_models::{
    CoreCommand, CoreEvent, ImageData, ImageFormat, ImageId, ImageMetadata, OcrJob, Point, Rect,
    Settings, Size, TranslationRequest,
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
    events.extend(state.dispatch(CoreCommand::RunOcrAndTranslate {
        job: OcrJob {
            id: "ocr-mvp-capture-001".to_owned(),
            image_id: image.id,
            source_rect: Some(region),
            language_hint: Some("en".to_owned()),
            provider: state.settings().ocr.provider.clone(),
            model_id: None,
        },
        target_language: state.settings().translate.target_language.clone(),
    }));

    events
}
