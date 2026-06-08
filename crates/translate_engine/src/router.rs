use shared_models::{
    ModelManifest, TranslateLocalBackend, TranslateProvider, TranslationRequest, TranslationResult,
};

use crate::{CTranslate2LocalEngine, MockTranslateEngine, TranslateEngine, TranslateEngineError};

#[derive(Debug, Clone)]
pub struct RoutedTranslateEngine {
    local_ct2: CTranslate2LocalEngine,
    mock: MockTranslateEngine,
}

impl Default for RoutedTranslateEngine {
    fn default() -> Self {
        Self {
            local_ct2: CTranslate2LocalEngine::default(),
            mock: MockTranslateEngine::default(),
        }
    }
}

impl TranslateEngine for RoutedTranslateEngine {
    fn provider(&self) -> TranslateProvider {
        TranslateProvider::Local(TranslateLocalBackend::CTranslate2)
    }

    fn load_model(&mut self, model: &ModelManifest) -> Result<(), TranslateEngineError> {
        match model.backend.as_str() {
            "ctranslate2" | "ct2" => self.local_ct2.load_model(model),
            _ => self.mock.load_model(model),
        }
    }

    fn translate(
        &self,
        request: &TranslationRequest,
    ) -> Result<TranslationResult, TranslateEngineError> {
        match &request.provider {
            TranslateProvider::Disabled => Err(TranslateEngineError::new(
                "translation_disabled",
                "translation is disabled for this request",
            )),
            TranslateProvider::Local(TranslateLocalBackend::CTranslate2) => {
                self.local_ct2.translate(request)
            }
            TranslateProvider::Local(TranslateLocalBackend::Custom(_)) => {
                self.mock.translate(request)
            }
            TranslateProvider::ExternalApi(_) => Err(TranslateEngineError::new(
                "translation_api_not_implemented",
                "external translation APIs are scheduled after the local CTranslate2 MVP",
            )),
            TranslateProvider::Experimental(_) => Err(TranslateEngineError::new(
                "translation_experimental_not_implemented",
                "experimental translation backends are scheduled after the local CTranslate2 MVP",
            )),
            TranslateProvider::Custom(_) => self.mock.translate(request),
        }
    }

    fn translate_batch(
        &self,
        requests: &[TranslationRequest],
    ) -> Result<Vec<TranslationResult>, TranslateEngineError> {
        let Some(first) = requests.first() else {
            return Ok(Vec::new());
        };

        match &first.provider {
            TranslateProvider::Disabled => Err(TranslateEngineError::new(
                "translation_disabled",
                "translation is disabled for this request",
            )),
            TranslateProvider::Local(TranslateLocalBackend::CTranslate2) => {
                self.local_ct2.translate_batch(requests)
            }
            TranslateProvider::Local(TranslateLocalBackend::Custom(_)) => {
                self.mock.translate_batch(requests)
            }
            TranslateProvider::ExternalApi(_) => Err(TranslateEngineError::new(
                "translation_api_not_implemented",
                "external translation APIs are scheduled after the local CTranslate2 MVP",
            )),
            TranslateProvider::Experimental(_) => Err(TranslateEngineError::new(
                "translation_experimental_not_implemented",
                "experimental translation backends are scheduled after the local CTranslate2 MVP",
            )),
            TranslateProvider::Custom(_) => self.mock.translate_batch(requests),
        }
    }
}
