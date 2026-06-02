use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use model_registry::ModelRegistry;
use ocr_engine::{OcrEngine, RoutedOcrEngine};
use shared_models::{
    CoreCommand, CoreEvent, ImageData, LanguageCode, OcrJob, OcrLocalBackend, OcrProvider,
    OcrRunMode, Settings, TranslateProvider, TranslationRequest,
};
use translate_engine::{MockTranslateEngine, TranslateEngine};

use crate::{
    ClipboardManager, ConfigStore, HistoryStore, HotkeyManager, OcrCoordinator, PluginRegistry,
    ScreenshotCoordinator, TranslateCoordinator,
};

pub struct CoreService {
    settings: Settings,
    config: ConfigStore,
    history: Arc<Mutex<HistoryStore>>,
    screenshot: ScreenshotCoordinator,
    ocr: OcrCoordinator,
    translate: TranslateCoordinator,
    pending_ocr_translations: HashMap<String, String>,
    ocr_engine: RoutedOcrEngine,
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
            history: Arc::new(Mutex::new(HistoryStore::default())),
            screenshot: ScreenshotCoordinator::default(),
            ocr: OcrCoordinator::default(),
            translate: TranslateCoordinator::default(),
            pending_ocr_translations: HashMap::new(),
            ocr_engine: RoutedOcrEngine::default(),
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
            CoreCommand::CancelOcr { job_id } => vec![self.ocr.cancel(job_id)],
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
            CoreCommand::ImportModel { manifest_path } => self.import_model(manifest_path),
            CoreCommand::UpdateSettings(settings) => {
                self.settings = settings.clone();
                self.config.replace(settings);
                vec![CoreEvent::SettingsUpdated]
            }
            CoreCommand::DrainEvents => self.drain_events(),
        };

        for event in &events {
            log_core_event(event);
        }
        events
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn history_snapshot(&self) -> HistoryStore {
        self.history.lock().expect("history lock poisoned").clone()
    }

    pub fn models(&self) -> &ModelRegistry {
        &self.models
    }

    pub fn capabilities(&self) -> Vec<&'static str> {
        vec![
            "screenshot",
            "ocr",
            ocr_engine::local_runtime_status(),
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
        let mut events = vec![CoreEvent::CaptureFinished {
            image_id: image_id.clone(),
            region,
        }];

        if self.settings.ocr.auto_run_after_capture {
            let job = OcrJob {
                id: format!("ocr-{}", image_id.0),
                image_id,
                source_rect: Some(region),
                language_hint: self.settings.ocr.language_hint.clone(),
                provider: self.settings.ocr.provider.clone(),
                provider_profile_id: self.settings.ocr.default_provider_profile_id.clone(),
                model_id: self.settings.ocr.default_model_id.clone(),
            };

            if self.settings.translate.auto_translate_after_ocr {
                events.extend(
                    self.run_ocr_and_translate(
                        job,
                        self.settings.translate.target_language.clone(),
                    ),
                );
            } else {
                events.extend(self.run_ocr(job));
            }
        }

        events
    }

    fn import_model(&mut self, manifest_path: String) -> Vec<CoreEvent> {
        match self.models.import_manifest_file(&manifest_path) {
            Ok(model) => vec![CoreEvent::ModelRegistered {
                model_id: model.id.clone(),
            }],
            Err(error) => vec![CoreEvent::Error {
                code: error.code,
                message: error.message,
            }],
        }
    }

    fn run_ocr(&mut self, mut job: OcrJob) -> Vec<CoreEvent> {
        self.apply_ocr_mode_defaults(&mut job);

        if job.provider == OcrProvider::Disabled {
            job.provider = self.settings.ocr.provider.clone();
        }
        if job.language_hint.is_none() {
            job.language_hint = self.settings.ocr.language_hint.clone();
        }
        if job.provider_profile_id.is_none() {
            job.provider_profile_id = self.settings.ocr.default_provider_profile_id.clone();
        }
        log::info!(
            "core starting ocr job={} image={} provider={:?}",
            job.id,
            job.image_id.0,
            job.provider
        );
        let mut events = vec![self.ocr.enqueue(job.clone())];
        self.apply_default_ocr_model(&mut job);
        self.ocr_engine
            .configure_provider_profiles(&self.settings.ocr.provider_profiles);

        let Some(image) = self.screenshot.image(&job.image_id) else {
            log::error!("core ocr image missing image={}", job.image_id.0);
            events.push(CoreEvent::Error {
                code: "image_not_found".to_owned(),
                message: format!("image '{}' is not available for OCR", job.image_id.0),
            });
            return events;
        };

        let image = image.clone();
        let history = Arc::clone(&self.history);
        let save_history = self.settings.history.enabled;
        let sender = self.ocr.completion_sender();
        let engine = self.ocr_engine.clone();
        thread::spawn(move || {
            let event = match engine.recognize(&job, &image) {
                Ok(result) => {
                    if save_history {
                        history
                            .lock()
                            .expect("history lock poisoned")
                            .push_ocr(result.clone());
                    }
                    CoreEvent::OcrCompleted { result }
                }
                Err(error) => CoreEvent::Error {
                    code: error.code,
                    message: error.message,
                },
            };
            let _ = sender.send(event);
        });

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
                    self.history
                        .lock()
                        .expect("history lock poisoned")
                        .push_translation(result.clone());
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
        self.pending_ocr_translations
            .insert(job.id.clone(), target_language);
        self.run_ocr(job)
    }

    fn drain_events(&mut self) -> Vec<CoreEvent> {
        let mut events = self
            .ocr
            .drain_completed()
            .into_iter()
            .filter(|event| match event {
                CoreEvent::OcrCompleted { result } => !self.ocr.is_canceled(&result.job_id),
                _ => true,
            })
            .collect::<Vec<_>>();

        let completed_ocr = events
            .iter()
            .filter_map(|event| match event {
                CoreEvent::OcrCompleted { result } => Some(result.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();

        for ocr_result in completed_ocr {
            let Some(target_language) = self.pending_ocr_translations.remove(&ocr_result.job_id)
            else {
                continue;
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
        }

        events
    }

    fn apply_default_ocr_model(&mut self, job: &mut OcrJob) {
        if job.model_id.is_none() {
            job.model_id = self.models.recommended_ocr().map(|model| model.id.clone());
        }

        if !matches!(job.provider, OcrProvider::Local(_)) {
            return;
        }

        if let Some(model) = job.model_id.as_deref().and_then(|id| self.models.find(id)) {
            log::info!("core loading ocr model id={}", model.id);
            if let Err(error) = self.ocr_engine.load_model(model) {
                log::error!(
                    "core failed to load ocr model id={} code={} message={}",
                    model.id,
                    error.code,
                    error.message
                );
            }
        }
    }

    fn apply_ocr_mode_defaults(&self, job: &mut OcrJob) {
        if matches!(job.provider, OcrProvider::System) {
            return;
        }

        match self.settings.ocr.mode {
            OcrRunMode::Lightweight => {
                job.provider = OcrProvider::Local(OcrLocalBackend::Mnn);
                job.model_id = self
                    .settings
                    .ocr
                    .default_model_id
                    .clone()
                    .or_else(|| Some("ppocr-v5-mobile-fp16-mnn".to_owned()));
            }
            OcrRunMode::Standard => {
                job.provider = OcrProvider::Local(OcrLocalBackend::Mnn);
                job.model_id = self
                    .settings
                    .ocr
                    .default_model_id
                    .clone()
                    .or_else(|| Some("ppocr-v5-mobile-mnn".to_owned()));
            }
            OcrRunMode::Compatible => {
                job.provider = OcrProvider::Local(OcrLocalBackend::Mnn);
                job.model_id = self
                    .settings
                    .ocr
                    .default_model_id
                    .clone()
                    .or_else(|| Some("ppocr-v4-mobile-mnn".to_owned()));
            }
            OcrRunMode::Cloud => {
                job.provider = self.settings.ocr.provider.clone();
                job.provider_profile_id = self.settings.ocr.default_provider_profile_id.clone();
            }
            OcrRunMode::Advanced => {}
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
        CoreCommand::CancelOcr { .. } => "cancel_ocr",
        CoreCommand::Translate { .. } => "translate",
        CoreCommand::RunOcrAndTranslate { .. } => "run_ocr_and_translate",
        CoreCommand::RegisterModel(_) => "register_model",
        CoreCommand::ImportModel { .. } => "import_model",
        CoreCommand::UpdateSettings(_) => "update_settings",
        CoreCommand::DrainEvents => "drain_events",
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
        CoreEvent::OcrCanceled { job_id } => {
            log::info!("core produced ocr_canceled job={job_id}");
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
    use std::thread;
    use std::time::Duration;

    use shared_models::{
        CoreCommand, CoreEvent, ImageData, ImageFormat, ImageId, ImageMetadata, OcrJob,
        OcrLocalBackend, OcrProvider, Point, Rect, Settings, Size,
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
                provider_profile_id: None,
                model_id: None,
            },
            target_language: "zh-CN".to_owned(),
        });

        assert!(
            events
                .iter()
                .any(|event| matches!(event, CoreEvent::OcrQueued { .. }))
        );

        let mut drained = Vec::new();
        for _ in 0..20 {
            thread::sleep(Duration::from_millis(10));
            drained.extend(service.handle_command(CoreCommand::DrainEvents));
            if drained
                .iter()
                .any(|event| matches!(event, CoreEvent::Error { .. }))
            {
                break;
            }
        }

        assert!(drained.iter().any(|event| {
            matches!(
                event,
                CoreEvent::Error { code, .. } if code == "local_ocr_runtime_disabled"
            )
        }));
    }

    #[test]
    fn auto_queues_ocr_after_capture_when_enabled() {
        let mut service = CoreService::default();
        let mut settings = Settings::default();
        settings.ocr.auto_run_after_capture = true;
        service.handle_command(CoreCommand::UpdateSettings(settings));

        let region = Rect::new(Point::ZERO, Size::new(20.0, 10.0));
        let image = ImageData {
            id: ImageId::new("auto-image"),
            metadata: ImageMetadata {
                id: ImageId::new("auto-image"),
                pixel_size: region.size,
                format: ImageFormat::Rgba8,
                monitor_name: None,
            },
            bytes: vec![0; 20 * 10 * 4],
        };

        let events = service.handle_command(CoreCommand::CompleteCapture { image, region });

        assert!(
            events
                .iter()
                .any(|event| matches!(event, CoreEvent::CaptureFinished { .. }))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, CoreEvent::OcrQueued { .. }))
        );
    }
}
