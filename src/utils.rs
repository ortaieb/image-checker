use crate::models::{DateTimeConstraint, LocationConstraint};
use chrono::{DateTime, FixedOffset};

const EARTH_RADIUS_KM: f64 = 6371.0;
const EARTH_RADIUS_M: f64 = EARTH_RADIUS_KM * 1000.0;

/// Calculate the distance between two GPS coordinates using the Haversine formula
/// Returns distance in meters
pub fn haversine_distance(coord1: (f64, f64), coord2: (f64, f64)) -> f64 {
    let (lat1, lon1) = coord1;
    let (lat2, lon2) = coord2;

    // Convert degrees to radians
    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let delta_lat = (lat2 - lat1).to_radians();
    let delta_lon = (lon2 - lon1).to_radians();

    // Haversine formula
    let a = (delta_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);

    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    // Distance in meters
    EARTH_RADIUS_M * c
}

/// Validate if GPS coordinates are within the specified location constraint
pub fn validate_location(
    actual_coords: (f64, f64),
    constraint: &LocationConstraint,
) -> Result<bool, String> {
    // Validate coordinate ranges
    let (lat, lon) = actual_coords;
    if !(-90.0..=90.0).contains(&lat) {
        return Err(format!(
            "Invalid latitude: {lat} (must be between -90 and 90)"
        ));
    }
    if !(-180.0..=180.0).contains(&lon) {
        return Err(format!(
            "Invalid longitude: {lon} (must be between -180 and 180)"
        ));
    }

    let expected_coords = (constraint.latitude, constraint.longitude);
    let distance = haversine_distance(actual_coords, expected_coords);

    Ok(distance <= constraint.max_distance_meters)
}

/// Validate if a timestamp is within the specified datetime constraint
pub fn validate_datetime(
    actual_time: &DateTime<FixedOffset>,
    constraint: &DateTimeConstraint,
) -> Result<bool, String> {
    // Check if actual time is within the allowed time range
    let is_valid = actual_time >= &constraint.start_time && actual_time <= &constraint.end_time;

    Ok(is_valid)
}

/// Convert coordinates to a human-readable string for debugging
pub fn coords_to_string(coords: (f64, f64)) -> String {
    let (lat, lon) = coords;
    let lat_dir = if lat >= 0.0 { "N" } else { "S" };
    let lon_dir = if lon >= 0.0 { "E" } else { "W" };

    format!("{:.6}°{}, {:.6}°{}", lat.abs(), lat_dir, lon.abs(), lon_dir)
}

/// Format distance in a human-readable way
pub fn format_distance(distance_meters: f64) -> String {
    if distance_meters < 1000.0 {
        format!("{distance_meters:.1}m")
    } else {
        format!("{:.2}km", distance_meters / 1000.0)
    }
}

/// Validate that coordinates are reasonable (not obviously invalid)
pub fn validate_coordinates(coords: (f64, f64)) -> Result<(), String> {
    let (lat, lon) = coords;

    if !(-90.0..=90.0).contains(&lat) {
        return Err(format!("Latitude {lat} is out of valid range (-90 to 90)"));
    }

    if !(-180.0..=180.0).contains(&lon) {
        return Err(format!(
            "Longitude {lon} is out of valid range (-180 to 180)"
        ));
    }

    // Check for obviously invalid coordinates (0,0 might be suspicious in many contexts)
    if lat == 0.0 && lon == 0.0 {
        return Err("Coordinates (0,0) may indicate missing or invalid GPS data".into());
    }

    Ok(())
}

/// Calculate the precision/uncertainty of GPS coordinates based on EXIF limitations
/// Returns approximate uncertainty in meters
pub fn estimate_gps_precision(coords: (f64, f64)) -> f64 {
    // EXIF GPS coordinates typically have limited precision
    // Each degree is approximately 111km, so:
    // - 6 decimal places: ~0.1m precision
    // - 5 decimal places: ~1m precision
    // - 4 decimal places: ~10m precision
    // - 3 decimal places: ~100m precision

    let (lat, lon) = coords;

    // Count decimal places in the coordinates
    let lat_str = format!("{lat:.10}");
    let lon_str = format!("{lon:.10}");

    let lat_decimals = count_decimal_places(&lat_str);
    let lon_decimals = count_decimal_places(&lon_str);

    // Use the coordinate with fewer decimal places (less precise)
    let min_decimals = lat_decimals.min(lon_decimals);

    match min_decimals {
        0..=2 => 1000.0, // ~1km uncertainty
        3 => 100.0,      // ~100m uncertainty
        4 => 10.0,       // ~10m uncertainty
        5 => 1.0,        // ~1m uncertainty
        _ => 0.1,        // ~0.1m uncertainty
    }
}

