use shared_models::{ModelManifest, TranslateProvider, TranslationRequest, TranslationResult};

use crate::TranslateEngineError;

pub trait TranslateEngine {
    fn provider(&self) -> TranslateProvider;
    fn load_model(&mut self, model: &ModelManifest) -> Result<(), TranslateEngineError>;
    fn translate(
        &self,
        request: &TranslationRequest,
    ) -> Result<TranslationResult, TranslateEngineError>;

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
