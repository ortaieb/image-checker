use image_checker::models::*;
use image_checker::utils::*;

use chrono::{FixedOffset, TimeZone};
use std::str::FromStr;

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
fn test_location_constraint_parsing() {
    let location_str = "not more than 100m from coordinates (51.492191, -0.266108)";
    let constraint = LocationConstraint::from_str(location_str).unwrap();

    assert_eq!(constraint.max_distance_meters, 100.0);
    assert!((constraint.latitude - 51.492191).abs() < 0.000001);
    assert!((constraint.longitude + 0.266108).abs() < 0.000001);
}

#[test]
fn test_location_constraint_parsing_variations() {
    // Test different distance units and formats
    let variations = vec![
        "not more than 50m from coordinates (0.0, 0.0)",
        "not more than 1000m from coordinates (-90.0, -180.0)",
        "not more than 25m from coordinates (90.0, 180.0)",
    ];

    for location_str in variations {
        let result = LocationConstraint::from_str(location_str);
        assert!(result.is_ok(), "Failed to parse: {}", location_str);
    }
}

#[test]
fn test_location_constraint_parsing_invalid() {
    let invalid_strings = vec![
        "invalid format",
        "not more than meters from coordinates",
        "not more than 100 from coordinates (51.0, -0.1)",
        "not more than 100m from coordinates invalid",
        "not more than 100m from coordinates (invalid, coords)",
    ];

    for invalid_str in invalid_strings {
        let result = LocationConstraint::from_str(invalid_str);
        assert!(
            result.is_err(),
            "Should have failed to parse: {}",
            invalid_str
        );
    }
}

#[test]
fn test_datetime_constraint_parsing() {
    let datetime_str = "image was taken not more than 10 minutes after 2025-08-01T15:23:00Z+1";
    let constraint = DateTimeConstraint::from_str(datetime_str).unwrap();

    assert_eq!(constraint.max_minutes_after, 10);
    assert_eq!(
        constraint.reference_time.format("%Y-%m-%d").to_string(),
        "2025-08-01"
    );
}

#[test]
fn test_datetime_constraint_parsing_variations() {
    let variations = vec![
        "image was taken not more than 5 minutes after 2025-08-01T15:23:00Z+1",
        "image was taken not more than 60 minutes after 2025-12-31T23:59:59Z+1",
        "image was taken not more than 1 minutes after 2025-01-01T00:00:00Z+1",
    ];

    for datetime_str in variations {
        let result = DateTimeConstraint::from_str(datetime_str);
        assert!(result.is_ok(), "Failed to parse: {}", datetime_str);
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
    let reference_time = FixedOffset::east_opt(3600)
        .unwrap() // +1 hour
        .with_ymd_and_hms(2025, 8, 1, 15, 23, 0)
        .unwrap();
    let actual_time = FixedOffset::east_opt(3600)
        .unwrap()
        .with_ymd_and_hms(2025, 8, 1, 15, 25, 0)
        .unwrap(); // 2 minutes later

    let constraint = DateTimeConstraint {
        max_minutes_after: 10,
        reference_time,
    };

    let result = validate_datetime(&actual_time, &constraint).unwrap();
    assert!(result);
}

#[test]
fn test_validate_datetime_outside_window() {
    let reference_time = FixedOffset::east_opt(3600)
        .unwrap()
        .with_ymd_and_hms(2025, 8, 1, 15, 23, 0)
        .unwrap();
    let actual_time = FixedOffset::east_opt(3600)
        .unwrap()
        .with_ymd_and_hms(2025, 8, 1, 15, 40, 0)
        .unwrap(); // 17 minutes later

    let constraint = DateTimeConstraint {
        max_minutes_after: 10,
        reference_time,
    };

    let result = validate_datetime(&actual_time, &constraint).unwrap();
    assert!(!result);
}

#[test]
fn test_validate_datetime_before_reference() {
    let reference_time = FixedOffset::east_opt(3600)
        .unwrap()
        .with_ymd_and_hms(2025, 8, 1, 15, 23, 0)
        .unwrap();
    let actual_time = FixedOffset::east_opt(3600)
        .unwrap()
        .with_ymd_and_hms(2025, 8, 1, 15, 20, 0)
        .unwrap(); // 3 minutes before

    let constraint = DateTimeConstraint {
        max_minutes_after: 10,
        reference_time,
    };

    let result = validate_datetime(&actual_time, &constraint).unwrap();
    assert!(!result); // Should fail because it's before reference time
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
        location: Some("not more than 100m from coordinates (51.492191, -0.266108)".to_string()),
        datetime: Some(
            "image was taken not more than 10 minutes after 2025-08-01T15:23:00Z+1".to_string(),
        ),
    };

    let context = ValidationContext::try_from(analysis_request).unwrap();

    assert_eq!(context.content_check, "Three birds on a wire");
    assert!(context.location_constraint.is_some());
    assert!(context.datetime_constraint.is_some());

    let location = context.location_constraint.unwrap();
    assert_eq!(location.max_distance_meters, 100.0);

    let datetime = context.datetime_constraint.unwrap();
    assert_eq!(datetime.max_minutes_after, 10);
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