fn count_decimal_places(num_str: &str) -> usize {
    if let Some(decimal_pos) = num_str.find('.') {
        let decimal_part = &num_str[decimal_pos + 1..];
        // Count non-zero digits from the right
        decimal_part.trim_end_matches('0').len()
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, TimeZone};

    #[test]
    fn test_haversine_distance_known_coordinates() {
        // Test with known coordinates from examples
        let coord1 = (51.491079, -0.269590); // Example image location
        let coord2 = (51.492191, -0.266108); // Expected location

        let distance = haversine_distance(coord1, coord2);

        // Distance should be less than 300m based on the coordinates
        assert!(distance < 300.0);
        assert!(distance > 200.0); // Should be around 250m
    }

    #[test]
    fn test_haversine_distance_same_point() {
        let coord = (51.5074, -0.1278); // London
        let distance = haversine_distance(coord, coord);

        assert!((distance - 0.0).abs() < 0.001); // Should be essentially 0
    }

    #[test]
    fn test_validate_location_within_range() {
        let actual = (51.491079, -0.269590);
        let constraint = LocationConstraint {
            max_distance_meters: 300.0,
            latitude: 51.492191,
            longitude: -0.266108,
        };

        let result = validate_location(actual, &constraint).unwrap();
        assert!(result); // Should be within 300m
    }

    #[test]
    fn test_validate_location_outside_range() {
        let actual = (51.491079, -0.269590);
        let constraint = LocationConstraint {
            max_distance_meters: 100.0, // Very strict limit
            latitude: 51.492191,
            longitude: -0.266108,
        };

        let result = validate_location(actual, &constraint).unwrap();
        assert!(!result); // Should be outside 100m range
    }

    #[test]
    fn test_validate_datetime_within_window() {
        let start_time = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2025, 8, 1, 15, 23, 0)
            .unwrap();
        let end_time = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2025, 8, 1, 15, 33, 0)
            .unwrap(); // 10 minutes later
        let actual_time = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2025, 8, 1, 15, 25, 0)
            .unwrap(); // 2 minutes after start

        let constraint = DateTimeConstraint {
            start_time,
            end_time,
        };

        let result = validate_datetime(&actual_time, &constraint).unwrap();
        assert!(result); // Should be within time window
    }

    #[test]
    fn test_validate_datetime_outside_window() {
        let start_time = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2025, 8, 1, 15, 23, 0)
            .unwrap();
        let end_time = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2025, 8, 1, 15, 33, 0)
            .unwrap(); // 10 minutes later
        let actual_time = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2025, 8, 1, 15, 40, 0)
            .unwrap(); // 17 minutes after start, 7 minutes after end

        let constraint = DateTimeConstraint {
            start_time,
            end_time,
        };

        let result = validate_datetime(&actual_time, &constraint).unwrap();
        assert!(!result); // Should be outside time window
    }

    #[test]
    fn test_validate_coordinates() {
        // Valid coordinates
        assert!(validate_coordinates((51.5074, -0.1278)).is_ok());

        // Invalid latitude
        assert!(validate_coordinates((91.0, 0.0)).is_err());
        assert!(validate_coordinates((-91.0, 0.0)).is_err());

        // Invalid longitude
        assert!(validate_coordinates((0.0, 181.0)).is_err());
        assert!(validate_coordinates((0.0, -181.0)).is_err());

        // Suspicious (0,0) coordinates
        assert!(validate_coordinates((0.0, 0.0)).is_err());
    }

    #[test]
    fn test_coords_to_string() {
        let coords = (51.491079, -0.269590);
        let formatted = coords_to_string(coords);
        assert!(formatted.contains("51.491079°N"));
        assert!(formatted.contains("0.269590°W"));
    }

    #[test]
    fn test_format_distance() {
        assert_eq!(format_distance(250.5), "250.5m");
        assert_eq!(format_distance(1500.0), "1.50km");
        assert_eq!(format_distance(999.9), "999.9m");
    }

    #[test]
    fn test_estimate_gps_precision() {
        // High precision coordinates
        let high_precision = (51.491079, -0.269590);
        let precision = estimate_gps_precision(high_precision);
        assert!(precision <= 1.0); // Should be very precise

        // Low precision coordinates
        let low_precision = (51.49, -0.27);
        let precision_low = estimate_gps_precision(low_precision);
        assert!(precision_low >= 100.0); // Should be less precise
    }
}
