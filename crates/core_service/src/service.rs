use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use model_registry::ModelRegistry;
use ocr_engine::{OcrEngine, OcrEngineError, RoutedOcrEngine};
use perf_trace::{PerfSpan, log_elapsed};
use platform_api::{AppPlatform, PlatformCapabilities};
use shared_models::{
    CoreCommand, CoreEvent, ImageData, LanguageCode, OcrJob, OcrProvider, Settings,
    TranslateProvider, TranslationRequest,
};
use translate_engine::{RoutedTranslateEngine, TranslateEngine, TranslateEngineError};

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
    translate_engine: RoutedTranslateEngine,
    models: ModelRegistry,
    hotkeys: HotkeyManager,
    clipboard: ClipboardManager,
    plugins: PluginRegistry,
    platform: Option<Arc<dyn AppPlatform>>,
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
            translate_engine: RoutedTranslateEngine::default(),
            models: ModelRegistry::with_builtin_defaults(),
            hotkeys: HotkeyManager::default(),
            clipboard: ClipboardManager::default(),
            plugins: PluginRegistry::default(),
            platform: None,
        }
    }
}

impl CoreService {
    pub fn with_platform(platform: Arc<dyn AppPlatform>) -> Self {
        let mut service = Self::default();
        service.platform = Some(platform);
        service
    }

    pub fn set_platform(&mut self, platform: Arc<dyn AppPlatform>) {
        self.platform = Some(platform);
    }

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

    pub fn models_mut(&mut self) -> &mut ModelRegistry {
        &mut self.models
    }

