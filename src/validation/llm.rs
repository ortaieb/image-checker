use base64::{engine::general_purpose, Engine as _};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;
use tracing::{debug, error, warn};

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("IO error reading image: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("API error: {0}")]
    Api(String),
    #[error("Invalid image format: {0}")]
    InvalidImage(String),
    #[error("Timeout error")]
    Timeout,
    #[error("Maximum retries exceeded")]
    MaxRetriesExceeded,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: Vec<ContentItem>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum ContentItem {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Serialize)]
struct ImageUrl {
    url: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: String,
}

#[derive(Debug, Clone)]
pub struct LlmClient {
    client: Client,
    api_url: String,
    model_name: String,
    max_retries: u32,
}

impl LlmClient {
    pub fn new(api_url: String, model_name: String, timeout: Duration) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_url,
            model_name,
            max_retries: 3,
        }
    }

    pub async fn validate_image_content<P: AsRef<Path>>(
        &self,
        image_path: P,
        content_description: &str,
    ) -> Result<String, LlmError> {
        debug!("Validating image content for: {}", content_description);

        let image_data = self.read_and_encode_image(&image_path).await?;
        let prompt = self.construct_validation_prompt(content_description);

        let response = self.call_llm_with_retry(&prompt, &image_data).await?;

        debug!("LLM response received: {} chars", response.len());
        Ok(response)
    }

    async fn read_and_encode_image<P: AsRef<Path>>(
        &self,
        image_path: P,
    ) -> Result<String, LlmError> {
        let path = image_path.as_ref();

        // Validate file exists
        if !path.exists() {
            return Err(LlmError::InvalidImage(format!(
                "Image file not found: {path:?}"
            )));
        }

        // Read image file
        let image_bytes = fs::read(path)?;

        // Validate image format by checking file extension and magic bytes
        self.validate_image_format(path, &image_bytes)?;

        // Encode to base64
        let base64_data = general_purpose::STANDARD.encode(&image_bytes);
        let mime_type = self.get_mime_type(path)?;

        Ok(format!("data:{mime_type};base64,{base64_data}"))
    }

    fn validate_image_format<P: AsRef<Path>>(&self, path: P, bytes: &[u8]) -> Result<(), LlmError> {
        let path = path.as_ref();
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Check file extension
        if !matches!(
            extension.as_str(),
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp"
        ) {
            return Err(LlmError::InvalidImage(format!(
                "Unsupported image extension: {extension}"
            )));
        }

        // Check magic bytes for common formats
        if bytes.len() < 8 {
            return Err(LlmError::InvalidImage("Image file too small".into()));
        }

        let is_valid = match extension.as_str() {
            "jpg" | "jpeg" => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
            "png" => bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]),
            "gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
            "bmp" => bytes.starts_with(b"BM"),
            "webp" => bytes[8..12] == *b"WEBP",
            _ => true, // Allow other formats to pass through
        };

        if !is_valid {
            return Err(LlmError::InvalidImage(format!(
                "Invalid {extension} file format"
            )));
        }

        Ok(())
    }

    fn get_mime_type<P: AsRef<Path>>(&self, path: P) -> Result<String, LlmError> {
        let extension = path
            .as_ref()
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        let mime_type = match extension.as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "bmp" => "image/bmp",
            "webp" => "image/webp",
            _ => {
                return Err(LlmError::InvalidImage(format!(
                    "Unknown MIME type for extension: {extension}"
                )))
            }
        };

        Ok(mime_type.to_string())
    }

    fn construct_validation_prompt(&self, content_description: &str) -> String {
        format!(
            "You are an image validation assistant. Please analyze this image and determine if it matches the following description: \"{content_description}\"\n\n\
            Respond with either:\n\
            - \"ACCEPTED\" if the image clearly matches the description\n\
            - \"REJECTED: [reason]\" if the image does not match, followed by a brief explanation\n\n\
            Be precise and focus on the key elements mentioned in the description. If the description mentions specific objects, locations, or characteristics, verify their presence in the image."
        )
    }

    async fn call_llm_with_retry(
        &self,
        prompt: &str,
        image_data: &str,
    ) -> Result<String, LlmError> {
        let mut attempt = 0;
        let mut delay = Duration::from_millis(1000); // Start with 1 second

        while attempt < self.max_retries {
            match self.call_llm(prompt, image_data).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    attempt += 1;

                    if attempt >= self.max_retries {
                        error!("Max retries exceeded for LLM call: {}", e);
                        return Err(LlmError::MaxRetriesExceeded);
                    }

                    warn!(
                        "LLM call failed (attempt {}): {}. Retrying in {:?}",
                        attempt, e, delay
                    );
                    sleep(delay).await;

                    // Exponential backoff with jitter
                    delay = std::cmp::min(delay * 2, Duration::from_secs(30));
                }
            }
        }

        Err(LlmError::MaxRetriesExceeded)
    }

    async fn call_llm(&self, prompt: &str, image_data: &str) -> Result<String, LlmError> {
        let request = ChatCompletionRequest {
            model: self.model_name.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: vec![
                    ContentItem::Text {
                        text: prompt.to_string(),
                    },
                    ContentItem::ImageUrl {
                        image_url: ImageUrl {
                            url: image_data.to_string(),
                        },
                    },
                ],
            }],
            max_tokens: Some(500),
            temperature: Some(0.1), // Low temperature for consistent responses
        };

        debug!("Sending request to LLM API: {}", self.api_url);

        let response = self
            .client
            .post(&self.api_url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(LlmError::Api(format!("HTTP {status}: {error_text}")));
        }

        let completion: ChatCompletionResponse = response.json().await?;

        if completion.choices.is_empty() {
            return Err(LlmError::Api("No choices in response".into()));
        }

        Ok(completion.choices[0].message.content.trim().to_string())
    }
}

