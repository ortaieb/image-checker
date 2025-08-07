use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone)]
pub struct ValidationRequest {
    #[serde(rename = "processing-id")]
    pub processing_id: String,

    #[serde(rename = "image-path")]
    pub image_path: Option<String>,

    pub image: Option<String>,

    #[serde(rename = "analysis-request")]
    pub analysis_request: AnalysisRequest,
}

impl ValidationRequest {
    pub fn get_image_path(&self) -> Option<String> {
        self.image_path.clone().or_else(|| self.image.clone())
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct LocationRequest {
    pub long: f64,
    pub lat: f64,
    pub max_distance: f64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DateTimeRequest {
    pub start: Option<String>,
    pub end: Option<String>,
    pub duration: Option<u64>, // duration in minutes
}

#[derive(Debug, Deserialize, Clone)]
pub struct AnalysisRequest {
    #[serde(rename = "image-path")]
    pub image_path: Option<String>,

    pub content: String,

    pub location: Option<LocationRequest>,

    pub datetime: Option<DateTimeRequest>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ValidationResponse {
    #[serde(rename = "processing-id")]
    pub processing_id: String,

    pub results: ValidationResults,
}

#[derive(Debug, Serialize, Clone)]
pub struct ValidationResults {
    pub resolution: Resolution,

    #[serde(rename = "resons", skip_serializing_if = "Option::is_none")]
    pub reasons: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Resolution {
    Accepted,
    Rejected,
}

#[derive(Debug, Serialize, Clone)]
pub struct StatusResponse {
    #[serde(rename = "processing-id")]
    pub processing_id: String,

    pub status: ProcessingStatus,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProcessingStatus {
    Accepted,
    #[serde(rename = "in_progress")]
    InProgress,
    Completed,
    Failed,
    #[serde(rename = "not_found")]
    NotFound,
}

#[derive(Debug, Clone)]
pub struct LocationConstraint {
    pub max_distance_meters: f64,
    pub latitude: f64,
    pub longitude: f64,
}

impl From<LocationRequest> for LocationConstraint {
    fn from(request: LocationRequest) -> Self {
        LocationConstraint {
            max_distance_meters: request.max_distance,
            latitude: request.lat,
            longitude: request.long,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DateTimeConstraint {
    pub start_time: DateTime<FixedOffset>,
    pub end_time: DateTime<FixedOffset>,
}

impl TryFrom<DateTimeRequest> for DateTimeConstraint {
    type Error = String;

    fn try_from(request: DateTimeRequest) -> Result<Self, Self::Error> {
        use chrono::Duration;

        // Validate that we have exactly 2 out of 3 fields
        let field_count = [
            request.start.is_some(),
            request.end.is_some(),
            request.duration.is_some(),
        ]
        .iter()
        .filter(|&&x| x)
        .count();

        if field_count != 2 {
            return Err(
                "Exactly two out of three fields (start, end, duration) must be provided"
                    .to_string(),
            );
        }

        let parse_datetime = |dt_str: &str| -> Result<DateTime<FixedOffset>, String> {
            DateTime::parse_from_rfc3339(dt_str)
                .or_else(|_| {
                    // Try parsing custom format like "2025-08-01T15:23:00Z+1"
                    if let Some(base_dt) = dt_str.strip_suffix("Z+1") {
                        let dt_with_tz = format!("{}+01:00", base_dt.trim_end_matches('Z'));
                        DateTime::parse_from_rfc3339(&dt_with_tz)
                    } else {
                        DateTime::parse_from_rfc3339(dt_str)
                    }
                })
                .map_err(|_| format!("Invalid datetime format: {dt_str}"))
        };

        let (start_time, end_time) = match (request.start, request.end, request.duration) {
            // Case 1: start + end provided
            (Some(start_str), Some(end_str), None) => {
                let start = parse_datetime(&start_str)?;
                let end = parse_datetime(&end_str)?;

                if end <= start {
                    return Err("End time must be after start time".to_string());
                }

                (start, end)
            }

            // Case 2: start + duration provided
            (Some(start_str), None, Some(duration_minutes)) => {
                let start = parse_datetime(&start_str)?;
                let end = start + Duration::minutes(duration_minutes as i64);
                (start, end)
            }

            // Case 3: end + duration provided
            (None, Some(end_str), Some(duration_minutes)) => {
                let end = parse_datetime(&end_str)?;
                let start = end - Duration::minutes(duration_minutes as i64);
                (start, end)
            }

            _ => return Err("Invalid combination of fields provided".to_string()),
        };

        Ok(DateTimeConstraint {
            start_time,
            end_time,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ValidationContext {
    pub content_check: String,
    pub location_constraint: Option<LocationConstraint>,
    pub datetime_constraint: Option<DateTimeConstraint>,
}

impl TryFrom<AnalysisRequest> for ValidationContext {
    type Error = String;

    fn try_from(request: AnalysisRequest) -> Result<Self, Self::Error> {
        let location_constraint = request.location.map(LocationConstraint::from);

        let datetime_constraint = if let Some(datetime) = request.datetime {
            Some(DateTimeConstraint::try_from(datetime)?)
        } else {
            None
        };

        Ok(ValidationContext {
            content_check: request.content,
            location_constraint,
            datetime_constraint,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_location_constraint_from_request() {
        let location_request = LocationRequest {
            long: -0.266108,
            lat: 51.492191,
            max_distance: 100.0,
        };

        let constraint = LocationConstraint::from(location_request);

        assert_eq!(constraint.max_distance_meters, 100.0);
        assert!((constraint.latitude - 51.492191).abs() < 0.000001);
        assert!((constraint.longitude + 0.266108).abs() < 0.000001);
    }

    #[test]
    fn test_datetime_constraint_from_start_and_duration() {
        let datetime_request = DateTimeRequest {
            start: Some("2025-08-01T15:23:00+01:00".to_string()),
            end: None,
            duration: Some(10), // 10 minutes
        };

        let constraint = DateTimeConstraint::try_from(datetime_request).unwrap();

        assert_eq!(
            constraint.start_time.format("%Y-%m-%d %H:%M").to_string(),
            "2025-08-01 15:23"
        );
        assert_eq!(
            constraint.end_time.format("%Y-%m-%d %H:%M").to_string(),
            "2025-08-01 15:33"
        );
    }

    #[test]
    fn test_datetime_constraint_from_start_and_end() {
        let datetime_request = DateTimeRequest {
            start: Some("2025-08-01T15:23:00+01:00".to_string()),
            end: Some("2025-08-01T15:33:00+01:00".to_string()),
            duration: None,
        };

        let constraint = DateTimeConstraint::try_from(datetime_request).unwrap();

        assert_eq!(
            constraint.start_time.format("%Y-%m-%d %H:%M").to_string(),
            "2025-08-01 15:23"
        );
        assert_eq!(
            constraint.end_time.format("%Y-%m-%d %H:%M").to_string(),
            "2025-08-01 15:33"
        );
    }

    #[test]
    fn test_datetime_constraint_from_end_and_duration() {
        let datetime_request = DateTimeRequest {
            start: None,
            end: Some("2025-08-01T15:33:00+01:00".to_string()),
            duration: Some(10), // 10 minutes
        };

        let constraint = DateTimeConstraint::try_from(datetime_request).unwrap();

        assert_eq!(
            constraint.start_time.format("%Y-%m-%d %H:%M").to_string(),
            "2025-08-01 15:23"
        );
        assert_eq!(
            constraint.end_time.format("%Y-%m-%d %H:%M").to_string(),
            "2025-08-01 15:33"
        );
    }

    #[test]
    fn test_datetime_constraint_invalid_combinations() {
        // Test with no fields
        let result = DateTimeConstraint::try_from(DateTimeRequest {
            start: None,
            end: None,
            duration: None,
        });
        assert!(result.is_err());

        // Test with all fields
        let result = DateTimeConstraint::try_from(DateTimeRequest {
            start: Some("2025-08-01T15:23:00+01:00".to_string()),
            end: Some("2025-08-01T15:33:00+01:00".to_string()),
            duration: Some(10),
        });
        assert!(result.is_err());

        // Test with only one field
        let result = DateTimeConstraint::try_from(DateTimeRequest {
            start: Some("2025-08-01T15:23:00+01:00".to_string()),
            end: None,
            duration: None,
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_request_image_path() {
        // Test request with image-path
        let json = r#"{
            "processing-id": "001",
            "image-path": "/path/to/image.jpg",
            "analysis-request": {
                "content": "test content"
            }
        }"#;

        let request: ValidationRequest = serde_json::from_str(json).unwrap();
        assert_eq!(
            request.get_image_path(),
            Some("/path/to/image.jpg".to_string())
        );
    }

    #[test]
    fn test_validation_request_image_null() {
        // Test request with image: null
        let json = r#"{
            "processing-id": "001", 
            "image": null,
            "analysis-request": {
                "content": "test content"
            }
        }"#;

        let request: ValidationRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.get_image_path(), None);
    }

    #[test]
    fn test_validation_response_accepted() {
        let response = ValidationResponse {
            processing_id: "001".to_string(),
            results: ValidationResults {
                resolution: Resolution::Accepted,
                reasons: None,
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"resolution\":\"accepted\""));
        assert!(!json.contains("resons"));
    }

    #[test]
    fn test_validation_response_rejected() {
        let response = ValidationResponse {
            processing_id: "001".to_string(),
            results: ValidationResults {
                resolution: Resolution::Rejected,
                reasons: Some(vec!["cannot locate image".to_string()]),
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"resolution\":\"rejected\""));
        assert!(json.contains("\"resons\"")); // Note the typo
        assert!(json.contains("cannot locate image"));
    }
}
