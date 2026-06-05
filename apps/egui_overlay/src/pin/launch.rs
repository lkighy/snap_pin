use std::path::{Path, PathBuf};
pub(crate) struct PinWindowLaunch<'a> {
    pub(crate) image_path: &'a PathBuf,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) opacity: f32,
    pub(crate) zoom_step: f32,
    pub(crate) min_width: f32,
    pub(crate) min_height: f32,
    pub(crate) always_on_top: bool,
    pub(crate) ocr_provider: &'a str,
    pub(crate) ocr_language_hint: Option<&'a str>,
    pub(crate) ocr_default_model_id: Option<&'a str>,
    pub(crate) ocr_models_registry: Option<&'a Path>,
    pub(crate) translate_provider: &'a str,
    pub(crate) translate_target_language: &'a str,
    pub(crate) translate_segmentation_mode: &'a str,
    pub(crate) translate_default_model_id: Option<&'a str>,
    pub(crate) ocr_text_font_height_ratio: f32,
    pub(crate) ocr_text_min_font_size: f32,
    pub(crate) ocr_text_max_font_size: f32,
    pub(crate) ocr_text_padding_x: f32,
    pub(crate) ocr_text_padding_y: f32,
    pub(crate) ocr_text_interaction_padding_x: f32,
    pub(crate) ocr_text_interaction_padding_y: f32,
}

pub(crate) fn spawn_pin_window(launch: &PinWindowLaunch<'_>) -> Result<(), String> {
    let current_exe = std::env::current_exe().map_err(|error| error.to_string())?;
    log::info!(
        "spawning pin window exe={} image={} x={} y={} width={} height={} opacity={} zoom_step={} always_on_top={}",
        current_exe.display(),
        launch.image_path.display(),
        launch.x,
        launch.y,
        launch.width,
        launch.height,
        launch.opacity,
        launch.zoom_step,
        launch.always_on_top
    );
    let child = std::process::Command::new(current_exe)
        .arg("--pin")
        .arg("--image")
        .arg(launch.image_path)
        .arg("--x")
        .arg(format!("{}", launch.x))
        .arg("--y")
        .arg(format!("{}", launch.y))
        .arg("--width")
        .arg(format!("{}", launch.width))
        .arg("--height")
        .arg(format!("{}", launch.height))
        .arg("--pin-opacity")
        .arg(format!("{}", launch.opacity))
        .arg("--pin-zoom-step")
        .arg(format!("{}", launch.zoom_step))
        .arg("--pin-min-width")
        .arg(format!("{}", launch.min_width))
        .arg("--pin-min-height")
        .arg(format!("{}", launch.min_height))
        .arg("--pin-always-on-top")
        .arg(format!("{}", launch.always_on_top))
        .arg("--ocr-provider")
        .arg(launch.ocr_provider)
        .arg("--ocr-language-hint")
        .arg(launch.ocr_language_hint.unwrap_or(""))
        .arg("--ocr-default-model-id")
        .arg(launch.ocr_default_model_id.unwrap_or(""))
        .arg("--ocr-models-registry")
        .arg(launch.ocr_models_registry.unwrap_or_else(|| Path::new("")))
        .arg("--translate-provider")
        .arg(launch.translate_provider)
        .arg("--translate-target-language")
        .arg(launch.translate_target_language)
        .arg("--translate-segmentation-mode")
        .arg(launch.translate_segmentation_mode)
        .arg("--translate-default-model-id")
        .arg(launch.translate_default_model_id.unwrap_or(""))
        .arg("--ocr-text-font-height-ratio")
        .arg(format!("{}", launch.ocr_text_font_height_ratio))
        .arg("--ocr-text-min-font-size")
        .arg(format!("{}", launch.ocr_text_min_font_size))
        .arg("--ocr-text-max-font-size")
        .arg(format!("{}", launch.ocr_text_max_font_size))
        .arg("--ocr-text-padding-x")
        .arg(format!("{}", launch.ocr_text_padding_x))
        .arg("--ocr-text-padding-y")
        .arg(format!("{}", launch.ocr_text_padding_y))
        .arg("--ocr-text-interaction-padding-x")
        .arg(format!("{}", launch.ocr_text_interaction_padding_x))
        .arg("--ocr-text-interaction-padding-y")
        .arg(format!("{}", launch.ocr_text_interaction_padding_y))
        .spawn()
        .map_err(|error| error.to_string())?;
    log::info!("pin window spawned pid={}", child.id());
    Ok(())
}
