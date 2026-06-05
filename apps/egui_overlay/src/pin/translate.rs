use std::path::PathBuf;

use model_registry::ModelRegistry;
use shared_models::{
    LanguageCode, ModelManifest, TranslateExternalProvider, TranslateLocalBackend,
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
    pub(crate) text: String,
    pub(crate) source_language: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct PinBlockTranslation {
    pub(crate) index: usize,
    pub(crate) source_text: String,
    pub(crate) translated_text: String,
    pub(crate) target_language: String,
}

pub(crate) fn translate_pin_blocks(
    request: PinBlockTranslateRequest,
) -> Result<Vec<PinBlockTranslation>, String> {
    let blocks = request
        .blocks
        .into_iter()
        .filter(|block| !block.text.trim().is_empty())
        .collect::<Vec<_>>();
    if blocks.is_empty() {
        return Err("no OCR text to translate".to_owned());
    }

    let mut engine = RoutedTranslateEngine::default();
    if matches!(request.provider, TranslateProvider::Local(_)) {
        let registry = load_model_registry(request.models_registry.as_deref());
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
        let model = select_translation_model(&registry, &probe)
            .ok_or_else(|| "missing local translation model".to_owned())?;
        engine.load_model(model).map_err(|error| error.message)?;
    }

    let mut translations = Vec::with_capacity(blocks.len());
    for block in blocks {
        let translation_request = TranslationRequest {
            id: format!("pin-translation-block-{}", block.index),
            source_text: block.text.trim().to_owned(),
            source_language: block.source_language.map(LanguageCode),
            target_language: LanguageCode(request.target_language.clone()),
            provider: request.provider.clone(),
            model_id: request.default_model_id.clone(),
            context: Some("pin_window_block".to_owned()),
        };
        let result = engine
            .translate(&translation_request)
            .map_err(|error| error.message)?;
        translations.push(PinBlockTranslation {
            index: block.index,
            source_text: result.source_text,
            translated_text: result.translated_text,
            target_language: result.target_language.0,
        });
    }

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
