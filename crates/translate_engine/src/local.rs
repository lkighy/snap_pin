use shared_models::{
    ModelManifest, TranslateLocalBackend, TranslateProvider, TranslationRequest, TranslationResult,
};

use perf_trace::{PerfSpan, log_elapsed};

use crate::{TranslateEngine, TranslateEngineError, TranslationModelBundle, ct2_backend};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CTranslate2LocalEngine {
    loaded_bundle: Option<TranslationModelBundle>,
}

impl CTranslate2LocalEngine {
    pub fn new() -> Self {
        Self {
            loaded_bundle: None,
        }
    }
}

impl Default for CTranslate2LocalEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TranslateEngine for CTranslate2LocalEngine {
    fn provider(&self) -> TranslateProvider {
        TranslateProvider::Local(TranslateLocalBackend::CTranslate2)
    }

    fn load_model(&mut self, model: &ModelManifest) -> Result<(), TranslateEngineError> {
        let span = PerfSpan::new("translate_engine_load_model_total")
            .field("model_id", &model.id)
            .field("backend", &model.backend);
        let bundle_start = std::time::Instant::now();
        let bundle = TranslationModelBundle::from_manifest(model)?;
        log_elapsed("translate_engine_model_bundle_from_manifest", bundle_start);
        self.loaded_bundle = Some(bundle);
        span.finish();
        Ok(())
    }

    fn translate(
        &self,
        request: &TranslationRequest,
    ) -> Result<TranslationResult, TranslateEngineError> {
        let Some(bundle) = self.loaded_bundle.as_ref() else {
            return Err(TranslateEngineError::new(
                "translation_model_not_loaded",
                "local translation requires a validated CTranslate2 model bundle before translation",
            ));
        };

        if request
            .model_id
            .as_deref()
            .is_some_and(|id| id != bundle.manifest.id)
        {
            return Err(TranslateEngineError::new(
                "translation_model_mismatch",
                format!(
                    "translation request expects model '{}' but loaded model is '{}'",
                    request.model_id.as_deref().unwrap_or_default(),
                    bundle.manifest.id
                ),
            ));
        }

        let span = PerfSpan::new("translate_engine_translate_total")
            .field("model_id", &bundle.manifest.id)
            .field("target", &request.target_language.0)
            .field("source_chars", request.source_text.chars().count());
        let result = ct2_backend::translate(bundle, request);
        if result.is_ok() {
            span.finish();
        }
        result
    }

    fn translate_batch(
        &self,
        requests: &[TranslationRequest],
    ) -> Result<Vec<TranslationResult>, TranslateEngineError> {
        let Some(bundle) = self.loaded_bundle.as_ref() else {
            return Err(TranslateEngineError::new(
                "translation_model_not_loaded",
                "local translation requires a validated CTranslate2 model bundle before translation",
            ));
        };

        if let Some(request) = requests.iter().find(|request| {
            request
                .model_id
                .as_deref()
                .is_some_and(|id| id != bundle.manifest.id)
        }) {
            return Err(TranslateEngineError::new(
                "translation_model_mismatch",
                format!(
                    "translation request expects model '{}' but loaded model is '{}'",
                    request.model_id.as_deref().unwrap_or_default(),
                    bundle.manifest.id
                ),
            ));
        }

        let source_chars = requests
            .iter()
            .map(|request| request.source_text.chars().count())
            .sum::<usize>();
        let span = PerfSpan::new("translate_engine_translate_batch_total")
            .field("model_id", &bundle.manifest.id)
            .field("requests", requests.len())
            .field("source_chars", source_chars);
        let result = ct2_backend::translate_batch(bundle, requests);
        if result.is_ok() {
            span.finish();
        }
        result
    }
}
