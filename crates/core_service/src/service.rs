use model_registry::ModelRegistry;
use ocr_engine::{MockOcrEngine, OcrEngine};
use shared_models::{
    CoreCommand, CoreEvent, ImageData, LanguageCode, OcrJob, Settings, TranslateProvider,
    TranslationRequest,
};
use translate_engine::{MockTranslateEngine, TranslateEngine};

use crate::{
    ClipboardManager, ConfigStore, HistoryStore, HotkeyManager, OcrCoordinator, PluginRegistry,
    ScreenshotCoordinator, TranslateCoordinator,
};

pub struct CoreService {
    settings: Settings,
    config: ConfigStore,
    history: HistoryStore,
    screenshot: ScreenshotCoordinator,
    ocr: OcrCoordinator,
    translate: TranslateCoordinator,
    ocr_engine: MockOcrEngine,
    translate_engine: MockTranslateEngine,
    models: ModelRegistry,
    hotkeys: HotkeyManager,
    clipboard: ClipboardManager,
    plugins: PluginRegistry,
}

impl Default for CoreService {
    fn default() -> Self {
        let settings = Settings::default();
        Self {
            config: ConfigStore::from_settings(settings.clone()),
            settings,
            history: HistoryStore::default(),
            screenshot: ScreenshotCoordinator::default(),
            ocr: OcrCoordinator::default(),
            translate: TranslateCoordinator::default(),
            ocr_engine: MockOcrEngine::default(),
            translate_engine: MockTranslateEngine::default(),
            models: ModelRegistry::with_builtin_defaults(),
            hotkeys: HotkeyManager::default(),
            clipboard: ClipboardManager::default(),
            plugins: PluginRegistry::default(),
        }
    }
}

impl CoreService {
    pub fn handle_command(&mut self, command: CoreCommand) -> Vec<CoreEvent> {
        log::info!("core handling command {}", command_name(&command));
        let events = match command {
            CoreCommand::StartCapture => vec![self.screenshot.start_capture()],
            CoreCommand::CancelCapture => vec![self.screenshot.cancel_capture()],
            CoreCommand::CompleteCapture { image, region } => self.complete_capture(image, region),
            CoreCommand::PinImage { image_id, bounds } => {
                vec![self.screenshot.pin_image(image_id, bounds)]
            }
            CoreCommand::RunOcr { job } => self.run_ocr(job),
            CoreCommand::Translate { request } => self.run_translation(request),
            CoreCommand::RunOcrAndTranslate {
                job,
                target_language,
            } => self.run_ocr_and_translate(job, target_language),
            CoreCommand::RegisterModel(manifest) => {
                let model_id = manifest.id.clone();
                self.models.register(manifest);
                vec![CoreEvent::ModelRegistered { model_id }]
            }
            CoreCommand::UpdateSettings(settings) => {
                self.settings = settings.clone();
                self.config.replace(settings);
                vec![CoreEvent::SettingsUpdated]
            }
        };

        for event in &events {
            log_core_event(event);
        }
        events
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn history(&self) -> &HistoryStore {
        &self.history
    }

    pub fn models(&self) -> &ModelRegistry {
        &self.models
    }

    pub fn capabilities(&self) -> Vec<&'static str> {
        vec![
            "screenshot",
            "ocr",
            "translate",
            "hotkeys",
            "clipboard",
            "plugins",
        ]
    }

    pub fn managers_ready(&self) -> bool {
        self.config.is_loaded()
            && self.hotkeys.is_enabled()
            && self.clipboard.is_available()
            && self.plugins.is_loaded()
    }

    fn complete_capture(
        &mut self,
        image: ImageData,
        region: shared_models::Rect,
    ) -> Vec<CoreEvent> {
        let image_id = image.id.clone();
        log::info!(
            "core storing captured image id={} bytes={} region={:?}",
            image_id.0,
            image.bytes.len(),
            region
        );
        self.screenshot.store_image(image);
        vec![CoreEvent::CaptureFinished { image_id, region }]
    }

