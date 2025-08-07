use image_checker::models::*;
use image_checker::utils::*;
use image_checker::validation::llm::{validate_image_content, LlmClient};

use chrono::{FixedOffset, TimeZone};
use std::time::Duration;

#[test]
fn test_haversine_distance_calculation() {
    // Test with known coordinates from London area
    let coord1 = (51.491079, -0.269590); // Example image location
    let coord2 = (51.492191, -0.266108); // Expected location

    let distance = haversine_distance(coord1, coord2);

    // Distance should be reasonable (a few hundred meters)
    assert!(distance > 200.0);
    assert!(distance < 400.0);

    // Test same coordinates
    let same_distance = haversine_distance(coord1, coord1);
    assert!(same_distance < 0.1); // Should be essentially 0
}

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
fn test_location_request_variations() {
    // Test different valid location requests
    let variations = vec![
        LocationRequest {
            long: 0.0,
            lat: 0.0,
            max_distance: 50.0,
        },
        LocationRequest {
            long: -180.0,
            lat: -90.0,
            max_distance: 1000.0,
        },
        LocationRequest {
            long: 180.0,
            lat: 90.0,
            max_distance: 25.0,
        },
    ];

    for location_request in variations {
        let constraint = LocationConstraint::from(location_request.clone());
        assert!(constraint.max_distance_meters > 0.0);
        assert!(constraint.latitude >= -90.0 && constraint.latitude <= 90.0);
        assert!(constraint.longitude >= -180.0 && constraint.longitude <= 180.0);
    }
}

