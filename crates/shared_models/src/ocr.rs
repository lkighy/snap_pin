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
    OnnxRuntime,
    PaddleRuntime,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcrExternalProvider {
    OpenAi,
    AzureVision,
    GoogleVision,
    BaiduOcr,
    TencentOcr,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrJob {
    pub id: String,
    pub image_id: ImageId,
    pub source_rect: Option<Rect>,
    pub language_hint: Option<String>,
    pub provider: OcrProvider,
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
