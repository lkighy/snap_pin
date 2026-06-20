#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslateProvider {
    Disabled,
    Local(TranslateLocalBackend),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslateLocalBackend {
    CTranslate2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageCode(pub String);

impl LanguageCode {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationRequest {
    pub id: String,
    pub source_text: String,
    pub source_language: Option<LanguageCode>,
    pub target_language: LanguageCode,
    pub provider: TranslateProvider,
    pub model_id: Option<String>,
    pub context: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationResult {
    pub request_id: String,
    pub source_text: String,
    pub translated_text: String,
    pub source_language: Option<LanguageCode>,
    pub target_language: LanguageCode,
    pub provider: TranslateProvider,
}
