use std::path::PathBuf;

use eframe::egui::Color32;

#[derive(Debug, Clone)]
pub(crate) struct CliArgs {
    pub(crate) mode: OverlayRunMode,
    pub(crate) image: Option<PathBuf>,
    pub(crate) snapshot: Option<PathBuf>,
    pub(crate) language: OverlayLanguage,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) include_cursor: bool,
    pub(crate) mask_opacity: f32,
    pub(crate) border_color: Color32,
    pub(crate) show_size_label: bool,
    pub(crate) show_toolbar: bool,
    pub(crate) show_magnifier: bool,
    pub(crate) magnifier_scale: f32,
    pub(crate) pin_hotkey: String,
    pub(crate) completion_action: String,
    pub(crate) pin_opacity: f32,
    pub(crate) pin_zoom_step: f32,
    pub(crate) pin_min_width: f32,
    pub(crate) pin_min_height: f32,
    pub(crate) pin_always_on_top: bool,
    pub(crate) ocr_provider: String,
    pub(crate) ocr_language_hint: Option<String>,
    pub(crate) ocr_default_model_id: Option<String>,
    pub(crate) ocr_models_registry: Option<PathBuf>,
    pub(crate) ocr_text_font_height_ratio: f32,
    pub(crate) ocr_text_min_font_size: f32,
    pub(crate) ocr_text_max_font_size: f32,
    pub(crate) ocr_text_padding_x: f32,
    pub(crate) ocr_text_padding_y: f32,
    pub(crate) ocr_text_interaction_padding_x: f32,
    pub(crate) ocr_text_interaction_padding_y: f32,
    pub(crate) resident: bool,
    pub(crate) control_port: u16,
}