    pub fn capabilities(&self) -> Vec<&'static str> {
        vec![
            "screenshot",
            "ocr",
            ocr_engine::local_runtime_status(),
            "translate",
            translate_engine::local_runtime_status(),
            "hotkeys",
            "clipboard",
            "plugins",
        ]
    }

    pub fn platform_capabilities(&self) -> PlatformCapabilities {
        self.platform.as_ref().map_or_else(
            || PlatformCapabilities::unavailable("platform runtime is not initialized"),
            |platform| platform.capabilities(),
        )
    }

    pub fn local_ocr_runtime_status(&self) -> &'static str {
        ocr_engine::local_runtime_status()
    }

    pub fn local_translate_runtime_status(&self) -> &'static str {
        translate_engine::local_runtime_status()
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
        let mut span = PerfSpan::new("core_run_ocr_prepare_total")
            .field("provider", ocr_provider_label(&job.provider));
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
        let model_start = std::time::Instant::now();
        self.apply_default_ocr_model(&mut job);
        log_elapsed("core_ocr_apply_default_model", model_start);
        let profiles_start = std::time::Instant::now();
        self.ocr_engine
            .configure_provider_profiles(&self.settings.ocr.provider_profiles);
        log_elapsed("core_ocr_configure_provider_profiles", profiles_start);

        let image_lookup_start = std::time::Instant::now();
        let Some(image) = self.screenshot.image(&job.image_id) else {
            log::error!("core ocr image missing image={}", job.image_id.0);
            events.push(CoreEvent::Error {
                code: "image_not_found".to_owned(),
                message: format!("image '{}' is not available for OCR", job.image_id.0),
            });
            return events;
        };
        log_elapsed("core_ocr_lookup_image", image_lookup_start);

        let image = image.clone();
        span.add_field("image_bytes", image.bytes.len());
        span.add_field("width", image.metadata.pixel_size.width.round().max(1.0));
        span.add_field("height", image.metadata.pixel_size.height.round().max(1.0));
        let history = Arc::clone(&self.history);
        let save_history = self.settings.history.enabled;
        let sender = self.ocr.completion_sender();
        let engine = self.ocr_engine.clone();
        let platform = self.platform.clone();
        let spawn_start = std::time::Instant::now();
        thread::spawn(move || {
            let worker_span = PerfSpan::new("core_ocr_worker_total")
                .field("provider", ocr_provider_label(&job.provider))
                .field("image_bytes", image.bytes.len());
            let recognize_start = std::time::Instant::now();
            let event = match recognize_ocr_job(&engine, platform, &job, &image) {
                Ok(result) => {
                    log_elapsed("core_ocr_worker_recognize", recognize_start);
                    if save_history {
                        let history_start = std::time::Instant::now();
                        history
                            .lock()
                            .expect("history lock poisoned")
                            .push_ocr(result.clone());
                        log_elapsed("core_ocr_worker_save_history", history_start);
                    }
                    CoreEvent::OcrCompleted { result }
                }
                Err(error) => {
                    log_elapsed("core_ocr_worker_recognize", recognize_start);
                    CoreEvent::Error {
                        code: error.code,
                        message: error.message,
                    }
                }
            };
            let send_start = std::time::Instant::now();
            let _ = sender.send(event);
            log_elapsed("core_ocr_worker_send_event", send_start);
            worker_span.finish();
        });
        log_elapsed("core_ocr_spawn_worker", spawn_start);
        span.finish();

        events
    }

    fn run_translation(&mut self, mut request: TranslationRequest) -> Vec<CoreEvent> {
        let mut span = PerfSpan::new("core_run_translation_prepare_total")
            .field("provider", translate_provider_label(&request.provider))
            .field("target", &request.target_language.0)
            .field("source_chars", request.source_text.chars().count());
        log::info!(
            "core starting translation request={} provider={:?} target={} source_chars={}",
            request.id,
            request.provider,
            request.target_language.0,
            request.source_text.chars().count()
        );
        let mut events = vec![self.translate.enqueue(request.clone())];

        let model_start = std::time::Instant::now();
        if let Err(error) = self.apply_default_translation_model(&mut request) {
            log_elapsed("core_translation_apply_default_model", model_start);
            log::error!(
                "core failed to prepare translation request={} code={} message={}",
                request.id,
                error.code,
                error.message
            );
            events.push(CoreEvent::Error {
                code: error.code,
                message: error.message,
            });
            return events;
        }
        log_elapsed("core_translation_apply_default_model", model_start);

        let history = Arc::clone(&self.history);
        let save_history = self.settings.history.enabled;
        let sender = self.translate.completion_sender();
        let engine = self.translate_engine.clone();
        span.add_field("model_id", request.model_id.as_deref().unwrap_or("none"));
        let spawn_start = std::time::Instant::now();
        thread::spawn(move || {
            let worker_span = PerfSpan::new("core_translation_worker_total")
                .field("provider", translate_provider_label(&request.provider))
                .field("target", &request.target_language.0)
                .field("source_chars", request.source_text.chars().count());
            let translate_start = std::time::Instant::now();
            let event = match engine.translate(&request) {
                Ok(result) => {
                    log_elapsed("core_translation_worker_translate", translate_start);
                    if save_history {
                        let history_start = std::time::Instant::now();
                        history
                            .lock()
                            .expect("history lock poisoned")
                            .push_translation(result.clone());
                        log_elapsed("core_translation_worker_save_history", history_start);
                    }
                    log::info!(
                        "core translation completed request={} translated_chars={}",
                        result.request_id,
                        result.translated_text.chars().count()
                    );
                    CoreEvent::TranslationCompleted { result }
                }
                Err(error) => {
                    log_elapsed("core_translation_worker_translate", translate_start);
                    log::error!(
                        "core translation failed code={} message={}",
                        error.code,
                        error.message
                    );
                    CoreEvent::Error {
                        code: error.code,
                        message: error.message,
                    }
                }
            };
            let send_start = std::time::Instant::now();
            let _ = sender.send(event);
            log_elapsed("core_translation_worker_send_event", send_start);
            worker_span.finish();
        });
        log_elapsed("core_translation_spawn_worker", spawn_start);
        span.finish();

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

        events.extend(self.translate.drain_completed());

        events
    }

    fn apply_default_ocr_model(&mut self, job: &mut OcrJob) {
        let mut span = PerfSpan::new("core_apply_default_ocr_model_total")
            .field("provider", ocr_provider_label(&job.provider));
        if !matches!(job.provider, OcrProvider::Local(_)) {
            span.finish();
            return;
        }

        if job.model_id.is_none() {
            let select_start = std::time::Instant::now();
            job.model_id = self
                .settings
                .ocr
                .default_model_id
                .clone()
                .or_else(|| self.models.recommended_ocr().map(|model| model.id.clone()));
            log_elapsed("core_select_default_ocr_model", select_start);
        }

        if let Some(model) = job.model_id.as_deref().and_then(|id| self.models.find(id)) {
            log::info!("core loading ocr model id={}", model.id);
            let load_start = std::time::Instant::now();
            if let Err(error) = self.ocr_engine.load_model(model) {
                log::error!(
                    "core failed to load ocr model id={} code={} message={}",
                    model.id,
                    error.code,
                    error.message
                );
            }
            log_elapsed("core_load_ocr_model", load_start);
            span.add_field("model_id", &model.id);
        }
        span.finish();
    }

    fn apply_default_translation_model(
        &mut self,
        request: &mut TranslationRequest,
    ) -> Result<(), TranslateEngineError> {
        let mut span = PerfSpan::new("core_apply_default_translation_model_total")
            .field("provider", translate_provider_label(&request.provider))
            .field("target", &request.target_language.0);
        if !matches!(request.provider, TranslateProvider::Local(_)) {
            span.finish();
            return Ok(());
        }

        if request.model_id.is_none() && matches!(request.provider, TranslateProvider::Local(_)) {
            let select_start = std::time::Instant::now();
            request.model_id = self
                .settings
                .translate
                .default_model_id
                .clone()
                .or_else(|| {
                    self.models
                        .recommended_translation(
                            request
                                .source_language
                                .as_ref()
                                .map(|language| language.0.as_str()),
                            &request.target_language.0,
                        )
                        .map(|model| model.id.clone())
                });
            log_elapsed("core_select_default_translation_model", select_start);
        }

        let Some(model) = request
            .model_id
            .as_deref()
            .and_then(|id| self.models.find(id))
        else {
            return Err(TranslateEngineError::new(
                "translation_model_not_found",
                format!(
                    "no local translation model is available for '{} -> {}'",
                    request
                        .source_language
                        .as_ref()
                        .map(|language| language.0.as_str())
                        .unwrap_or("auto"),
                    request.target_language.0
                ),
            ));
        };

        log::info!("core loading translation model id={}", model.id);
        let load_start = std::time::Instant::now();
        let result = self.translate_engine.load_model(model);
        log_elapsed("core_load_translation_model", load_start);
        span.add_field("model_id", &model.id);
        if result.is_ok() {
            span.finish();
        }
        result
    }
}

