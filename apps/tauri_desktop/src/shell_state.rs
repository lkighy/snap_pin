use core_service::CoreService;
use shared_models::{CoreCommand, CoreEvent, Settings};
use tauri::AppHandle;

use crate::ipc::bridge::DesktopIpcBridge;
use crate::settings::store;

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
    pub fn from_store(app: &AppHandle) -> Self {
        let mut state = Self::default();
        if let Some(settings) = store::load(app) {
            log::info!("applying persisted settings");
            state.update_settings(Settings::from(settings));
        } else {
            log::info!("using default settings");
        }
        state
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
        format!(
            "history: {} ocr result(s), {} translation(s)",
            self.core.history().ocr_results().len(),
            self.core.history().translations().len()
        )
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
        CoreCommand::Translate { .. } => "translate",
        CoreCommand::RunOcrAndTranslate { .. } => "run_ocr_and_translate",
        CoreCommand::UpdateSettings(_) => "update_settings",
        CoreCommand::RegisterModel(_) => "register_model",
    }
}

fn event_name(event: &CoreEvent) -> &'static str {
    match event {
        CoreEvent::CaptureStarted => "capture_started",
        CoreEvent::CaptureCanceled => "capture_canceled",
        CoreEvent::CaptureFinished { .. } => "capture_finished",
        CoreEvent::ImagePinned { .. } => "image_pinned",
        CoreEvent::OcrQueued { .. } => "ocr_queued",
        CoreEvent::OcrCompleted { .. } => "ocr_completed",
        CoreEvent::TranslationQueued { .. } => "translation_queued",
        CoreEvent::TranslationCompleted { .. } => "translation_completed",
        CoreEvent::ModelRegistered { .. } => "model_registered",
        CoreEvent::SettingsUpdated => "settings_updated",
        CoreEvent::Error { .. } => "error",
    }
}