    fn run_ocr(&mut self, mut job: OcrJob) -> Vec<CoreEvent> {
        log::info!(
            "core starting ocr job={} image={} provider={:?}",
            job.id,
            job.image_id.0,
            job.provider
        );
        let mut events = vec![self.ocr.enqueue(job.clone())];
        self.apply_default_ocr_model(&mut job);

        let Some(image) = self.screenshot.image(&job.image_id) else {
            log::error!("core ocr image missing image={}", job.image_id.0);
            events.push(CoreEvent::Error {
                code: "image_not_found".to_owned(),
                message: format!("image '{}' is not available for OCR", job.image_id.0),
            });
            return events;
        };

        match self.ocr_engine.recognize(&job, image) {
            Ok(result) => {
                if self.settings.history.enabled {
                    self.history.push_ocr(result.clone());
                }
                log::info!(
                    "core ocr completed job={} blocks={} text_chars={}",
                    result.job_id,
                    result.blocks.len(),
                    result.plain_text.chars().count()
                );
                events.push(CoreEvent::OcrCompleted { result });
            }
            Err(error) => {
                log::error!(
                    "core ocr failed code={} message={}",
                    error.code,
                    error.message
                );
                events.push(CoreEvent::Error {
                    code: error.code,
                    message: error.message,
                });
            }
        }

        events
    }

    fn run_translation(&mut self, mut request: TranslationRequest) -> Vec<CoreEvent> {
        log::info!(
            "core starting translation request={} provider={:?} target={} source_chars={}",
            request.id,
            request.provider,
            request.target_language.0,
            request.source_text.chars().count()
        );
        let mut events = vec![self.translate.enqueue(request.clone())];
        self.apply_default_translation_model(&mut request);

        match self.translate_engine.translate(&request) {
            Ok(result) => {
                if self.settings.history.enabled {
                    self.history.push_translation(result.clone());
                }
                log::info!(
                    "core translation completed request={} translated_chars={}",
                    result.request_id,
                    result.translated_text.chars().count()
                );
                events.push(CoreEvent::TranslationCompleted { result });
            }
            Err(error) => {
                log::error!(
                    "core translation failed code={} message={}",
                    error.code,
                    error.message
                );
                events.push(CoreEvent::Error {
                    code: error.code,
                    message: error.message,
                });
            }
        }

        events
    }

    fn run_ocr_and_translate(&mut self, job: OcrJob, target_language: String) -> Vec<CoreEvent> {
        log::info!(
            "core starting ocr and translation job={} target={}",
            job.id,
            target_language
        );
        let mut events = self.run_ocr(job);
        let Some(ocr_result) = events.iter().find_map(|event| match event {
            CoreEvent::OcrCompleted { result } => Some(result.clone()),
            _ => None,
        }) else {
            log::error!("core ocr and translation stopped because ocr did not complete");
            return events;
        };

        let request = TranslationRequest {
            id: format!("translate-{}", ocr_result.job_id),
            source_text: ocr_result.plain_text,
            source_language: ocr_result
                .blocks
                .first()
                .and_then(|block| block.language.clone())
                .map(LanguageCode),
            target_language: LanguageCode(target_language),
            provider: self.settings.translate.provider.clone(),
            model_id: None,
            context: Some(format!("ocr_job:{}", ocr_result.job_id)),
        };

        events.extend(self.run_translation(request));
        events
    }

    fn apply_default_ocr_model(&mut self, job: &mut OcrJob) {
        if job.model_id.is_none() {
            job.model_id = self.models.recommended_ocr().map(|model| model.id.clone());
        }

        if let Some(model) = job.model_id.as_deref().and_then(|id| self.models.find(id)) {
            log::info!("core loading ocr model id={}", model.id);
            let _ = self.ocr_engine.load_model(model);
        }
    }

