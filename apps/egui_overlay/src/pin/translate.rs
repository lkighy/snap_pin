use std::path::PathBuf;

use model_registry::ModelRegistry;
use perf_trace::{PerfSpan, log_elapsed};
use shared_models::{
    LanguageCode, ModelManifest, Rect, TranslateExternalProvider, TranslateLocalBackend,
    TranslateProvider, TranslationRequest,
};
use translate_engine::{RoutedTranslateEngine, TranslateEngine};

pub(crate) struct PinBlockTranslateRequest {
    pub(crate) blocks: Vec<PinTranslatableBlock>,
    pub(crate) target_language: String,
    pub(crate) provider: TranslateProvider,
    pub(crate) default_model_id: Option<String>,
    pub(crate) models_registry: Option<PathBuf>,
}

pub(crate) struct PinTranslatableBlock {
    pub(crate) index: usize,
    pub(crate) block_indices: Vec<usize>,
    pub(crate) bounds: Rect,
    pub(crate) text: String,
    pub(crate) source_language: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct PinBlockTranslation {
    pub(crate) index: usize,
    pub(crate) block_indices: Vec<usize>,
    pub(crate) source_text: String,
    pub(crate) translated_text: String,
    pub(crate) target_language: String,
}

pub(crate) fn translate_pin_blocks(
    request: PinBlockTranslateRequest,
) -> Result<Vec<PinBlockTranslation>, String> {
    let mut span = PerfSpan::new("pin_translate_blocks_total")
        .field("provider", translate_provider_label(&request.provider))
        .field("target", &request.target_language);
    let filter_start = std::time::Instant::now();
    let blocks = request
        .blocks
        .into_iter()
        .filter(|block| !block.text.trim().is_empty())
        .collect::<Vec<_>>();
    log_elapsed("pin_translate_filter_blocks", filter_start);
    span.add_field("units", blocks.len());
    if blocks.is_empty() {
        return Err("no OCR text to translate".to_owned());
    }

    let engine_start = std::time::Instant::now();
    let mut engine = RoutedTranslateEngine::default();
    log_elapsed("pin_translate_create_routed_engine", engine_start);
    if matches!(request.provider, TranslateProvider::Local(_)) {
        let registry_start = std::time::Instant::now();
        let registry = load_model_registry(request.models_registry.as_deref());
        log_elapsed("pin_translate_load_model_registry", registry_start);
        let probe = TranslationRequest {
            id: "pin-translation-model-selection".to_owned(),
            source_text: blocks
                .first()
                .map(|block| block.text.trim().to_owned())
                .unwrap_or_default(),
            source_language: blocks
                .first()
                .and_then(|block| block.source_language.clone())
                .map(LanguageCode),
            target_language: LanguageCode(request.target_language.clone()),
            provider: request.provider.clone(),
            model_id: request.default_model_id.clone(),
            context: Some("pin_window".to_owned()),
        };
        let select_start = std::time::Instant::now();
        let model = select_translation_model(&registry, &probe)
            .ok_or_else(|| "missing local translation model".to_owned())?;
        log_elapsed("pin_translate_select_model", select_start);
        let load_start = std::time::Instant::now();
        engine.load_model(model).map_err(|error| error.message)?;
        log_elapsed("pin_translate_load_model", load_start);
    }

    let translate_all_start = std::time::Instant::now();
    let translation_inputs = blocks
        .into_iter()
        .map(|block| {
            let request = TranslationRequest {
                id: format!("pin-translation-block-{}", block.index),
                source_text: block.text.trim().to_owned(),
                source_language: block.source_language.clone().map(LanguageCode),
                target_language: LanguageCode(request.target_language.clone()),
                provider: request.provider.clone(),
                model_id: request.default_model_id.clone(),
                context: Some("pin_window_block".to_owned()),
            };
            (block, request)
        })
        .collect::<Vec<_>>();
    let translation_requests = translation_inputs
        .iter()
        .map(|(_, request)| request.clone())
        .collect::<Vec<_>>();
    let translate_batch_start = std::time::Instant::now();
    let results = engine
        .translate_batch(&translation_requests)
        .map_err(|error| error.message)?;
    log_elapsed("pin_translate_batch", translate_batch_start);
    if results.len() != translation_inputs.len() {
        return Err(format!(
            "translation result count mismatch: expected {}, got {}",
            translation_inputs.len(),
            results.len()
        ));
    }

    let translations = translation_inputs
        .into_iter()
        .zip(results)
        .map(|((block, _request), result)| PinBlockTranslation {
            index: block.index,
            block_indices: block.block_indices,
            source_text: result.source_text,
            translated_text: result.translated_text,
            target_language: result.target_language.0,
        })
        .collect::<Vec<_>>();
    log_elapsed("pin_translate_all_blocks", translate_all_start);
    span.finish();

    Ok(translations)
}

pub(crate) fn parse_translate_provider(value: &str) -> TranslateProvider {
    match value {
        "disabled" => TranslateProvider::Disabled,
        "api-deepl" => TranslateProvider::ExternalApi(TranslateExternalProvider::DeepL),
        "api-google" => TranslateProvider::ExternalApi(TranslateExternalProvider::Google),
        "api-azure" => TranslateProvider::ExternalApi(TranslateExternalProvider::Azure),
        "api-openai" => TranslateProvider::ExternalApi(TranslateExternalProvider::OpenAi),
        "api-baidu" => TranslateProvider::ExternalApi(TranslateExternalProvider::Baidu),
        "api-tencent" => TranslateProvider::ExternalApi(TranslateExternalProvider::Tencent),
        "api-custom" => TranslateProvider::ExternalApi(TranslateExternalProvider::CustomHttp),
        _ => TranslateProvider::Local(TranslateLocalBackend::CTranslate2),
    }
}

fn select_translation_model<'a>(
    registry: &'a ModelRegistry,
    request: &TranslationRequest,
) -> Option<&'a ModelManifest> {
    request
        .model_id
        .as_deref()
        .and_then(|model_id| registry.find(model_id))
        .or_else(|| {
            registry.recommended_translation(
                request
                    .source_language
                    .as_ref()
                    .map(|language| language.0.as_str()),
                &request.target_language.0,
            )
        })
}

fn load_model_registry(path: Option<&std::path::Path>) -> ModelRegistry {
    let mut registry = ModelRegistry::with_builtin_defaults();
    let Some(path) = path else {
        return registry;
    };

    match std::fs::read_to_string(path) {
        Ok(contents) => match serde_json::from_str::<Vec<ModelManifest>>(&contents) {
            Ok(models) => {
                for model in models {
                    registry.register(model);
                }
            }
            Err(error) => {
                log::error!(
                    "failed to parse translation model registry {}: {error}",
                    path.display()
                );
            }
        },
        Err(error) => {
            log::warn!(
                "translation model registry not loaded from {}: {error}",
                path.display()
            );
        }
    }

    registry
}

fn translate_provider_label(provider: &TranslateProvider) -> &'static str {
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
}
