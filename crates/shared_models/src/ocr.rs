use crate::{ImageId, Rect};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcrProvider {
    Disabled,
    System,
    Local(OcrLocalBackend),
    ExternalApi(OcrExternalProvider),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcrLocalBackend {
    Mnn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcrExternalProvider {
    CustomHttp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcrRunMode {
    Standard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrProviderProfile {
    pub id: String,
    pub provider: OcrExternalProvider,
    pub endpoint: Option<String>,
    pub model: Option<String>,
    pub language_hint: Option<String>,
    pub timeout_ms: u64,
    pub retry_limit: u8,
    pub privacy_notice_acknowledged: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrJob {
    pub id: String,
    pub image_id: ImageId,
    pub source_rect: Option<Rect>,
    pub language_hint: Option<String>,
    pub provider: OcrProvider,
    pub provider_profile_id: Option<String>,
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrTextBlock {
    pub text: String,
    pub bounds: Rect,
    pub confidence: Option<f32>,
    pub language: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrResult {
    pub job_id: String,
    pub image_id: ImageId,
    pub blocks: Vec<OcrTextBlock>,
    pub plain_text: String,
}
