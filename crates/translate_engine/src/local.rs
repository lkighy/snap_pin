use shared_models::{
    ModelManifest, TranslateLocalBackend, TranslateProvider, TranslationRequest, TranslationResult,
};

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
        let bundle = TranslationModelBundle::from_manifest(model)?;
        self.loaded_bundle = Some(bundle);
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

        ct2_backend::translate(bundle, request)
    }
}
