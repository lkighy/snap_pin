use shared_models::{LanguageCode, TranslationRequest, TranslationResult};

use perf_trace::{PerfSpan, log_elapsed};

use crate::{TranslateEngineError, TranslationModelBundle};

pub fn translate(
    bundle: &TranslationModelBundle,
    request: &TranslationRequest,
) -> Result<TranslationResult, TranslateEngineError> {
    let span = PerfSpan::new("ct2_backend_translate_total")
        .field("model_id", &bundle.manifest.id)
        .field("target", &request.target_language.0)
        .field("source_chars", request.source_text.chars().count());
    let defaults_start = std::time::Instant::now();
    let request = request_with_model_defaults(bundle, request)?;
    log_elapsed("ct2_backend_request_defaults", defaults_start);
    let runtime_start = std::time::Instant::now();
    let result = translate_with_runtime(bundle, &request);
    log_elapsed("ct2_backend_translate_with_runtime", runtime_start);
    if result.is_ok() {
        span.finish();
    }
    result
}

pub fn translate_batch(
    bundle: &TranslationModelBundle,
    requests: &[TranslationRequest],
) -> Result<Vec<TranslationResult>, TranslateEngineError> {
    let span = PerfSpan::new("ct2_backend_translate_batch_total")
        .field("model_id", &bundle.manifest.id)
        .field("requests", requests.len());
    if requests.is_empty() {
        span.finish();
        return Ok(Vec::new());
    }

    let defaults_start = std::time::Instant::now();
    let requests = requests
        .iter()
        .map(|request| request_with_model_defaults(bundle, request))
        .collect::<Result<Vec<_>, _>>()?;
    log_elapsed("ct2_backend_batch_request_defaults", defaults_start);
    let runtime_start = std::time::Instant::now();
    let result = translate_batch_with_runtime(bundle, &requests);
    log_elapsed("ct2_backend_batch_translate_with_runtime", runtime_start);
    if result.is_ok() {
        span.finish();
    }
    result
}

pub fn runtime_status() -> &'static str {
    if cfg!(feature = "local-translate-ct2") {
        "local-translate-ct2-enabled"
    } else {
        "local-translate-ct2-disabled"
    }
}

fn request_with_model_defaults(
    bundle: &TranslationModelBundle,
    request: &TranslationRequest,
) -> Result<TranslationRequest, TranslateEngineError> {
    let span = PerfSpan::new("ct2_backend_request_with_model_defaults")
        .field("model_id", &bundle.manifest.id)
        .field("target", &request.target_language.0);
    let mut request = request.clone();
    let source_start = std::time::Instant::now();
    request.source_language = resolved_source_language(bundle, request.source_language.as_ref());
    log_elapsed("ct2_backend_resolve_source_language", source_start);

    let pair_start = std::time::Instant::now();
    if !bundle.supports_language_pair(
        request
            .source_language
            .as_ref()
            .map(|language| language.0.as_str()),
        &request.target_language.0,
    ) {
        return Err(TranslateEngineError::new(
            "translation_language_pair_unsupported",
            format!(
                "translation model '{}' does not support '{} -> {}'",
                bundle.manifest.id,
                request
                    .source_language
                    .as_ref()
                    .map(|language| language.0.as_str())
                    .unwrap_or("auto"),
                request.target_language.0
            ),
        ));
    }
    log_elapsed("ct2_backend_validate_language_pair", pair_start);
    span.finish();

    Ok(request)
}

fn resolved_source_language(
    bundle: &TranslationModelBundle,
    requested: Option<&LanguageCode>,
) -> Option<LanguageCode> {
    let requested = requested
        .map(|language| language.0.trim())
        .filter(|language| !language.is_empty() && !language.eq_ignore_ascii_case("auto"));

    requested
        .map(LanguageCode::new)
        .or_else(|| bundle.default_source_language().map(LanguageCode::new))
}

#[cfg(feature = "local-translate-ct2")]
fn translate_with_runtime(
    bundle: &TranslationModelBundle,
    request: &TranslationRequest,
) -> Result<TranslationResult, TranslateEngineError> {
    translate_batch_with_runtime(bundle, std::slice::from_ref(request)).and_then(|mut results| {
        results.pop().ok_or_else(|| {
            TranslateEngineError::new(
                "local_translation_empty",
                "CTranslate2 returned no translation hypotheses",
            )
        })
    })
}