    fn apply_default_translation_model(&mut self, request: &mut TranslationRequest) {
        if request.model_id.is_none() && matches!(request.provider, TranslateProvider::Local(_)) {
            request.model_id = self
                .models
                .recommended_translation(
                    request
                        .source_language
                        .as_ref()
                        .map(|language| language.0.as_str()),
                    &request.target_language.0,
                )
                .map(|model| model.id.clone());
        }

        if let Some(model) = request
            .model_id
            .as_deref()
            .and_then(|id| self.models.find(id))
        {
            log::info!("core loading translation model id={}", model.id);
            let _ = self.translate_engine.load_model(model);
        }
    }
}

fn command_name(command: &CoreCommand) -> &'static str {
    match command {
        CoreCommand::StartCapture => "start_capture",
        CoreCommand::CancelCapture => "cancel_capture",
        CoreCommand::CompleteCapture { .. } => "complete_capture",
        CoreCommand::PinImage { .. } => "pin_image",
        CoreCommand::RunOcr { .. } => "run_ocr",
        CoreCommand::Translate { .. } => "translate",
        CoreCommand::RunOcrAndTranslate { .. } => "run_ocr_and_translate",
        CoreCommand::RegisterModel(_) => "register_model",
        CoreCommand::UpdateSettings(_) => "update_settings",
    }
}

fn log_core_event(event: &CoreEvent) {
    match event {
        CoreEvent::CaptureFinished { image_id, region } => {
            log::info!(
                "core produced capture_finished image={} region={region:?}",
                image_id.0
            );
        }
        CoreEvent::ImagePinned { image_id } => {
            log::info!("core produced image_pinned image={}", image_id.0);
        }
        CoreEvent::OcrQueued { job_id } => {
            log::info!("core produced ocr_queued job={job_id}");
        }
        CoreEvent::OcrCompleted { result } => {
            log::info!(
                "core produced ocr_completed job={} blocks={}",
                result.job_id,
                result.blocks.len()
            );
        }
        CoreEvent::TranslationQueued { request_id } => {
            log::info!("core produced translation_queued request={request_id}");
        }
        CoreEvent::TranslationCompleted { result } => {
            log::info!(
                "core produced translation_completed request={}",
                result.request_id
            );
        }
        CoreEvent::ModelRegistered { model_id } => {
            log::info!("core produced model_registered model={model_id}");
        }
        CoreEvent::Error { code, message } => {
            log::error!("core produced error code={code} message={message}");
        }
        CoreEvent::CaptureStarted => log::info!("core produced capture_started"),
        CoreEvent::CaptureCanceled => log::info!("core produced capture_canceled"),
        CoreEvent::SettingsUpdated => log::info!("core produced settings_updated"),
    }
}

#[cfg(test)]
mod tests {
    use shared_models::{
        CoreCommand, CoreEvent, ImageData, ImageFormat, ImageId, ImageMetadata, OcrJob,
        OcrLocalBackend, OcrProvider, Point, Rect, Size,
    };

    use super::CoreService;

    #[test]
    fn runs_capture_ocr_and_translation_flow() {
        let mut service = CoreService::default();
        let region = Rect::new(Point::ZERO, Size::new(100.0, 60.0));
        let image = ImageData {
            id: ImageId::new("test-image"),
            metadata: ImageMetadata {
                id: ImageId::new("test-image"),
                pixel_size: region.size,
                format: ImageFormat::Rgba8,
                monitor_name: None,
            },
            bytes: vec![0; 100 * 60 * 4],
        };

        service.handle_command(CoreCommand::CompleteCapture {
            image: image.clone(),
            region,
        });

        let events = service.handle_command(CoreCommand::RunOcrAndTranslate {
            job: OcrJob {
                id: "test-job".to_owned(),
                image_id: image.id,
                source_rect: Some(region),
                language_hint: Some("en".to_owned()),
                provider: OcrProvider::Local(OcrLocalBackend::Mnn),
                model_id: None,
            },
            target_language: "zh-CN".to_owned(),
        });

        assert!(
            events
                .iter()
                .any(|event| matches!(event, CoreEvent::OcrCompleted { .. }))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, CoreEvent::TranslationCompleted { .. }))
        );
        assert_eq!(service.history().ocr_results().len(), 1);
        assert_eq!(service.history().translations().len(), 1);
    }
}