pub async fn validate_image_content<P: AsRef<Path>>(
    client: &LlmClient,
    image_path: P,
    content_description: &str,
) -> Result<bool, LlmError> {
    let response = client
        .validate_image_content(image_path, content_description)
        .await?;

    // Parse the response to determine if validation passed
    let is_accepted = response.to_uppercase().starts_with("ACCEPTED");

    debug!("Content validation result: {} -> {}", response, is_accepted);

    Ok(is_accepted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_construct_validation_prompt() {
        let client = LlmClient::new(
            "http://localhost:8080".into(),
            "llava:7b".into(),
            Duration::from_secs(30),
        );

        let prompt = client.construct_validation_prompt("Three birds on a wire");

        assert!(prompt.contains("Three birds on a wire"));
        assert!(prompt.contains("ACCEPTED"));
        assert!(prompt.contains("REJECTED"));
    }

    #[test]
    fn test_get_mime_type() {
        let client = LlmClient::new(
            "http://localhost:8080".into(),
            "llava:7b".into(),
            Duration::from_secs(30),
        );

        assert_eq!(client.get_mime_type("test.jpg").unwrap(), "image/jpeg");
        assert_eq!(client.get_mime_type("test.png").unwrap(), "image/png");
        assert_eq!(client.get_mime_type("test.gif").unwrap(), "image/gif");

        assert!(client.get_mime_type("test.txt").is_err());
    }

    #[tokio::test]
    async fn test_validate_image_format() {
        let client = LlmClient::new(
            "http://localhost:8080".into(),
            "llava:7b".into(),
            Duration::from_secs(30),
        );

        // Create a temporary file with JPEG magic bytes
        let mut temp_file = NamedTempFile::with_suffix(".jpg").unwrap();
        temp_file
            .write_all(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46])
            .unwrap();

        let result = client.validate_image_format(
            temp_file.path(),
            &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46],
        );
        assert!(result.is_ok());

        // Test invalid magic bytes
        let result = client.validate_image_format(
            temp_file.path(),
            &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        );
        assert!(result.is_err());
    }

    // Integration tests with real LLM API should be in tests/ directory
    // as they require a running LLaVa service
}
