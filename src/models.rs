use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

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
pub struct AnalysisRequest {
    #[serde(rename = "image-path")]
    pub image_path: Option<String>,

    pub content: String,

    pub location: Option<String>,

    pub datetime: Option<String>,
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

impl FromStr for LocationConstraint {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Parse "not more than 100m from coordinates (51.492191, -0.266108)"
        let parts: Vec<&str> = s.split_whitespace().collect();

        if parts.len() < 8 {
            return Err(format!("Invalid location format: {s}"));
        }

        // Extract distance (e.g., "100m")
        let distance_str = parts[3];
        let distance_meters = if let Some(stripped) = distance_str.strip_suffix("m") {
            stripped
                .parse::<f64>()
                .map_err(|_| format!("Invalid distance: {distance_str}"))?
        } else {
            return Err(format!("Distance must end with 'm': {distance_str}"));
        };

        // Extract coordinates from "(lat, lon)"
        let coords_part = parts[6..].join(" ");
        let coords_part = coords_part.trim_start_matches('(').trim_end_matches(')');
        let coord_parts: Vec<&str> = coords_part.split(',').collect();

        if coord_parts.len() != 2 {
            return Err(format!("Invalid coordinates format: {coords_part}"));
        }

        let latitude = coord_parts[0]
            .trim()
            .parse::<f64>()
            .map_err(|_| format!("Invalid latitude: {}", coord_parts[0]))?;
        let longitude = coord_parts[1]
            .trim()
            .parse::<f64>()
            .map_err(|_| format!("Invalid longitude: {}", coord_parts[1]))?;

        Ok(LocationConstraint {
            max_distance_meters: distance_meters,
            latitude,
            longitude,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DateTimeConstraint {
    pub max_minutes_after: u64,
    pub reference_time: DateTime<FixedOffset>,
}

impl FromStr for DateTimeConstraint {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Parse "image was taken not more than 10 minutes after 2025-08-01T15:23:00Z+1"
        let parts: Vec<&str> = s.split_whitespace().collect();

        if parts.len() < 10 {
            return Err(format!("Invalid datetime format: {s}"));
        }

        // Extract minutes - should be the number before "minutes"
        let minutes_str = parts[6];
        let max_minutes_after = minutes_str
            .parse::<u64>()
            .map_err(|_| format!("Invalid minutes: {minutes_str}"))?;

        // Extract datetime - last part should be the timestamp
        let datetime_str = parts[parts.len() - 1];

        // Handle different datetime formats
        let reference_time = if datetime_str.contains('+') || datetime_str.contains('Z') {
            DateTime::parse_from_rfc3339(datetime_str)
                .or_else(|_| {
                    // Try parsing custom format like "2025-08-01T15:23:00Z+1"
                    if let Some(base_dt) = datetime_str.strip_suffix("Z+1") {
                        let dt_with_tz = format!("{}+01:00", base_dt.trim_end_matches('Z'));
                        DateTime::parse_from_rfc3339(&dt_with_tz)
                    } else {
                        DateTime::parse_from_rfc3339(datetime_str)
                    }
                })
                .map_err(|_| format!("Invalid datetime: {datetime_str}"))?
        } else {
            return Err(format!("Datetime must include timezone: {datetime_str}"));
        };

        Ok(DateTimeConstraint {
            max_minutes_after,
            reference_time,
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
        let location_constraint = if let Some(location) = request.location {
            Some(LocationConstraint::from_str(&location)?)
        } else {
            None
        };

        let datetime_constraint = if let Some(datetime) = request.datetime {
            Some(DateTimeConstraint::from_str(&datetime)?)
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
    fn test_location_constraint_parsing() {
        let location_str = "not more than 100m from coordinates (51.492191, -0.266108)";
        let constraint = LocationConstraint::from_str(location_str).unwrap();

        assert_eq!(constraint.max_distance_meters, 100.0);
        assert!((constraint.latitude - 51.492191).abs() < 0.000001);
        assert!((constraint.longitude + 0.266108).abs() < 0.000001);
    }

    #[test]
    fn test_datetime_constraint_parsing() {
        let datetime_str = "image was taken not more than 10 minutes after 2025-08-01T15:23:00Z+1";
        let constraint = DateTimeConstraint::from_str(datetime_str).unwrap();

        assert_eq!(constraint.max_minutes_after, 10);
        // The reference time should be parsed correctly
        assert_eq!(
            constraint.reference_time.format("%Y-%m-%d").to_string(),
            "2025-08-01"
        );
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
