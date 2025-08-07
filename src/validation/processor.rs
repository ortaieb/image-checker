use crate::config::Config;
use crate::models::{ProcessingRequest, Resolution, ValidationContext, ValidationResults};
use crate::utils::{coords_to_string, format_distance, validate_datetime, validate_location};
use crate::validation::exif::{extract_exif_metadata, ExifError};
use crate::validation::llm::{validate_image_content, LlmClient, LlmError};

use std::path::Path;
use thiserror::Error;
use tokio::try_join;
use tracing::{debug, error, info, warn};

#[derive(Debug, Error)]
pub enum ProcessorError {
    #[error("Image file not found: {0}")]
    ImageNotFound(String),
    #[error("EXIF processing error: {0}")]
    Exif(#[from] ExifError),
    #[error("LLM processing error: {0}")]
    Llm(#[from] LlmError),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Validation context error: {0}")]
    ValidationContext(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

pub struct ValidationProcessor {
    llm_client: LlmClient,
    image_base_dir: String,
}

impl ValidationProcessor {
    pub fn new(config: &Config) -> Self {
        let llm_client = LlmClient::new(
            config.llm_api_url.clone(),
            config.llm_model_name.clone(),
            config.request_timeout(),
        );

        Self {
            llm_client,
            image_base_dir: config.image_base_dir.clone(),
        }
    }

    pub async fn validate_request(
        &self,
        request: ProcessingRequest,
    ) -> Result<ValidationResults, ProcessorError> {
        info!("Starting validation for request: {}", request.processing_id);

        // Determine image path before moving analysis_request
        let image_path = self.resolve_image_path(&request)?;

        // Parse validation context
        let context = ValidationContext::try_from(request.analysis_request)
            .map_err(ProcessorError::ValidationContext)?;

        // Validate image file exists
        if !Path::new(&image_path).exists() {
            warn!("Image file not found: {}", image_path);
            return Ok(ValidationResults {
                resolution: Resolution::Rejected,
                reasons: Some(vec!["cannot locate image".to_string()]),
            });
        }

        // Perform parallel validation of content and metadata
        let validation_result = self
            .perform_parallel_validation(&image_path, &context)
            .await;

        match validation_result {
            Ok((content_valid, location_valid, datetime_valid, reasons)) => {
                let overall_valid = content_valid && location_valid && datetime_valid;

                let result = if overall_valid {
                    info!("Validation passed for image: {}", image_path);
                    ValidationResults {
                        resolution: Resolution::Accepted,
                        reasons: None,
                    }
                } else {
                    info!(
                        "Validation failed for image: {} - reasons: {:?}",
                        image_path, reasons
                    );
                    ValidationResults {
                        resolution: Resolution::Rejected,
                        reasons: Some(reasons),
                    }
                };

                Ok(result)
            }
            Err(e) => {
                error!("Validation error for image {}: {}", image_path, e);
                Ok(ValidationResults {
                    resolution: Resolution::Rejected,
                    reasons: Some(vec![format!("validation error: {}", e)]),
                })
            }
        }
    }

    fn resolve_image_path(&self, request: &ProcessingRequest) -> Result<String, ProcessorError> {
        let image_path = request
            .get_image_path()
            .ok_or_else(|| ProcessorError::ImageNotFound("no image path provided".to_string()))?;

        // Handle both absolute paths and relative paths with image_base_dir
        if image_path.starts_with('/') {
            Ok(image_path)
        } else if image_path.starts_with("$image_base_dir/") {
            let relative_path = image_path.strip_prefix("$image_base_dir/").unwrap();
            Ok(format!("{}/{}", self.image_base_dir, relative_path))
        } else {
            Ok(format!("{}/{}", self.image_base_dir, image_path))
        }
    }

    async fn perform_parallel_validation(
        &self,
        image_path: &str,
        context: &ValidationContext,
    ) -> Result<(bool, bool, bool, Vec<String>), ProcessorError> {
        debug!("Performing parallel validation for: {}", image_path);

        // Perform content validation and EXIF extraction in parallel
        let (content_result, exif_result) = try_join!(
            self.validate_content(image_path, &context.content_check),
            self.extract_and_validate_metadata(image_path, context)
        )?;

        let mut reasons = Vec::new();

        // Process content validation result
        let content_valid = content_result;
        if !content_valid {
            reasons.push(format!(
                "image content does not match description: '{}'",
                context.content_check
            ));
        }

        // Process metadata validation result
        let (location_valid, datetime_valid, mut meta_reasons) = exif_result;
        reasons.append(&mut meta_reasons);

        debug!(
            "Validation results - content: {}, location: {}, datetime: {}",
            content_valid, location_valid, datetime_valid
        );

        Ok((content_valid, location_valid, datetime_valid, reasons))
    }

    async fn validate_content(
        &self,
        image_path: &str,
        content_description: &str,
    ) -> Result<bool, ProcessorError> {
        debug!("Validating image content: {}", content_description);

        let is_valid =
            validate_image_content(&self.llm_client, image_path, content_description).await?;

        debug!("Content validation result: {}", is_valid);
        Ok(is_valid)
    }

    async fn extract_and_validate_metadata(
        &self,
        image_path: &str,
        context: &ValidationContext,
    ) -> Result<(bool, bool, Vec<String>), ProcessorError> {
        debug!("Extracting and validating metadata");

        // Extract EXIF data
        let exif_data = extract_exif_metadata(image_path)?;
        let mut reasons = Vec::new();

        // Validate location constraint if present
        let location_valid = if let Some(location_constraint) = &context.location_constraint {
            match exif_data.gps_coordinates {
                Some(coords) => {
                    debug!("Found GPS coordinates: {}", coords_to_string(coords));

                    match validate_location(coords, location_constraint) {
                        Ok(valid) => {
                            if !valid {
                                let expected_coords =
                                    (location_constraint.latitude, location_constraint.longitude);
                                let actual_distance =
                                    crate::utils::haversine_distance(coords, expected_coords);
                                reasons.push(format!(
                                    "image location {} is {} from expected location {}, exceeding {} limit",
                                    coords_to_string(coords),
                                    format_distance(actual_distance),
                                    coords_to_string(expected_coords),
                                    format_distance(location_constraint.max_distance_meters)
                                ));
                            }
                            valid
                        }
                        Err(e) => {
                            reasons.push(format!("location validation error: {e}"));
                            false
                        }
                    }
                }
                None => {
                    reasons.push("image does not contain GPS coordinates".to_string());
                    false
                }
            }
        } else {
            true // No location constraint, so it passes
        };

        // Validate datetime constraint if present
        let datetime_valid = if let Some(datetime_constraint) = &context.datetime_constraint {
            // Try to use DateTimeOriginal first, then DateTime
            let image_timestamp = exif_data.datetime_original.or(exif_data.timestamp);

            match image_timestamp {
                Some(timestamp) => {
                    debug!("Found image timestamp: {}", timestamp);

                    match validate_datetime(&timestamp, datetime_constraint) {
                        Ok(valid) => {
                            if !valid {
                                let time_diff = if timestamp < datetime_constraint.start_time {
                                    format!(
                                        "{} minutes before allowed start time",
                                        (datetime_constraint.start_time - timestamp).num_minutes()
                                    )
                                } else {
                                    format!(
                                        "{} minutes after allowed end time",
                                        (timestamp - datetime_constraint.end_time).num_minutes()
                                    )
                                };

                                reasons.push(format!(
                                    "image timestamp {} is {}, outside allowed time range {} to {}",
                                    timestamp.format("%Y-%m-%d %H:%M:%S %z"),
                                    time_diff,
                                    datetime_constraint
                                        .start_time
                                        .format("%Y-%m-%d %H:%M:%S %z"),
                                    datetime_constraint.end_time.format("%Y-%m-%d %H:%M:%S %z")
                                ));
                            }
                            valid
                        }
                        Err(e) => {
                            reasons.push(format!("datetime validation error: {e}"));
                            false
                        }
                    }
                }
                None => {
                    reasons.push("image does not contain timestamp information".to_string());
                    false
                }
            }
        } else {
            true // No datetime constraint, so it passes
        };

        debug!(
            "Metadata validation results - location: {}, datetime: {}",
            location_valid, datetime_valid
        );

        Ok((location_valid, datetime_valid, reasons))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AnalysisRequest, DateTimeRequest, LocationRequest};

    fn create_test_config() -> Config {
        Config {
            host: "127.0.0.1".to_string(),
            port: 3000,
            image_base_dir: "/tmp".to_string(),
            llm_api_url: "http://localhost:8080".to_string(),
            llm_model_name: "llava:7b".to_string(),
            request_timeout_seconds: 30,
            processing_timeout_minutes: 5,
            queue_size: 100,
            throttle_requests_per_minute: 60,
        }
    }

    #[test]
    fn test_resolve_image_path() {
        let config = create_test_config();
        let processor = ValidationProcessor::new(&config);

        // Test absolute path
        let request = ProcessingRequest {
            processing_id: "test".to_string(),
            image_path: Some("/absolute/path/image.jpg".to_string()),
            image: None,
            analysis_request: AnalysisRequest {
                image_path: None,
                content: "test".to_string(),
                location: None,
                datetime: None,
            },
        };

        let resolved = processor.resolve_image_path(&request).unwrap();
        assert_eq!(resolved, "/absolute/path/image.jpg");

        // Test relative path with $image_base_dir
        let request = ProcessingRequest {
            processing_id: "test".to_string(),
            image_path: Some("$image_base_dir/image.jpg".to_string()),
            image: None,
            analysis_request: AnalysisRequest {
                image_path: None,
                content: "test".to_string(),
                location: None,
                datetime: None,
            },
        };

        let resolved = processor.resolve_image_path(&request).unwrap();
        assert_eq!(resolved, "/tmp/image.jpg");
    }

    #[test]
    fn test_validation_context_creation() {
        let analysis_request = AnalysisRequest {
            image_path: None,
            content: "Three birds on a wire".to_string(),
            location: Some(LocationRequest {
                long: -0.266108,
                lat: 51.492191,
                max_distance: 100.0,
            }),
            datetime: Some(DateTimeRequest {
                start: Some("2025-08-01T15:23:00+01:00".to_string()),
                end: None,
                duration: Some(10), // 10 minutes
            }),
        };

        let context = ValidationContext::try_from(analysis_request).unwrap();

        assert_eq!(context.content_check, "Three birds on a wire");
        assert!(context.location_constraint.is_some());
        assert!(context.datetime_constraint.is_some());

        let location = context.location_constraint.unwrap();
        assert_eq!(location.max_distance_meters, 100.0);
        assert!((location.latitude - 51.492191).abs() < 0.000001);
        assert!((location.longitude + 0.266108).abs() < 0.000001);

        let datetime = context.datetime_constraint.unwrap();
        assert_eq!(
            datetime.start_time.format("%Y-%m-%d %H:%M").to_string(),
            "2025-08-01 15:23"
        );
        assert_eq!(
            datetime.end_time.format("%Y-%m-%d %H:%M").to_string(),
            "2025-08-01 15:33"
        );
    }

    // Integration tests with real image files and LLM API should be in tests/ directory
}