fn recognize_ocr_job(
    engine: &RoutedOcrEngine,
    platform: Option<Arc<dyn AppPlatform>>,
    job: &OcrJob,
    image: &ImageData,
) -> Result<shared_models::OcrResult, OcrEngineError> {
    if job.provider != OcrProvider::System {
        return engine.recognize(job, image);
    }

    let Some(platform) = platform else {
        return Err(OcrEngineError::new(
            "system_ocr_unavailable",
            "system OCR requires an injected platform runtime",
        ));
    };

    platform
        .system_ocr()
        .recognize(job, image)
        .map_err(|error| OcrEngineError::new(error.code, error.message))
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

fn ocr_provider_label(provider: &OcrProvider) -> &'static str {
    match provider {
        OcrProvider::Disabled => "disabled",
        OcrProvider::System => "system",
        OcrProvider::Local(shared_models::OcrLocalBackend::Mnn) => "local-mnn",
        OcrProvider::Local(shared_models::OcrLocalBackend::OnnxRuntime) => "local-onnx",
        OcrProvider::Local(shared_models::OcrLocalBackend::PaddleRuntime) => "local-paddle",
        OcrProvider::Local(shared_models::OcrLocalBackend::Custom(_)) => "local-custom",
        OcrProvider::ExternalApi(shared_models::OcrExternalProvider::OpenAi) => "api-openai",
        OcrProvider::ExternalApi(shared_models::OcrExternalProvider::AzureVision) => "api-azure",
        OcrProvider::ExternalApi(shared_models::OcrExternalProvider::GoogleVision) => "api-google",
        OcrProvider::ExternalApi(shared_models::OcrExternalProvider::BaiduOcr) => "api-baidu",
        OcrProvider::ExternalApi(shared_models::OcrExternalProvider::TencentOcr) => "api-tencent",
        OcrProvider::ExternalApi(shared_models::OcrExternalProvider::Custom(_)) => "api-custom",
    }
}

