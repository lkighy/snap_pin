use std::path::PathBuf;

use model_registry::ModelRegistry;
use ocr_engine::{OcrEngine, RoutedOcrEngine};
use shared_models::{
    ImageData, ImageFormat, ImageId, ImageMetadata, OcrExternalProvider, OcrJob, OcrLocalBackend,
    OcrProvider, OcrResult, Point, Rect, Size,
};

pub(crate) struct PinOcrRequest {
    pub(crate) path: PathBuf,
    pub(crate) provider: OcrProvider,
    pub(crate) language_hint: Option<String>,
    pub(crate) default_model_id: Option<String>,
    pub(crate) models_registry: Option<PathBuf>,
    pub(crate) load_error_prefix: String,
}

pub(crate) fn recognize_pin_image(request: PinOcrRequest) -> Result<OcrResult, String> {
    let image = image::open(&request.path)
        .map_err(|error| format!("{}: {error}", request.load_error_prefix))?
        .to_rgba8();
    let width = image.width();
    let height = image.height();
    let image_id = ImageId::new(format!("pin-{}", pin_image_id_from_path(&request.path)));
    let image_data = ImageData {
        id: image_id.clone(),
        metadata: ImageMetadata {
            id: image_id.clone(),
            pixel_size: Size::new(width as f32, height as f32),
            format: ImageFormat::Rgba8,
            monitor_name: None,
        },
        bytes: image.into_raw(),
    };
    let mut job = OcrJob {
        id: format!("pin-ocr-{}", pin_image_id_from_path(&request.path)),
        image_id,
        source_rect: Some(Rect::new(
            Point::ZERO,
            Size::new(width as f32, height as f32),
        )),
        language_hint: request.language_hint,
        provider: request.provider,
        provider_profile_id: None,
        model_id: request.default_model_id,
    };

    if job.provider == OcrProvider::System {
        let platform = platform_runtime::create_platform();
        return platform
            .system_ocr()
            .recognize(&job, &image_data)
            .map_err(|error| error.message);
    }

    let mut engine = RoutedOcrEngine::default();
    if matches!(job.provider, OcrProvider::Local(_)) {
        let registry = load_model_registry(request.models_registry.as_deref());
        let model = job
            .model_id
            .as_deref()
            .and_then(|model_id| registry.find(model_id))
            .or_else(|| registry.recommended_ocr());

        if let Some(model) = model {
            if job.model_id.is_none() {
                job.model_id = Some(model.id.clone());
            }
            engine.load_model(model).map_err(|error| error.message)?;
        }
    }

    engine
        .recognize(&job, &image_data)
        .map_err(|error| error.message)
}

pub(crate) fn parse_ocr_provider(value: &str) -> OcrProvider {
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

fn load_model_registry(path: Option<&std::path::Path>) -> ModelRegistry {
    let mut registry = ModelRegistry::with_builtin_defaults();
    let Some(path) = path else {
        return registry;
    };

    match std::fs::read_to_string(path) {
        Ok(contents) => {
            match serde_json::from_str::<Vec<shared_models::ModelManifest>>(&contents) {
                Ok(models) => {
                    for model in models {
                        registry.register(model);
                    }
                }
                Err(error) => {
                    log::error!(
                        "failed to parse OCR model registry {}: {error}",
                        path.display()
                    );
                }
            }
        }
        Err(error) => {
            log::warn!(
                "OCR model registry not loaded from {}: {error}",
                path.display()
            );
        }
    }

    registry
}

fn pin_image_id_from_path(path: &std::path::Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("image")
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() || value == '-' || value == '_' {
                value
            } else {
                '_'
            }
        })
        .collect()
}
