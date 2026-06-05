use serde::{Deserialize, Serialize};
use shared_models::{
    CaptureCompletionAction, OcrExternalProvider, OcrLocalBackend, OcrProvider, OcrProviderProfile,
    OcrRunMode, Settings, TranslateExternalProvider, TranslateLocalBackend, TranslateProvider,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsDto {
    #[serde(default = "default_interface_settings")]
    pub interface: InterfaceSettingsDto,
    pub capture: CaptureSettingsDto,
    pub overlay: OverlaySettingsDto,
    pub pin: PinSettingsDto,
    pub ocr: OcrSettingsDto,
    pub translation: TranslationSettingsDto,
    pub hotkeys: HotkeySettingsDto,
    pub history: HistorySettingsDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceSettingsDto {
    pub language: String,
}

fn default_interface_settings() -> InterfaceSettingsDto {
    InterfaceSettingsDto {
        language: "zh-CN".to_owned(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSettingsDto {
    pub include_cursor: bool,
    pub auto_copy_to_clipboard: bool,
    pub freeze_screen_on_capture: bool,
    pub show_size_label: bool,
    pub show_toolbar: bool,
    pub capture_delay_ms: u64,
    pub mask_opacity: f32,
    pub border_color: String,
    pub completion_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlaySettingsDto {
    pub show_magnifier: bool,
    pub magnifier_scale: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinSettingsDto {
    pub default_opacity: f32,
    pub click_through: bool,
    pub always_on_top: bool,
    pub remember_position: bool,
    pub zoom_step: f32,
    #[serde(default = "default_pin_min_width")]
    pub min_width: f32,
    #[serde(default = "default_pin_min_height")]
    pub min_height: f32,
    pub show_ocr_text: bool,
    pub show_translation_text: bool,
    #[serde(default = "default_ocr_text_overlay_settings")]
    pub ocr_text: OcrTextOverlaySettingsDto,
}

fn default_pin_min_width() -> f32 {
    96.0
}

fn default_pin_min_height() -> f32 {
    72.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrTextOverlaySettingsDto {
    #[serde(default = "default_ocr_text_font_height_ratio")]
    pub font_height_ratio: f32,
    #[serde(default = "default_ocr_text_min_font_size")]
    pub min_font_size: f32,
    #[serde(default = "default_ocr_text_max_font_size")]
    pub max_font_size: f32,
    #[serde(default = "default_ocr_text_padding_x")]
    pub padding_x: f32,
    #[serde(default = "default_ocr_text_padding_y")]
    pub padding_y: f32,
    #[serde(default = "default_ocr_text_interaction_padding_x")]
    pub interaction_padding_x: f32,
    #[serde(default = "default_ocr_text_interaction_padding_y")]
    pub interaction_padding_y: f32,
}

fn default_ocr_text_overlay_settings() -> OcrTextOverlaySettingsDto {
    OcrTextOverlaySettingsDto {
        font_height_ratio: default_ocr_text_font_height_ratio(),
        min_font_size: default_ocr_text_min_font_size(),
        max_font_size: default_ocr_text_max_font_size(),
        padding_x: default_ocr_text_padding_x(),
        padding_y: default_ocr_text_padding_y(),
        interaction_padding_x: default_ocr_text_interaction_padding_x(),
        interaction_padding_y: default_ocr_text_interaction_padding_y(),
    }
}

fn default_ocr_text_font_height_ratio() -> f32 {
    0.46
}

fn default_ocr_text_min_font_size() -> f32 {
    6.0
}

fn default_ocr_text_max_font_size() -> f32 {
    42.0
}

fn default_ocr_text_padding_x() -> f32 {
    2.0
}

fn default_ocr_text_padding_y() -> f32 {
    1.0
}

fn default_ocr_text_interaction_padding_x() -> f32 {
    2.0
}

fn default_ocr_text_interaction_padding_y() -> f32 {
    4.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrSettingsDto {
    #[serde(default = "default_ocr_mode")]
    pub mode: String,
    pub provider: String,
    pub language_hint: String,
    pub auto_run_after_capture: bool,
    #[serde(default)]
    pub default_model_id: String,
    #[serde(default)]
    pub provider_profiles: Vec<OcrProviderProfileDto>,
    #[serde(default)]
    pub default_provider_profile_id: String,
}

fn default_ocr_mode() -> String {
    "standard".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrProviderProfileDto {
    pub id: String,
    pub provider: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub language_hint: String,
    #[serde(default = "default_ocr_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub retry_limit: u8,
    #[serde(default)]
    pub privacy_notice_acknowledged: bool,
}

fn default_ocr_timeout_ms() -> u64 {
    15_000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationSettingsDto {
    pub provider: String,
    pub target_language: String,
    pub auto_translate_after_ocr: bool,
    #[serde(default)]
    pub default_model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeySettingsDto {
    pub capture: String,
    #[serde(default = "default_pin_selection_hotkey")]
    pub pin_selection: String,
    pub toggle_pins_click_through: String,
    pub show_history: String,
}

fn default_pin_selection_hotkey() -> String {
    "Ctrl+Shift+X".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySettingsDto {
    pub enabled: bool,
    pub max_entries: usize,
}

impl From<&Settings> for AppSettingsDto {
    fn from(settings: &Settings) -> Self {
        Self {
            interface: InterfaceSettingsDto {
                language: normalize_language(&settings.interface.language),
            },
            capture: CaptureSettingsDto {
                include_cursor: settings.capture.include_cursor,
                auto_copy_to_clipboard: settings.capture.auto_copy_to_clipboard,
                freeze_screen_on_capture: settings.capture.freeze_screen_on_capture,
                show_size_label: settings.capture.show_size_label,
                show_toolbar: settings.capture.show_toolbar,
                capture_delay_ms: settings.capture.capture_delay_ms,
                mask_opacity: settings.capture.mask_opacity,
                border_color: settings.capture.border_color.clone(),
                completion_action: completion_action_name(&settings.capture.completion_action),
            },
            overlay: OverlaySettingsDto {
                show_magnifier: settings.overlay.show_magnifier,
                magnifier_scale: settings.overlay.magnifier_scale,
            },
            pin: PinSettingsDto {
                default_opacity: settings.pin.default_opacity,
                click_through: settings.pin.click_through,
                always_on_top: settings.pin.always_on_top,
                remember_position: settings.pin.remember_position,
                zoom_step: settings.pin.zoom_step,
                min_width: settings.pin.min_width,
                min_height: settings.pin.min_height,
                show_ocr_text: settings.pin.show_ocr_text,
                show_translation_text: settings.pin.show_translation_text,
                ocr_text: OcrTextOverlaySettingsDto::from(&settings.pin.ocr_text),
            },
            ocr: OcrSettingsDto {
                mode: ocr_mode_name(&settings.ocr.mode),
                provider: ocr_provider_name(&settings.ocr.provider),
                language_hint: settings.ocr.language_hint.clone().unwrap_or_default(),
                auto_run_after_capture: settings.ocr.auto_run_after_capture,
                default_model_id: settings.ocr.default_model_id.clone().unwrap_or_default(),
                provider_profiles: settings
                    .ocr
                    .provider_profiles
                    .iter()
                    .map(OcrProviderProfileDto::from)
                    .collect(),
                default_provider_profile_id: settings
                    .ocr
                    .default_provider_profile_id
                    .clone()
                    .unwrap_or_default(),
            },
            translation: TranslationSettingsDto {
                provider: translate_provider_name(&settings.translate.provider),
                target_language: settings.translate.target_language.clone(),
                auto_translate_after_ocr: settings.translate.auto_translate_after_ocr,
                default_model_id: settings
                    .translate
                    .default_model_id
                    .clone()
                    .unwrap_or_default(),
            },
            hotkeys: HotkeySettingsDto {
                capture: settings.hotkeys.capture.clone(),
                pin_selection: settings.hotkeys.pin_selection.clone(),
                toggle_pins_click_through: settings.hotkeys.toggle_pins_click_through.clone(),
                show_history: settings.hotkeys.show_history.clone(),
            },
            history: HistorySettingsDto {
                enabled: settings.history.enabled,
                max_entries: settings.history.max_entries,
            },
        }
    }
}

impl From<AppSettingsDto> for Settings {
    fn from(dto: AppSettingsDto) -> Self {
        let mut settings = Settings::default();

        settings.interface.language = normalize_language(&dto.interface.language);

        settings.capture.include_cursor = dto.capture.include_cursor;
        settings.capture.auto_copy_to_clipboard = dto.capture.auto_copy_to_clipboard;
        settings.capture.freeze_screen_on_capture = dto.capture.freeze_screen_on_capture;
        settings.capture.show_size_label = dto.capture.show_size_label;
        settings.capture.show_toolbar = dto.capture.show_toolbar;
        settings.capture.capture_delay_ms = dto.capture.capture_delay_ms.min(30_000);
        settings.capture.mask_opacity = dto.capture.mask_opacity.clamp(0.0, 0.9);
        settings.capture.border_color = dto.capture.border_color;
        settings.capture.completion_action =
            parse_completion_action(&dto.capture.completion_action);

        settings.overlay.show_magnifier = dto.overlay.show_magnifier;
        settings.overlay.magnifier_scale = dto.overlay.magnifier_scale.clamp(1.0, 6.0);
        settings.overlay.default_pin_opacity = dto.pin.default_opacity.clamp(0.2, 1.0);
        settings.overlay.click_through_pins = dto.pin.click_through;

        settings.pin.default_opacity = dto.pin.default_opacity.clamp(0.2, 1.0);
        settings.pin.click_through = dto.pin.click_through;
        settings.pin.always_on_top = dto.pin.always_on_top;
        settings.pin.remember_position = dto.pin.remember_position;
        settings.pin.zoom_step = dto.pin.zoom_step.clamp(0.05, 0.5);
        settings.pin.min_width = dto.pin.min_width.clamp(16.0, 2048.0);
        settings.pin.min_height = dto.pin.min_height.clamp(16.0, 2048.0);
        settings.pin.show_ocr_text = dto.pin.show_ocr_text;
        settings.pin.show_translation_text = dto.pin.show_translation_text;
        settings.pin.ocr_text = dto.pin.ocr_text.into_settings();

        settings.ocr.mode = parse_ocr_mode(&dto.ocr.mode);
        settings.ocr.provider = parse_ocr_provider(&dto.ocr.provider);
        settings.ocr.language_hint = empty_to_none(dto.ocr.language_hint);
        settings.ocr.auto_run_after_capture = dto.ocr.auto_run_after_capture;
        settings.ocr.default_model_id = empty_to_none(dto.ocr.default_model_id);
        settings.ocr.provider_profiles = dto
            .ocr
            .provider_profiles
            .into_iter()
            .filter_map(OcrProviderProfile::try_from_dto)
            .collect();
        settings.ocr.default_provider_profile_id =
            empty_to_none(dto.ocr.default_provider_profile_id);

        settings.translate.provider = parse_translate_provider(&dto.translation.provider);
        settings.translate.target_language = dto.translation.target_language;
        settings.translate.auto_translate_after_ocr = dto.translation.auto_translate_after_ocr;
        settings.translate.default_model_id = empty_to_none(dto.translation.default_model_id);

        settings.hotkeys.capture = dto.hotkeys.capture;
        settings.hotkeys.pin_selection = dto.hotkeys.pin_selection;
        settings.hotkeys.toggle_pins_click_through = dto.hotkeys.toggle_pins_click_through;
        settings.hotkeys.show_history = dto.hotkeys.show_history;

        settings.history.enabled = dto.history.enabled;
        settings.history.max_entries = dto.history.max_entries.clamp(10, 10_000);

        settings
    }
}

impl From<&shared_models::OcrTextOverlaySettings> for OcrTextOverlaySettingsDto {
    fn from(settings: &shared_models::OcrTextOverlaySettings) -> Self {
        Self {
            font_height_ratio: settings.font_height_ratio,
            min_font_size: settings.min_font_size,
            max_font_size: settings.max_font_size,
            padding_x: settings.padding_x,
            padding_y: settings.padding_y,
            interaction_padding_x: settings.interaction_padding_x,
            interaction_padding_y: settings.interaction_padding_y,
        }
    }
}

impl OcrTextOverlaySettingsDto {
    fn into_settings(self) -> shared_models::OcrTextOverlaySettings {
        let min_font_size = self.min_font_size.clamp(4.0, 96.0);
        let max_font_size = self.max_font_size.clamp(min_font_size, 128.0);
        shared_models::OcrTextOverlaySettings {
            font_height_ratio: self.font_height_ratio.clamp(0.1, 2.0),
            min_font_size,
            max_font_size,
            padding_x: self.padding_x.clamp(0.0, 32.0),
            padding_y: self.padding_y.clamp(0.0, 32.0),
            interaction_padding_x: self.interaction_padding_x.clamp(0.0, 48.0),
            interaction_padding_y: self.interaction_padding_y.clamp(0.0, 48.0),
        }
    }
}

impl From<&OcrProviderProfile> for OcrProviderProfileDto {
    fn from(profile: &OcrProviderProfile) -> Self {
        Self {
            id: profile.id.clone(),
            provider: ocr_external_provider_name(&profile.provider),
            endpoint: profile.endpoint.clone().unwrap_or_default(),
            model: profile.model.clone().unwrap_or_default(),
            language_hint: profile.language_hint.clone().unwrap_or_default(),
            timeout_ms: profile.timeout_ms,
            retry_limit: profile.retry_limit,
            privacy_notice_acknowledged: profile.privacy_notice_acknowledged,
        }
    }
}

trait TryFromOcrProviderProfileDto {
    fn try_from_dto(dto: OcrProviderProfileDto) -> Option<OcrProviderProfile>;
}

impl TryFromOcrProviderProfileDto for OcrProviderProfile {
    fn try_from_dto(dto: OcrProviderProfileDto) -> Option<OcrProviderProfile> {
        let id = dto.id.trim().to_owned();
        if id.is_empty() {
            return None;
        }

        Some(OcrProviderProfile {
            id,
            provider: parse_ocr_external_provider(&dto.provider),
            endpoint: empty_to_none(dto.endpoint),
            model: empty_to_none(dto.model),
            language_hint: empty_to_none(dto.language_hint),
            timeout_ms: dto.timeout_ms.clamp(1_000, 120_000),
            retry_limit: dto.retry_limit.min(5),
            privacy_notice_acknowledged: dto.privacy_notice_acknowledged,
        })
    }
}

fn normalize_language(value: &str) -> String {
    match value {
        "en" | "ja" | "ko" | "fr" | "de" => value.to_owned(),
        "zh-CN" | "zh" | "zh-cn" | "zh-Hans" | "zh-hans" => "zh-CN".to_owned(),
        _ => "zh-CN".to_owned(),
    }
}

fn empty_to_none(value: String) -> Option<String> {
    let value = value.trim().to_owned();
    if value.is_empty() { None } else { Some(value) }
}

fn completion_action_name(action: &CaptureCompletionAction) -> String {
    match action {
        CaptureCompletionAction::Pin => "pin",
        CaptureCompletionAction::CopyToClipboard => "copy",
        CaptureCompletionAction::SaveToFile => "save",
        CaptureCompletionAction::OpenEditor => "editor",
    }
    .to_owned()
}

fn parse_completion_action(value: &str) -> CaptureCompletionAction {
    match value {
        "copy" => CaptureCompletionAction::CopyToClipboard,
        "save" => CaptureCompletionAction::SaveToFile,
        "editor" => CaptureCompletionAction::OpenEditor,
        _ => CaptureCompletionAction::Pin,
    }
}

fn ocr_provider_name(provider: &OcrProvider) -> String {
    match provider {
        OcrProvider::Disabled => "disabled",
        OcrProvider::System => "system",
        OcrProvider::Local(OcrLocalBackend::Mnn) => "local-mnn",
        OcrProvider::Local(OcrLocalBackend::OnnxRuntime) => "local-onnx",
        OcrProvider::Local(OcrLocalBackend::PaddleRuntime) => "local-paddle",
        OcrProvider::Local(OcrLocalBackend::Custom(_)) => "local-custom",
        OcrProvider::ExternalApi(OcrExternalProvider::OpenAi) => "api-openai",
        OcrProvider::ExternalApi(OcrExternalProvider::AzureVision) => "api-azure",
        OcrProvider::ExternalApi(OcrExternalProvider::GoogleVision) => "api-google",
        OcrProvider::ExternalApi(OcrExternalProvider::BaiduOcr) => "api-baidu",
        OcrProvider::ExternalApi(OcrExternalProvider::TencentOcr) => "api-tencent",
        OcrProvider::ExternalApi(OcrExternalProvider::Custom(_)) => "api-custom",
    }
    .to_owned()
}

fn ocr_mode_name(mode: &OcrRunMode) -> String {
    match mode {
        OcrRunMode::Lightweight => "lightweight",
        OcrRunMode::Standard => "standard",
        OcrRunMode::Compatible => "compatible",
        OcrRunMode::Advanced => "advanced",
        OcrRunMode::Cloud => "cloud",
    }
    .to_owned()
}

fn parse_ocr_mode(value: &str) -> OcrRunMode {
    match value {
        "lightweight" => OcrRunMode::Lightweight,
        "compatible" => OcrRunMode::Compatible,
        "advanced" => OcrRunMode::Advanced,
        "cloud" => OcrRunMode::Cloud,
        _ => OcrRunMode::Standard,
    }
}

fn ocr_external_provider_name(provider: &OcrExternalProvider) -> String {
    match provider {
        OcrExternalProvider::OpenAi => "api-openai",
        OcrExternalProvider::AzureVision => "api-azure",
        OcrExternalProvider::GoogleVision => "api-google",
        OcrExternalProvider::BaiduOcr => "api-baidu",
        OcrExternalProvider::TencentOcr => "api-tencent",
        OcrExternalProvider::Custom(_) => "api-custom",
    }
    .to_owned()
}

fn parse_ocr_external_provider(value: &str) -> OcrExternalProvider {
    match value {
        "api-openai" => OcrExternalProvider::OpenAi,
        "api-azure" => OcrExternalProvider::AzureVision,
        "api-google" => OcrExternalProvider::GoogleVision,
        "api-baidu" => OcrExternalProvider::BaiduOcr,
        "api-tencent" => OcrExternalProvider::TencentOcr,
        "api-custom" => OcrExternalProvider::Custom("custom".to_owned()),
        _ => OcrExternalProvider::Custom("custom".to_owned()),
    }
}

fn parse_ocr_provider(value: &str) -> OcrProvider {
    match value {
        "disabled" => OcrProvider::Disabled,
        "system" => OcrProvider::System,
        "local-onnx" => OcrProvider::Local(OcrLocalBackend::OnnxRuntime),
        "local-paddle" => OcrProvider::Local(OcrLocalBackend::PaddleRuntime),
        "api-openai" => OcrProvider::ExternalApi(OcrExternalProvider::OpenAi),
        "api-azure" => OcrProvider::ExternalApi(OcrExternalProvider::AzureVision),
        "api-google" => OcrProvider::ExternalApi(OcrExternalProvider::GoogleVision),
        "api-baidu" => OcrProvider::ExternalApi(OcrExternalProvider::BaiduOcr),
        "api-tencent" => OcrProvider::ExternalApi(OcrExternalProvider::TencentOcr),
        "api-custom" => OcrProvider::ExternalApi(OcrExternalProvider::Custom("custom".to_owned())),
        _ => OcrProvider::Local(OcrLocalBackend::Mnn),
    }
}

fn translate_provider_name(provider: &TranslateProvider) -> String {
    match provider {
        TranslateProvider::Disabled => "disabled",
        TranslateProvider::Local(TranslateLocalBackend::CTranslate2) => "local-ct2",
        TranslateProvider::Local(TranslateLocalBackend::Custom(_)) => "local-custom",
        TranslateProvider::ExternalApi(TranslateExternalProvider::DeepL) => "api-deepl",
        TranslateProvider::ExternalApi(TranslateExternalProvider::Google) => "api-google",
        TranslateProvider::ExternalApi(TranslateExternalProvider::Azure) => "api-azure",
        TranslateProvider::ExternalApi(TranslateExternalProvider::OpenAi) => "api-openai",
        TranslateProvider::ExternalApi(TranslateExternalProvider::Baidu) => "api-baidu",
        TranslateProvider::ExternalApi(TranslateExternalProvider::Tencent) => "api-tencent",
        TranslateProvider::ExternalApi(TranslateExternalProvider::CustomHttp) => "api-custom",
        TranslateProvider::ExternalApi(TranslateExternalProvider::Custom(_)) => "api-custom",
        TranslateProvider::Experimental(_) => "experimental",
        TranslateProvider::Custom(_) => "custom",
    }
    .to_owned()
}

fn parse_translate_provider(value: &str) -> TranslateProvider {
    match value {
        "disabled" => TranslateProvider::Disabled,
        "api-deepl" => TranslateProvider::ExternalApi(TranslateExternalProvider::DeepL),
        "api-google" => TranslateProvider::ExternalApi(TranslateExternalProvider::Google),
        "api-azure" => TranslateProvider::ExternalApi(TranslateExternalProvider::Azure),
        "api-openai" => TranslateProvider::ExternalApi(TranslateExternalProvider::OpenAi),
        "api-baidu" => TranslateProvider::ExternalApi(TranslateExternalProvider::Baidu),
        "api-tencent" => TranslateProvider::ExternalApi(TranslateExternalProvider::Tencent),
        "api-custom" => TranslateProvider::ExternalApi(TranslateExternalProvider::CustomHttp),
        "experimental-rust-bert" => {
            TranslateProvider::Experimental(shared_models::TranslateExperimentalBackend::RustBert)
        }
        "experimental-candle" => {
            TranslateProvider::Experimental(shared_models::TranslateExperimentalBackend::Candle)
        }
        _ => TranslateProvider::Local(TranslateLocalBackend::CTranslate2),
    }
}
