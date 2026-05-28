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
    pub(crate) show_magnifier: bool,
    pub(crate) magnifier_scale: f32,
    pub(crate) pin_hotkey: String,
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
            show_magnifier: true,
            magnifier_scale: 2.0,
            pin_hotkey: "Ctrl+Shift+X".to_owned(),
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
                "--show-magnifier" => {
                    parsed.show_magnifier = parse_next(&mut args, parsed.show_magnifier)
                }
                "--magnifier-scale" => {
                    parsed.magnifier_scale = parse_next(&mut args, parsed.magnifier_scale)
                }
                "--pin-hotkey" => parsed.pin_hotkey = parse_next(&mut args, parsed.pin_hotkey),
                _ => {}
            }
        }

        parsed.mask_opacity = parsed.mask_opacity.clamp(0.0, 0.9);
        parsed.magnifier_scale = parsed.magnifier_scale.clamp(1.0, 6.0);
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
