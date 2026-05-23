use crate::{OcrProvider, TranslateProvider};

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub interface: InterfaceSettings,
    pub capture: CaptureSettings,
    pub overlay: OverlaySettings,
    pub pin: PinSettings,
    pub ocr: OcrSettings,
    pub translate: TranslateSettings,
    pub hotkeys: HotkeySettings,
    pub history: HistorySettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            interface: InterfaceSettings::default(),
            capture: CaptureSettings::default(),
            overlay: OverlaySettings::default(),
            pin: PinSettings::default(),
            ocr: OcrSettings::default(),
            translate: TranslateSettings::default(),
            hotkeys: HotkeySettings::default(),
            history: HistorySettings::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceSettings {
    pub language: String,
}

impl Default for InterfaceSettings {
    fn default() -> Self {
        Self {
            language: "zh-CN".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureCompletionAction {
    Pin,
    CopyToClipboard,
    SaveToFile,
    OpenEditor,
}

impl Default for CaptureCompletionAction {
    fn default() -> Self {
        Self::Pin
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaptureSettings {
    pub include_cursor: bool,
    pub auto_copy_to_clipboard: bool,
    pub freeze_screen_on_capture: bool,
    pub show_size_label: bool,
    pub show_toolbar: bool,
    pub capture_delay_ms: u64,
    pub mask_opacity: f32,
    pub border_color: String,
    pub completion_action: CaptureCompletionAction,
}

impl Default for CaptureSettings {
    fn default() -> Self {
        Self {
            include_cursor: false,
            auto_copy_to_clipboard: true,
            freeze_screen_on_capture: true,
            show_size_label: true,
            show_toolbar: true,
            capture_delay_ms: 0,
            mask_opacity: 0.46,
            border_color: "#2f8aa3".to_owned(),
            completion_action: CaptureCompletionAction::Pin,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OverlaySettings {
    pub show_magnifier: bool,
    pub magnifier_scale: f32,
    pub default_pin_opacity: f32,
    pub click_through_pins: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PinSettings {
    pub default_opacity: f32,
    pub click_through: bool,
    pub always_on_top: bool,
    pub remember_position: bool,
    pub zoom_step: f32,
    pub show_ocr_text: bool,
    pub show_translation_text: bool,
}

impl Default for PinSettings {
    fn default() -> Self {
        Self {
            default_opacity: 1.0,
            click_through: false,
            always_on_top: true,
            remember_position: true,
            zoom_step: 0.1,
            show_ocr_text: true,
            show_translation_text: true,
        }
    }
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            show_magnifier: true,
            magnifier_scale: 2.0,
            default_pin_opacity: 1.0,
            click_through_pins: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrSettings {
    pub provider: OcrProvider,
    pub language_hint: Option<String>,
    pub auto_run_after_capture: bool,
}

impl Default for OcrSettings {
    fn default() -> Self {
        Self {
            provider: OcrProvider::Local(crate::OcrLocalBackend::Mnn),
            language_hint: None,
            auto_run_after_capture: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslateSettings {
    pub provider: TranslateProvider,
    pub target_language: String,
    pub auto_translate_after_ocr: bool,
}

impl Default for TranslateSettings {
    fn default() -> Self {
        Self {
            provider: TranslateProvider::Local(crate::TranslateLocalBackend::CTranslate2),
            target_language: "zh-CN".to_owned(),
            auto_translate_after_ocr: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeySettings {
    pub capture: String,
    pub toggle_pins_click_through: String,
    pub show_history: String,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            capture: "Ctrl+Shift+A".to_owned(),
            toggle_pins_click_through: "Ctrl+Shift+X".to_owned(),
            show_history: "Ctrl+Shift+H".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistorySettings {
    pub enabled: bool,
    pub max_entries: usize,
}

impl Default for HistorySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 500,
        }
    }
}