impl CliArgs {
    pub(crate) fn parse() -> Self {
        let mut args = std::env::args().skip(1);
        let mut parsed = Self {
            mode: OverlayRunMode::Capture,
            image: None,
            snapshot: None,
            language: OverlayLanguage::ZhCn,
            x: 120.0,
            y: 120.0,
            width: 480.0,
            height: 320.0,
            include_cursor: false,
            mask_opacity: 0.46,
            border_color: Color32::from_rgb(47, 138, 163),
            show_size_label: true,
            show_toolbar: true,
            show_magnifier: true,
            magnifier_scale: 2.0,
            pin_hotkey: "Ctrl+Shift+X".to_owned(),
            completion_action: "pin".to_owned(),
            pin_opacity: 1.0,
            pin_zoom_step: 0.1,
            pin_min_width: 96.0,
            pin_min_height: 72.0,
            pin_always_on_top: true,
            ocr_provider: "local-mnn".to_owned(),
            ocr_language_hint: None,
            ocr_default_model_id: None,
            ocr_models_registry: None,
            ocr_text_font_height_ratio: 0.46,
            ocr_text_min_font_size: 6.0,
            ocr_text_max_font_size: 42.0,
            ocr_text_padding_x: 2.0,
            ocr_text_padding_y: 1.0,
            ocr_text_interaction_padding_x: 2.0,
            ocr_text_interaction_padding_y: 4.0,
            resident: false,
            control_port: 47232,
        };

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--capture" => parsed.mode = OverlayRunMode::Capture,
                "--pin" => parsed.mode = OverlayRunMode::Pin,
                "--image" => parsed.image = args.next().map(PathBuf::from),
                "--snapshot" => parsed.snapshot = args.next().map(PathBuf::from),
                "--language" => {
                    if let Some(language) = args.next() {
                        parsed.language = OverlayLanguage::from_code(&language);
                    }
                }
                "--x" => parsed.x = parse_next(&mut args, parsed.x),
                "--y" => parsed.y = parse_next(&mut args, parsed.y),
                "--width" => parsed.width = parse_next(&mut args, parsed.width),
                "--height" => parsed.height = parse_next(&mut args, parsed.height),
                "--include-cursor" => parsed.include_cursor = true,
                "--resident" => parsed.resident = true,
                "--control-port" => {
                    parsed.control_port = parse_next(&mut args, parsed.control_port)
                }
                "--mask-opacity" => {
                    parsed.mask_opacity = parse_next(&mut args, parsed.mask_opacity)
                }
                "--border-color" => {
                    if let Some(color) = args.next().and_then(|value| parse_color(&value)) {
                        parsed.border_color = color;
                    }
                }
                "--show-size-label" => {
                    parsed.show_size_label = parse_next(&mut args, parsed.show_size_label)
                }
                "--show-toolbar" => {
                    parsed.show_toolbar = parse_next(&mut args, parsed.show_toolbar)
                }
                "--show-magnifier" => {
                    parsed.show_magnifier = parse_next(&mut args, parsed.show_magnifier)
                }
                "--magnifier-scale" => {
                    parsed.magnifier_scale = parse_next(&mut args, parsed.magnifier_scale)
                }
                "--pin-hotkey" => parsed.pin_hotkey = parse_next(&mut args, parsed.pin_hotkey),
                "--completion-action" => {
                    parsed.completion_action = parse_next(&mut args, parsed.completion_action)
                }
                "--pin-opacity" => parsed.pin_opacity = parse_next(&mut args, parsed.pin_opacity),
                "--pin-zoom-step" => {
                    parsed.pin_zoom_step = parse_next(&mut args, parsed.pin_zoom_step)
                }
                "--pin-min-width" => {
                    parsed.pin_min_width = parse_next(&mut args, parsed.pin_min_width)
                }
                "--pin-min-height" => {
                    parsed.pin_min_height = parse_next(&mut args, parsed.pin_min_height)
                }
                "--pin-always-on-top" => {
                    parsed.pin_always_on_top = parse_next(&mut args, parsed.pin_always_on_top)
                }
                "--ocr-provider" => {
                    parsed.ocr_provider = parse_next(&mut args, parsed.ocr_provider)
                }
                "--ocr-language-hint" => {
                    parsed.ocr_language_hint = args.next().filter(|value| !value.is_empty())
                }
                "--ocr-default-model-id" => {
                    parsed.ocr_default_model_id = args.next().filter(|value| !value.is_empty())
                }
                "--ocr-models-registry" => {
                    parsed.ocr_models_registry = args
                        .next()
                        .filter(|value| !value.is_empty())
                        .map(PathBuf::from)
                }
                "--ocr-text-font-height-ratio" => {
                    parsed.ocr_text_font_height_ratio =
                        parse_next(&mut args, parsed.ocr_text_font_height_ratio)
                }
                "--ocr-text-min-font-size" => {
                    parsed.ocr_text_min_font_size =
                        parse_next(&mut args, parsed.ocr_text_min_font_size)
                }
                "--ocr-text-max-font-size" => {
                    parsed.ocr_text_max_font_size =
                        parse_next(&mut args, parsed.ocr_text_max_font_size)
                }
                "--ocr-text-padding-x" => {
                    parsed.ocr_text_padding_x = parse_next(&mut args, parsed.ocr_text_padding_x)
                }
                "--ocr-text-padding-y" => {
                    parsed.ocr_text_padding_y = parse_next(&mut args, parsed.ocr_text_padding_y)
                }
                "--ocr-text-interaction-padding-x" => {
                    parsed.ocr_text_interaction_padding_x =
                        parse_next(&mut args, parsed.ocr_text_interaction_padding_x)
                }
                "--ocr-text-interaction-padding-y" => {
                    parsed.ocr_text_interaction_padding_y =
                        parse_next(&mut args, parsed.ocr_text_interaction_padding_y)
                }
                _ => {}
            }
        }

        parsed.mask_opacity = parsed.mask_opacity.clamp(0.0, 0.9);
        parsed.magnifier_scale = parsed.magnifier_scale.clamp(1.0, 6.0);
        parsed.pin_opacity = parsed.pin_opacity.clamp(0.2, 1.0);
        parsed.pin_zoom_step = parsed.pin_zoom_step.clamp(0.05, 0.5);
        parsed.pin_min_width = parsed.pin_min_width.clamp(16.0, 2048.0);
        parsed.pin_min_height = parsed.pin_min_height.clamp(16.0, 2048.0);
        parsed.ocr_text_font_height_ratio = parsed.ocr_text_font_height_ratio.clamp(0.1, 2.0);
        parsed.ocr_text_min_font_size = parsed.ocr_text_min_font_size.clamp(4.0, 96.0);
        parsed.ocr_text_max_font_size = parsed
            .ocr_text_max_font_size
            .clamp(parsed.ocr_text_min_font_size, 128.0);
        parsed.ocr_text_padding_x = parsed.ocr_text_padding_x.clamp(0.0, 32.0);
        parsed.ocr_text_padding_y = parsed.ocr_text_padding_y.clamp(0.0, 32.0);
        parsed.ocr_text_interaction_padding_x =
            parsed.ocr_text_interaction_padding_x.clamp(0.0, 48.0);
        parsed.ocr_text_interaction_padding_y =
            parsed.ocr_text_interaction_padding_y.clamp(0.0, 48.0);
        parsed.width = parsed.width.max(64.0);
        parsed.height = parsed.height.max(64.0);
        parsed
    }
}

fn parse_next<T>(args: &mut impl Iterator<Item = String>, fallback: T) -> T
where
    T: std::str::FromStr,
{
    args.next()
        .and_then(|value| value.parse::<T>().ok())
        .unwrap_or(fallback)
}

pub(crate) fn parse_color(value: &str) -> Option<Color32> {
    let value = value.trim().trim_start_matches('#');
    if value.len() != 6 {
        return None;
    }

    let rgb = u32::from_str_radix(value, 16).ok()?;
    Some(Color32::from_rgb(
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverlayRunMode {
    Capture,
    Pin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverlayLanguage {
    ZhCn,
    En,
    Ja,
    Ko,
    Fr,
    De,
}

impl OverlayLanguage {
    pub(crate) fn from_code(value: &str) -> Self {
        match value {
            "en" => Self::En,
            "ja" => Self::Ja,
            "ko" => Self::Ko,
            "fr" => Self::Fr,
            "de" => Self::De,
            "zh-CN" | "zh" | "zh-cn" | "zh-Hans" | "zh-hans" => Self::ZhCn,
            _ => Self::ZhCn,
        }
    }
}
