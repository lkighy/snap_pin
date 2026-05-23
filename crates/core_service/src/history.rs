use shared_models::{OcrResult, TranslationResult};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct HistoryStore {
    ocr_results: Vec<OcrResult>,
    translations: Vec<TranslationResult>,
}

impl HistoryStore {
    pub fn push_ocr(&mut self, result: OcrResult) {
        self.ocr_results.push(result);
    }

    pub fn push_translation(&mut self, result: TranslationResult) {
        self.translations.push(result);
    }

    pub fn ocr_results(&self) -> &[OcrResult] {
        &self.ocr_results
    }

    pub fn translations(&self) -> &[TranslationResult] {
        &self.translations
    }
}
