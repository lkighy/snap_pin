use shared_models::{
    ModelManifest, TranslateLocalBackend, TranslateProvider, TranslationRequest, TranslationResult,
};

use crate::{TranslateEngine, TranslateEngineError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockTranslateEngine {
    provider: TranslateProvider,
    loaded_model_id: Option<String>,
}

impl Default for MockTranslateEngine {
    fn default() -> Self {
        Self {
            provider: TranslateProvider::Local(TranslateLocalBackend::CTranslate2),
            loaded_model_id: None,
        }
    }
}

impl TranslateEngine for MockTranslateEngine {
    fn provider(&self) -> TranslateProvider {
        self.provider.clone()
    }

    fn load_model(&mut self, model: &ModelManifest) -> Result<(), TranslateEngineError> {
        self.loaded_model_id = Some(model.id.clone());
        Ok(())
    }

    fn translate(
        &self,
        request: &TranslationRequest,
    ) -> Result<TranslationResult, TranslateEngineError> {
        let model = self
            .loaded_model_id
            .as_deref()
            .or(request.model_id.as_deref())
            .unwrap_or("unloaded-model");

        Ok(TranslationResult {
            request_id: request.id.clone(),
            source_text: request.source_text.clone(),
            translated_text: format!(
                "[{} -> {} via {}] {}",
                request
                    .source_language
                    .as_ref()
                    .map(|language| language.0.as_str())
                    .unwrap_or("auto"),
                request.target_language.0,
                model,
                request.source_text
            ),
            source_language: request.source_language.clone(),
            target_language: request.target_language.clone(),
            provider: request.provider.clone(),
        })
    }

    fn translate_batch(
        &self,
        requests: &[TranslationRequest],
    ) -> Result<Vec<TranslationResult>, TranslateEngineError> {
        requests
            .iter()
            .map(|request| self.translate(request))
            .collect()
    }
}
