use std::time::Duration;

use base64::Engine;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use shared_models::{
    ImageData, ImageFormat, OcrExternalProvider, OcrJob, OcrProvider, OcrProviderProfile,
    OcrResult, OcrTextBlock, Point, Rect, Size,
};

use crate::{ExternalOcrClient, OcrEngineError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpOcrClient {
    provider: OcrExternalProvider,
}

impl HttpOcrClient {
    pub fn new(provider: OcrExternalProvider) -> Self {
        Self { provider }
    }
}

impl ExternalOcrClient for HttpOcrClient {
    fn provider(&self) -> OcrProvider {
        OcrProvider::ExternalApi(self.provider.clone())
    }

    fn recognize_remote(
        &self,
        profile: &OcrProviderProfile,
        job: &OcrJob,
        image: &ImageData,
    ) -> Result<OcrResult, OcrEngineError> {
        validate_profile(profile, &self.provider)?;

        match &self.provider {
            OcrExternalProvider::Custom(_) => recognize_custom_http(profile, job, image),
            provider => Err(OcrEngineError::new(
                "external_ocr_provider_unimplemented",
                format!(
                    "external OCR provider '{}' needs a dedicated API adapter before it can run",
                    external_provider_name(provider)
                ),
            )),
        }
    }
}

pub fn validate_profile(
    profile: &OcrProviderProfile,
    expected_provider: &OcrExternalProvider,
) -> Result<(), OcrEngineError> {
    if &profile.provider != expected_provider {
        return Err(OcrEngineError::new(
            "ocr_profile_provider_mismatch",
            format!(
                "OCR profile '{}' does not match requested provider",
                profile.id
            ),
        ));
    }

    if !profile.privacy_notice_acknowledged {
        return Err(OcrEngineError::new(
            "ocr_privacy_notice_required",
            format!(
                "external OCR profile '{}' requires privacy notice acknowledgement",
                profile.id
            ),
        ));
    }

    if profile.timeout_ms == 0 {
        return Err(OcrEngineError::new(
            "ocr_profile_invalid_timeout",
            format!("OCR profile '{}' has an invalid timeout", profile.id),
        ));
    }

    Ok(())
}

#[derive(Debug, Serialize)]
struct CustomHttpOcrRequest<'a> {
    image_base64: String,
    image_format: &'a str,
    language_hint: Option<&'a str>,
    return_blocks: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CustomHttpOcrResponse {
    #[serde(default)]
    plain_text: String,
    #[serde(default)]
    blocks: Vec<CustomHttpOcrBlock>,
    #[serde(default)]
    language: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CustomHttpOcrBlock {
    text: String,
    bounds: CustomHttpBounds,
    #[serde(default)]
    confidence: Option<f32>,
    #[serde(default)]
    language: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CustomHttpBounds {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

fn recognize_custom_http(
    profile: &OcrProviderProfile,
    job: &OcrJob,
    image: &ImageData,
) -> Result<OcrResult, OcrEngineError> {
    let endpoint = profile.endpoint.as_deref().ok_or_else(|| {
        OcrEngineError::new(
            "ocr_profile_endpoint_missing",
            format!("external OCR profile '{}' requires an endpoint", profile.id),
        )
    })?;

    let client = Client::builder()
        .timeout(Duration::from_millis(profile.timeout_ms))
        .build()
        .map_err(|error| OcrEngineError::new("external_ocr_client_failed", error.to_string()))?;
    let request = CustomHttpOcrRequest {
        image_base64: base64::engine::general_purpose::STANDARD.encode(&image.bytes),
        image_format: image_format_name(image.metadata.format),
        language_hint: job
            .language_hint
            .as_deref()
            .or(profile.language_hint.as_deref()),
        return_blocks: true,
    };

    let response = client
        .post(endpoint)
        .json(&request)
        .send()
        .map_err(|error| OcrEngineError::new("external_ocr_request_failed", error.to_string()))?;

    if !response.status().is_success() {
        return Err(OcrEngineError::new(
            "external_ocr_request_failed",
            format!("external OCR returned HTTP {}", response.status()),
        ));
    }

    let response = response
        .json::<CustomHttpOcrResponse>()
        .map_err(|error| OcrEngineError::new("external_ocr_response_invalid", error.to_string()))?;

    let blocks = response
        .blocks
        .into_iter()
        .map(|block| OcrTextBlock {
            text: block.text,
            bounds: Rect::new(
                Point::new(block.bounds.x, block.bounds.y),
                Size::new(block.bounds.width, block.bounds.height),
            ),
            confidence: block.confidence,
            language: block.language.or_else(|| response.language.clone()),
        })
        .collect::<Vec<_>>();
    let plain_text = if response.plain_text.is_empty() {
        blocks
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        response.plain_text
    };

    Ok(OcrResult {
        job_id: job.id.clone(),
        image_id: image.id.clone(),
        blocks,
        plain_text,
    })
}

fn image_format_name(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Bgra8 => "bgra8",
        ImageFormat::Rgba8 => "rgba8",
        ImageFormat::Png => "png",
    }
}

fn external_provider_name(provider: &OcrExternalProvider) -> &str {
    match provider {
        OcrExternalProvider::OpenAi => "openai",
        OcrExternalProvider::AzureVision => "azure-vision",
        OcrExternalProvider::GoogleVision => "google-vision",
        OcrExternalProvider::BaiduOcr => "baidu-ocr",
        OcrExternalProvider::TencentOcr => "tencent-ocr",
        OcrExternalProvider::Custom(_) => "custom-http",
    }
}

#[cfg(test)]
mod tests {
    use shared_models::{OcrExternalProvider, OcrProviderProfile};

    use super::validate_profile;

    #[test]
    fn requires_privacy_notice_for_external_profile() {
        let profile = OcrProviderProfile {
            id: "custom".to_owned(),
            provider: OcrExternalProvider::Custom("custom".to_owned()),
            endpoint: Some("http://127.0.0.1:9999/ocr".to_owned()),
            model: None,
            language_hint: None,
            timeout_ms: 1_000,
            retry_limit: 0,
            privacy_notice_acknowledged: false,
        };

        let error = validate_profile(&profile, &OcrExternalProvider::Custom("custom".to_owned()))
            .unwrap_err();

        assert_eq!(error.code, "ocr_privacy_notice_required");
    }
}
