use std::sync::Arc;

use core_service::CoreService;
use platform_api::AppPlatform;
use shared_models::{CoreCommand, CoreEvent, Settings};
use tauri::AppHandle;

use crate::ipc::bridge::DesktopIpcBridge;
use crate::settings::{models, store};

pub struct ShellState {
    core: CoreService,
    ipc: DesktopIpcBridge,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            core: CoreService::default(),
            ipc: DesktopIpcBridge::default(),
        }
    }
}

impl ShellState {
    pub fn with_platform(platform: Arc<dyn AppPlatform>) -> Self {
        Self {
            core: CoreService::with_platform(platform),
            ipc: DesktopIpcBridge::default(),
        }
    }

    pub fn from_store(app: &AppHandle) -> Self {
        let mut state = Self::default();
        state.load_from_store(app);
        state
    }

    pub fn from_store_with_platform(app: &AppHandle, platform: Arc<dyn AppPlatform>) -> Self {
        let mut state = Self::with_platform(platform);
        state.load_from_store(app);
        state
    }

    fn load_from_store(&mut self, app: &AppHandle) {
        if let Some(settings) = store::load(app) {
            log::info!("applying persisted settings");
            self.update_settings(Settings::from(settings));
        } else {
            log::info!("using default settings");
        }
        for model in models::load(app) {
            self.dispatch(CoreCommand::RegisterModel(model));
        }
    }

    pub fn boot_summary(&self) -> String {
        format!(
            "snap pin desktop shell ready: {}",
            self.core.capabilities().join(", ")
        )
    }

    pub fn dispatch(&mut self, command: CoreCommand) -> Vec<CoreEvent> {
        log::info!("dispatching core command {}", command_name(&command));
        let events = self.core.handle_command(command);
        for event in &events {
            match event {
                CoreEvent::Error { code, message } => {
                    log::error!("core event error code={code} message={message}");
                }
                _ => log::info!("core event {}", event_name(event)),
            }
        }
        events
    }

    pub fn settings(&self) -> &Settings {
        self.core.settings()
    }

    pub fn update_settings(&mut self, settings: Settings) -> Vec<CoreEvent> {
        self.dispatch(CoreCommand::UpdateSettings(settings))
    }

    pub fn history_summary(&self) -> String {
        let history = self.core.history_snapshot();
        format!(
            "history: {} ocr result(s), {} translation(s)",
            history.ocr_results().len(),
            history.translations().len()
        )
    }

    pub fn record_mvp_results(
        &mut self,
        ocr: shared_models::OcrResult,
        translation: shared_models::TranslationResult,
    ) {
        self.core.record_mvp_results(ocr, translation);
    }

    pub fn model_summary(&self) -> String {
        let model_ids = self
            .core
            .models()
            .list()
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        format!("models: {}", model_ids)
    }

    pub fn local_ocr_runtime_status(&self) -> &'static str {
        self.core.local_ocr_runtime_status()
    }

    pub fn local_translate_runtime_status(&self) -> &'static str {
        self.core.local_translate_runtime_status()
    }

    pub fn platform_capabilities(&self) -> platform_api::PlatformCapabilities {
        self.core.platform_capabilities()
    }

    pub fn model_manifests(&self) -> &[shared_models::ModelManifest] {
        self.core.models().list()
    }

    pub fn model_manifests_mut(&mut self) -> &mut Vec<shared_models::ModelManifest> {
        self.core.models_mut().list_mut()
    }

    pub fn ipc(&mut self) -> &mut DesktopIpcBridge {
        &mut self.ipc
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
        CoreCommand::UpdateSettings(_) => "update_settings",
        CoreCommand::RegisterModel(_) => "register_model",
        CoreCommand::ImportModel { .. } => "import_model",
        CoreCommand::DrainEvents => "drain_events",
    }
}

fn event_name(event: &CoreEvent) -> &'static str {
    match event {
        CoreEvent::CaptureStarted => "capture_started",
        CoreEvent::CaptureCanceled => "capture_canceled",
        CoreEvent::CaptureFinished { .. } => "capture_finished",
        CoreEvent::ImagePinned { .. } => "image_pinned",
        CoreEvent::OcrQueued { .. } => "ocr_queued",
        CoreEvent::OcrCanceled { .. } => "ocr_canceled",
        CoreEvent::OcrCompleted { .. } => "ocr_completed",
        CoreEvent::TranslationQueued { .. } => "translation_queued",
        CoreEvent::TranslationCompleted { .. } => "translation_completed",
        CoreEvent::ModelRegistered { .. } => "model_registered",
        CoreEvent::SettingsUpdated => "settings_updated",
        CoreEvent::Error { .. } => "error",
    }
}
