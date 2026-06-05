use crate::{OcrProvider, OcrProviderProfile, OcrRunMode, TranslateProvider};

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
    pub min_width: f32,
    pub min_height: f32,
    pub show_ocr_text: bool,
    pub show_translation_text: bool,
    pub ocr_text: OcrTextOverlaySettings,
}

impl Default for PinSettings {
    fn default() -> Self {
        Self {
            default_opacity: 1.0,
            click_through: false,
            always_on_top: true,
            remember_position: true,
            zoom_step: 0.1,
            min_width: 96.0,
            min_height: 72.0,
            show_ocr_text: true,
            show_translation_text: true,
            ocr_text: OcrTextOverlaySettings::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrTextOverlaySettings {
    pub font_height_ratio: f32,
    pub min_font_size: f32,
    pub max_font_size: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub interaction_padding_x: f32,
    pub interaction_padding_y: f32,
}

impl Default for OcrTextOverlaySettings {
    fn default() -> Self {
        Self {
            font_height_ratio: 0.46,
            min_font_size: 6.0,
            max_font_size: 42.0,
            padding_x: 2.0,
            padding_y: 1.0,
            interaction_padding_x: 2.0,
            interaction_padding_y: 4.0,
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
    pub mode: OcrRunMode,
    pub provider: OcrProvider,
    pub language_hint: Option<String>,
    pub auto_run_after_capture: bool,
    pub default_model_id: Option<String>,
    pub provider_profiles: Vec<OcrProviderProfile>,
    pub default_provider_profile_id: Option<String>,
}

impl Default for OcrSettings {
    fn default() -> Self {
        Self {
            mode: OcrRunMode::Standard,
            provider: OcrProvider::Local(crate::OcrLocalBackend::Mnn),
            language_hint: None,
            auto_run_after_capture: false,
            default_model_id: None,
            provider_profiles: Vec::new(),
            default_provider_profile_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslateSettings {
    pub provider: TranslateProvider,
    pub target_language: String,
    pub auto_translate_after_ocr: bool,
    pub default_model_id: Option<String>,
}

impl Default for TranslateSettings {
    fn default() -> Self {
        Self {
            provider: TranslateProvider::Local(crate::TranslateLocalBackend::CTranslate2),
            target_language: "zh-CN".to_owned(),
            auto_translate_after_ocr: false,
            default_model_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeySettings {
    pub capture: String,
    pub pin_selection: String,
    pub toggle_pins_click_through: String,
    pub show_history: String,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            capture: "Ctrl+Shift+A".to_owned(),
            pin_selection: "Ctrl+Shift+X".to_owned(),
            toggle_pins_click_through: "Ctrl+Shift+T".to_owned(),
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
