use chrono::{DateTime, FixedOffset, NaiveDateTime, Utc};
use exif::{In, Reader, Tag, Value};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExifError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("EXIF parsing error: {0}")]
    Parsing(#[from] exif::Error),
    #[error("Missing GPS data in EXIF")]
    MissingGpsData,
    #[error("Missing timestamp data in EXIF")]
    MissingTimestamp,
    #[error("Invalid GPS coordinate format: {0}")]
    InvalidGpsFormat(String),
    #[error("Invalid timestamp format: {0}")]
    InvalidTimestamp(String),
}

#[derive(Debug, Clone)]
pub struct ExifData {
    pub gps_coordinates: Option<(f64, f64)>, // (latitude, longitude)
    pub timestamp: Option<DateTime<FixedOffset>>,
    pub datetime_original: Option<DateTime<FixedOffset>>,
}

pub fn extract_exif_metadata<P: AsRef<Path>>(image_path: P) -> Result<ExifData, ExifError> {
    let file = File::open(&image_path)?;
    let mut reader = BufReader::new(&file);

    let exif_reader = Reader::new();
    let exif = exif_reader.read_from_container(&mut reader)?;

    let gps_coordinates = extract_gps_coordinates(&exif)?;
    let timestamp = extract_datetime(&exif, Tag::DateTime)?;
    let datetime_original = extract_datetime(&exif, Tag::DateTimeOriginal)?;

    Ok(ExifData {
        gps_coordinates,
        timestamp,
        datetime_original,
    })
}

fn extract_gps_coordinates(exif: &exif::Exif) -> Result<Option<(f64, f64)>, ExifError> {
    // Try to extract GPS latitude
    let lat_field = exif.get_field(Tag::GPSLatitude, In::PRIMARY);
    let lat_ref_field = exif.get_field(Tag::GPSLatitudeRef, In::PRIMARY);

    // Try to extract GPS longitude
    let lon_field = exif.get_field(Tag::GPSLongitude, In::PRIMARY);
    let lon_ref_field = exif.get_field(Tag::GPSLongitudeRef, In::PRIMARY);

    // If any GPS fields are missing, return None (not an error)
    if lat_field.is_none()
        || lat_ref_field.is_none()
        || lon_field.is_none()
        || lon_ref_field.is_none()
    {
        return Ok(None);
    }

    let lat_field = lat_field.unwrap();
    let lat_ref_field = lat_ref_field.unwrap();
    let lon_field = lon_field.unwrap();
    let lon_ref_field = lon_ref_field.unwrap();

    // Extract latitude DMS values
    let lat_dms = extract_gps_dms(&lat_field.value)?;
    let lat_ref = extract_gps_ref(&lat_ref_field.value)?;

    // Extract longitude DMS values
    let lon_dms = extract_gps_dms(&lon_field.value)?;
    let lon_ref = extract_gps_ref(&lon_ref_field.value)?;

    // Convert DMS to decimal degrees
    let latitude = dms_to_decimal(lat_dms) * if lat_ref == "S" { -1.0 } else { 1.0 };
    let longitude = dms_to_decimal(lon_dms) * if lon_ref == "W" { -1.0 } else { 1.0 };

    Ok(Some((latitude, longitude)))
}

fn extract_gps_dms(value: &Value) -> Result<(f64, f64, f64), ExifError> {
    match value {
        Value::Rational(rationals) => {
            if rationals.len() != 3 {
                return Err(ExifError::InvalidGpsFormat(format!(
                    "Expected 3 rational values for DMS, got {}",
                    rationals.len()
                )));
            }

            let degrees = rationals[0].to_f64();
            let minutes = rationals[1].to_f64();
            let seconds = rationals[2].to_f64();

            Ok((degrees, minutes, seconds))
        }
        _ => Err(ExifError::InvalidGpsFormat(
            "GPS coordinates must be stored as rational values".into(),
        )),
    }
}

fn extract_gps_ref(value: &Value) -> Result<String, ExifError> {
    match value {
        Value::Ascii(ascii_values) => {
            if ascii_values.is_empty() {
                return Err(ExifError::InvalidGpsFormat("Empty GPS reference".into()));
            }

            let ref_str = String::from_utf8_lossy(&ascii_values[0]);
            Ok(ref_str.trim_end_matches('\0').to_string())
        }
        _ => Err(ExifError::InvalidGpsFormat(
            "GPS reference must be ASCII value".into(),
        )),
    }
}

fn dms_to_decimal(dms: (f64, f64, f64)) -> f64 {
    let (degrees, minutes, seconds) = dms;
    degrees + minutes / 60.0 + seconds / 3600.0
}

fn extract_datetime(
    exif: &exif::Exif,
    tag: Tag,
) -> Result<Option<DateTime<FixedOffset>>, ExifError> {
    let field = exif.get_field(tag, In::PRIMARY);

    if field.is_none() {
        return Ok(None);
    }

    let field = field.unwrap();

    match &field.value {
        Value::Ascii(ascii_values) => {
            if ascii_values.is_empty() {
                return Ok(None);
            }

            let datetime_str = String::from_utf8_lossy(&ascii_values[0]);
            let datetime_str = datetime_str.trim_end_matches('\0');

            // EXIF datetime format is "YYYY:MM:DD HH:MM:SS"
            let naive_dt = NaiveDateTime::parse_from_str(datetime_str, "%Y:%m:%d %H:%M:%S")
                .map_err(|_| ExifError::InvalidTimestamp(datetime_str.to_string()))?;

            // Convert to UTC (EXIF timestamps are typically in local time, but we assume UTC for consistency)
            let utc_dt = DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc);
            let fixed_offset_dt = utc_dt.with_timezone(&FixedOffset::east_opt(0).unwrap());

            Ok(Some(fixed_offset_dt))
        }
        _ => Err(ExifError::InvalidTimestamp(
            "DateTime must be stored as ASCII value".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dms_to_decimal_conversion() {
        // Test known conversion: 51°29'27.48" = 51.4909667
        let dms = (51.0, 29.0, 27.48);
        let decimal = dms_to_decimal(dms);
        assert!((decimal - 51.4909667).abs() < 0.000001);
    }

    #[test]
    fn test_dms_to_decimal_negative() {
        // Test with Southern/Western coordinates
        let dms = (0.0, 16.0, 9.324); // 0°16'9.324" = 0.2692567
        let decimal = dms_to_decimal(dms) * -1.0; // Western longitude
        assert!((decimal + 0.2692567).abs() < 0.000001);
    }

    #[test]
    fn test_extract_exif_metadata_missing_file() {
        let result = extract_exif_metadata("nonexistent.jpg");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ExifError::Io(_)));
    }

    // Note: Integration tests with real images should be in the tests/ directory
    // since we need actual image files with EXIF data for testing
}