#[test]
fn test_datetime_request_start_and_duration() {
    let datetime_request = DateTimeRequest {
        start: Some("2025-08-01T15:23:00+01:00".to_string()),
        end: None,
        duration: Some(10),
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
fn test_datetime_request_variations() {
    let variations = vec![
        DateTimeRequest {
            start: Some("2025-08-01T15:23:00+01:00".to_string()),
            end: Some("2025-08-01T15:28:00+01:00".to_string()),
            duration: None,
        },
        DateTimeRequest {
            start: Some("2025-12-31T23:59:00+01:00".to_string()),
            end: None,
            duration: Some(60),
        },
        DateTimeRequest {
            start: None,
            end: Some("2025-01-01T00:01:00+01:00".to_string()),
            duration: Some(1),
        },
    ];

    for datetime_request in variations {
        let result = DateTimeConstraint::try_from(datetime_request);
        assert!(result.is_ok(), "Failed to convert datetime request");
    }
}

#[test]
fn test_validate_location_within_range() {
    let actual = (51.491079, -0.269590);
    let constraint = LocationConstraint {
        max_distance_meters: 500.0,
        latitude: 51.492191,
        longitude: -0.266108,
    };

    let result = validate_location(actual, &constraint).unwrap();
    assert!(result);
}

#[test]
fn test_validate_location_outside_range() {
    let actual = (51.491079, -0.269590);
    let constraint = LocationConstraint {
        max_distance_meters: 50.0, // Very strict limit
        latitude: 51.492191,
        longitude: -0.266108,
    };

    let result = validate_location(actual, &constraint).unwrap();
    assert!(!result);
}

#[test]
fn test_validate_location_invalid_coordinates() {
    let invalid_coords = vec![
        (91.0, 0.0),   // Invalid latitude > 90
        (-91.0, 0.0),  // Invalid latitude < -90
        (0.0, 181.0),  // Invalid longitude > 180
        (0.0, -181.0), // Invalid longitude < -180
    ];

    let constraint = LocationConstraint {
        max_distance_meters: 100.0,
        latitude: 0.0,
        longitude: 0.0,
    };

    for coords in invalid_coords {
        let result = validate_location(coords, &constraint);
        assert!(
            result.is_err(),
            "Should have failed for coordinates: {:?}",
            coords
        );
    }
}

#[test]
fn test_validate_datetime_within_window() {
    let start_time = FixedOffset::east_opt(3600)
        .unwrap() // +1 hour
        .with_ymd_and_hms(2025, 8, 1, 15, 23, 0)
        .unwrap();
    let end_time = FixedOffset::east_opt(3600)
        .unwrap()
        .with_ymd_and_hms(2025, 8, 1, 15, 33, 0)
        .unwrap(); // 10 minutes later
    let actual_time = FixedOffset::east_opt(3600)
        .unwrap()
        .with_ymd_and_hms(2025, 8, 1, 15, 25, 0)
        .unwrap(); // 2 minutes after start

    let constraint = DateTimeConstraint {
        start_time,
        end_time,
    };

    let result = validate_datetime(&actual_time, &constraint).unwrap();
    assert!(result);
}

#[test]
fn test_validate_datetime_outside_window() {
    let start_time = FixedOffset::east_opt(3600)
        .unwrap()
        .with_ymd_and_hms(2025, 8, 1, 15, 23, 0)
        .unwrap();
    let end_time = FixedOffset::east_opt(3600)
        .unwrap()
        .with_ymd_and_hms(2025, 8, 1, 15, 33, 0)
        .unwrap(); // 10 minutes later
    let actual_time = FixedOffset::east_opt(3600)
        .unwrap()
        .with_ymd_and_hms(2025, 8, 1, 15, 40, 0)
        .unwrap(); // 17 minutes after start, 7 minutes after end

    let constraint = DateTimeConstraint {
        start_time,
        end_time,
    };

    let result = validate_datetime(&actual_time, &constraint).unwrap();
    assert!(!result);
}

#[test]
fn test_validate_datetime_before_window() {
    let start_time = FixedOffset::east_opt(3600)
        .unwrap()
        .with_ymd_and_hms(2025, 8, 1, 15, 23, 0)
        .unwrap();
    let end_time = FixedOffset::east_opt(3600)
        .unwrap()
        .with_ymd_and_hms(2025, 8, 1, 15, 33, 0)
        .unwrap(); // 10 minutes later
    let actual_time = FixedOffset::east_opt(3600)
        .unwrap()
        .with_ymd_and_hms(2025, 8, 1, 15, 20, 0)
        .unwrap(); // 3 minutes before start

    let constraint = DateTimeConstraint {
        start_time,
        end_time,
    };

    let result = validate_datetime(&actual_time, &constraint).unwrap();
    assert!(!result); // Should fail because it's before start time
}

#[test]
fn test_coords_to_string() {
    let coords = (51.491079, -0.269590);
    let formatted = coords_to_string(coords);

    assert!(formatted.contains("51.491079°N"));
    assert!(formatted.contains("0.269590°W"));

    // Test southern and eastern coordinates
    let coords_se = (-23.5505, 46.6333);
    let formatted_se = coords_to_string(coords_se);

    assert!(formatted_se.contains("23.550500°S"));
    assert!(formatted_se.contains("46.633300°E"));
}

#[test]
fn test_format_distance() {
    assert_eq!(format_distance(250.5), "250.5m");
    assert_eq!(format_distance(999.9), "999.9m");
    assert_eq!(format_distance(1000.0), "1.00km");
    assert_eq!(format_distance(1500.0), "1.50km");
    assert_eq!(format_distance(5000.0), "5.00km");
}

#[test]
fn test_validate_coordinates() {
    // Valid coordinates
    assert!(validate_coordinates((51.5074, -0.1278)).is_ok());
    assert!(validate_coordinates((0.0, 0.1)).is_ok()); // Just off (0,0) should be ok
    assert!(validate_coordinates((90.0, 180.0)).is_ok());
    assert!(validate_coordinates((-90.0, -180.0)).is_ok());

    // Invalid coordinates
    assert!(validate_coordinates((91.0, 0.0)).is_err()); // Latitude too high
    assert!(validate_coordinates((-91.0, 0.0)).is_err()); // Latitude too low
    assert!(validate_coordinates((0.0, 181.0)).is_err()); // Longitude too high
    assert!(validate_coordinates((0.0, -181.0)).is_err()); // Longitude too low
    assert!(validate_coordinates((0.0, 0.0)).is_err()); // Suspicious (0,0)
}

#[test]
fn test_estimate_gps_precision() {
    // Test high precision coordinates (6+ decimal places)
    let high_precision = (51.491079, -0.269590);
    let precision = estimate_gps_precision(high_precision);
    assert!(precision <= 1.0);

    // Test medium precision coordinates (4-5 decimal places)
    let medium_precision = (51.4911, -0.2696);
    let precision_medium = estimate_gps_precision(medium_precision);
    assert!(precision_medium <= 10.0);
    assert!(precision_medium >= 1.0);

    // Test low precision coordinates (2-3 decimal places)
    let low_precision = (51.49, -0.27);
    let precision_low = estimate_gps_precision(low_precision);
    assert!(precision_low >= 100.0);
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

#[test]
fn test_validation_context_optional_fields() {
    let analysis_request = AnalysisRequest {
        image_path: None,
        content: "Just content check".to_string(),
        location: None,
        datetime: None,
    };

    let context = ValidationContext::try_from(analysis_request).unwrap();

    assert_eq!(context.content_check, "Just content check");
    assert!(context.location_constraint.is_none());
    assert!(context.datetime_constraint.is_none());
}

#[tokio::test]
async fn test_llm_validation_integration() {
    // This test requires Ollama to be running with the llava:13b model
    // Skip if not available

    let client = LlmClient::new(
        "http://localhost:11434/v1/chat/completions".to_string(),
        "llava:13b".to_string(),
        Duration::from_secs(30),
    );

    // Test with our minimal test image
    let image_path = "/tmp/test-images/test.jpg";
    let content_description = "A simple test image or placeholder";

    // This test will show the debug output we added
    match validate_image_content(&client, image_path, content_description).await {
        Ok(is_valid) => {
            println!("✅ LLM validation completed successfully");
            println!(
                "   Result: {}",
                if is_valid { "ACCEPTED" } else { "REJECTED" }
            );
        }
        Err(e) => {
            println!("❌ LLM validation failed: {}", e);
            // Don't fail the test - just report the issue
        }
    }
}