fn translate_provider_label(provider: &TranslateProvider) -> &'static str {
    match provider {
        TranslateProvider::Disabled => "disabled",
        TranslateProvider::Local(shared_models::TranslateLocalBackend::CTranslate2) => "local-ct2",
        TranslateProvider::Local(shared_models::TranslateLocalBackend::Custom(_)) => "local-custom",
        TranslateProvider::ExternalApi(shared_models::TranslateExternalProvider::DeepL) => {
            "api-deepl"
        }
        TranslateProvider::ExternalApi(shared_models::TranslateExternalProvider::Google) => {
            "api-google"
        }
        TranslateProvider::ExternalApi(shared_models::TranslateExternalProvider::Azure) => {
            "api-azure"
        }
        TranslateProvider::ExternalApi(shared_models::TranslateExternalProvider::OpenAi) => {
            "api-openai"
        }
        TranslateProvider::ExternalApi(shared_models::TranslateExternalProvider::Baidu) => {
            "api-baidu"
        }
        TranslateProvider::ExternalApi(shared_models::TranslateExternalProvider::Tencent) => {
            "api-tencent"
        }
        TranslateProvider::ExternalApi(shared_models::TranslateExternalProvider::CustomHttp) => {
            "api-custom"
        }
        TranslateProvider::ExternalApi(shared_models::TranslateExternalProvider::Custom(_)) => {
            "api-custom"
        }
        TranslateProvider::Experimental(_) => "experimental",
        TranslateProvider::Custom(_) => "custom",
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
        CoreCommand, CoreEvent, ImageData, ImageFormat, ImageId, ImageMetadata, LanguageCode,
        ModelDomain, ModelFile, ModelManifest, ModelSource, OcrJob, OcrLocalBackend, OcrProvider,
        Point, Rect, Settings, Size, TranslateLocalBackend, TranslateProvider, TranslationRequest,
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

    #[test]
    fn local_translation_reports_disabled_runtime_without_mock_result() {
        let mut service = CoreService::default();
        let model_root = std::env::temp_dir().join(format!(
            "snap-pin-core-translation-model-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&model_root).unwrap();
        std::fs::write(model_root.join("model.bin"), [1]).unwrap();
        std::fs::write(model_root.join("config.json"), [2]).unwrap();
        std::fs::write(model_root.join("source.spm"), [3]).unwrap();
        std::fs::write(model_root.join("target.spm"), [4]).unwrap();

        service.handle_command(CoreCommand::RegisterModel(translation_manifest(
            ModelSource::LocalPath(model_root.to_string_lossy().into_owned()),
        )));

        let events = service.handle_command(CoreCommand::Translate {
            request: TranslationRequest {
                id: "translate-test".to_owned(),
                source_text: "hello".to_owned(),
                source_language: Some(LanguageCode::new("en")),
                target_language: LanguageCode::new("zh-CN"),
                provider: TranslateProvider::Local(TranslateLocalBackend::CTranslate2),
                model_id: Some("test-opus-mt-en-zh-ct2-int8".to_owned()),
                context: None,
            },
        });

        assert!(events.iter().any(|event| {
            matches!(
                event,
                CoreEvent::TranslationQueued { request_id } if request_id == "translate-test"
            )
        }));
        assert!(
            !events
                .iter()
                .any(|event| { matches!(event, CoreEvent::TranslationCompleted { .. }) })
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
                CoreEvent::Error { code, .. } if code == "local_translate_runtime_disabled"
            )
        }));

        let _ = std::fs::remove_dir_all(model_root);
    }

    #[test]
    fn local_translation_requires_imported_model_files() {
        let mut service = CoreService::default();

        let events = service.handle_command(CoreCommand::Translate {
            request: TranslationRequest {
                id: "translate-test".to_owned(),
                source_text: "hello".to_owned(),
                source_language: Some(LanguageCode::new("en")),
                target_language: LanguageCode::new("zh-CN"),
                provider: TranslateProvider::Local(TranslateLocalBackend::CTranslate2),
                model_id: Some("opus-mt-en-zh-ct2-int8".to_owned()),
                context: None,
            },
        });

        assert!(events.iter().any(|event| {
            matches!(
                event,
                CoreEvent::Error { code, .. } if code == "translation_model_not_installed"
            )
        }));
    }

    fn translation_manifest(source: ModelSource) -> ModelManifest {
        ModelManifest {
            id: "test-opus-mt-en-zh-ct2-int8".to_owned(),
            name: "Test OPUS-MT English to Chinese CTranslate2 int8".to_owned(),
            domain: ModelDomain::Translation,
            family: "opus-mt".to_owned(),
            backend: "ctranslate2".to_owned(),
            version: "marian".to_owned(),
            source_languages: vec!["en".to_owned()],
            target_languages: vec!["zh-CN".to_owned()],
            quantization: Some("int8".to_owned()),
            low_spec_friendly: true,
            multilingual: false,
            source,
            files: vec![
                ModelFile::required("model", "model.bin"),
                ModelFile::required("config", "config.json"),
                ModelFile::required("source_tokenizer", "source.spm"),
                ModelFile::required("target_tokenizer", "target.spm"),
            ],
        }
    }
}