#[cfg(feature = "local-translate-ct2")]
fn translate_batch_with_runtime(
    bundle: &TranslationModelBundle,
    requests: &[TranslationRequest],
) -> Result<Vec<TranslationResult>, TranslateEngineError> {
    use ct2rs::{
        Config, Device, TranslationOptions, Translator, tokenizers::sentencepiece::Tokenizer,
    };

    let model_dir = bundle.model.parent().ok_or_else(|| {
        TranslateEngineError::new(
            "translation_model_path_invalid",
            format!(
                "translation model '{}' has no parent directory",
                bundle.model.display()
            ),
        )
    })?;
    let tokenizer_start = std::time::Instant::now();
    let tokenizer = Tokenizer::from_file(&bundle.source_tokenizer, &bundle.target_tokenizer)
        .map_err(|error| {
            TranslateEngineError::new(
                "translation_tokenizer_load_failed",
                format!(
                    "failed to load sentencepiece tokenizers '{}' and '{}': {error}",
                    bundle.source_tokenizer.display(),
                    bundle.target_tokenizer.display()
                ),
            )
        })?;
    log_elapsed("ct2_backend_load_tokenizer", tokenizer_start);
    let config = Config {
        device: Device::CPU,
        ..Config::default()
    };
    let translator_start = std::time::Instant::now();
    let translator =
        Translator::with_tokenizer(model_dir, tokenizer, &config).map_err(|error| {
            TranslateEngineError::new(
                "translation_engine_load_failed",
                format!(
                    "failed to load CTranslate2 model '{}': {error}",
                    model_dir.display()
                ),
            )
        })?;
    log_elapsed("ct2_backend_create_translator", translator_start);

    let sources = requests
        .iter()
        .map(|request| request.source_text.clone())
        .collect::<Vec<_>>();
    let batch_start = std::time::Instant::now();
    let results = translator
        .translate_batch(&sources, &TranslationOptions::default(), None)
        .map_err(|error| {
            TranslateEngineError::new("local_translation_failed", error.to_string())
        })?;
    log_elapsed("ct2_backend_translate_batch", batch_start);
    let normalize_start = std::time::Instant::now();
    if results.len() != requests.len() {
        return Err(TranslateEngineError::new(
            "local_translation_result_count_mismatch",
            format!(
                "CTranslate2 returned {} translations for {} requests",
                results.len(),
                requests.len()
            ),
        ));
    }
    let translations = requests
        .iter()
        .zip(results)
        .map(|(request, (translated_text, _score))| TranslationResult {
            request_id: request.id.clone(),
            source_text: request.source_text.clone(),
            translated_text,
            source_language: request.source_language.clone(),
            target_language: request.target_language.clone(),
            provider: request.provider.clone(),
        })
        .collect::<Vec<_>>();
    log_elapsed("ct2_backend_normalize_result", normalize_start);

    Ok(translations)
}

#[cfg(not(feature = "local-translate-ct2"))]
fn translate_with_runtime(
    _bundle: &TranslationModelBundle,
    _request: &TranslationRequest,
) -> Result<TranslationResult, TranslateEngineError> {
    let span = PerfSpan::new("ct2_backend_translate_with_runtime_disabled");
    span.finish();
    Err(TranslateEngineError::new(
        "local_translate_runtime_disabled",
        "local translation runtime is not compiled; enable the 'local-translate-ct2' feature to use CTranslate2 translation",
    ))
}

#[cfg(not(feature = "local-translate-ct2"))]
fn translate_batch_with_runtime(
    _bundle: &TranslationModelBundle,
    _requests: &[TranslationRequest],
) -> Result<Vec<TranslationResult>, TranslateEngineError> {
    let span = PerfSpan::new("ct2_backend_translate_batch_with_runtime_disabled");
    span.finish();
    Err(TranslateEngineError::new(
        "local_translate_runtime_disabled",
        "local translation runtime is not compiled; enable the 'local-translate-ct2' feature to use CTranslate2 translation",
    ))
}

#[cfg(test)]
mod tests {
    use shared_models::{
        LanguageCode, ModelDomain, ModelFile, ModelManifest, ModelSource, TranslateLocalBackend,
        TranslateProvider, TranslationRequest,
    };

    use crate::TranslationModelBundle;

    use super::{request_with_model_defaults, translate};

    fn bundle() -> TranslationModelBundle {
        TranslationModelBundle {
            manifest: ModelManifest {
                id: "opus-mt-en-zh-ct2-int8".to_owned(),
                name: "OPUS-MT English to Chinese CTranslate2 int8".to_owned(),
                domain: ModelDomain::Translation,
                family: "opus-mt".to_owned(),
                backend: "ctranslate2".to_owned(),
                version: "marian".to_owned(),
                source_languages: vec!["en".to_owned()],
                target_languages: vec!["zh-CN".to_owned()],
                quantization: Some("int8".to_owned()),
                low_spec_friendly: true,
                multilingual: false,
                source: ModelSource::BuiltIn,
                files: vec![
                    ModelFile::required("model", "model.bin"),
                    ModelFile::required("config", "config.json"),
                    ModelFile::required("source_tokenizer", "source.spm"),
                    ModelFile::required("target_tokenizer", "target.spm"),
                ],
            },
            model: "model.bin".into(),
            config: "config.json".into(),
            source_tokenizer: "source.spm".into(),
            target_tokenizer: "target.spm".into(),
            vocabulary: None,
        }
    }

    fn request(source_language: Option<&str>, target_language: &str) -> TranslationRequest {
        TranslationRequest {
            id: "translate-test".to_owned(),
            source_text: "hello".to_owned(),
            source_language: source_language.map(LanguageCode::new),
            target_language: LanguageCode::new(target_language),
            provider: TranslateProvider::Local(TranslateLocalBackend::CTranslate2),
            model_id: Some("opus-mt-en-zh-ct2-int8".to_owned()),
            context: None,
        }
    }

    #[test]
    fn rejects_unsupported_language_pair_before_runtime() {
        let error = translate(&bundle(), &request(Some("en"), "ja")).unwrap_err();

        assert_eq!(error.code, "translation_language_pair_unsupported");
    }

    #[test]
    fn defaults_missing_source_language_from_single_source_model() {
        let request = request_with_model_defaults(&bundle(), &request(None, "zh-CN")).unwrap();

        assert_eq!(request.source_language, Some(LanguageCode::new("en")));
    }

    #[test]
    fn defaults_auto_source_language_from_single_source_model() {
        let request =
            request_with_model_defaults(&bundle(), &request(Some("auto"), "zh-CN")).unwrap();

        assert_eq!(request.source_language, Some(LanguageCode::new("en")));
    }

    #[test]
    fn rejects_explicit_unsupported_source_language() {
        let error =
            request_with_model_defaults(&bundle(), &request(Some("fr"), "zh-CN")).unwrap_err();

        assert_eq!(error.code, "translation_language_pair_unsupported");
    }
}
