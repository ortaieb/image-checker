use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Unsupported URI scheme: {0}")]
    UnsupportedScheme(String),
    #[error("Invalid URI format: {0}")]
    InvalidUri(String),
    #[error("Path does not exist: {0}")]
    PathNotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq)]
pub enum StorageUri {
    LocalPath(String),
    FileUri(String),
    // Future: S3Uri(String), GcsUri(String), etc.
}

impl StorageUri {
    pub fn parse(uri: &str) -> Result<Self, StorageError> {
        if uri.starts_with("file://") {
            // Handle file:// URI
            let path = uri
                .strip_prefix("file://")
                .ok_or_else(|| StorageError::InvalidUri(uri.to_string()))?;

            // Convert to absolute path if needed
            let absolute_path = if path.starts_with('/') {
                path.to_string()
            } else {
                return Err(StorageError::InvalidUri(format!(
                    "file:// URI must use absolute paths: {uri}"
                )));
            };

            Ok(StorageUri::FileUri(absolute_path))
        } else if uri.contains("://") {
            // Other URI schemes
            let scheme = uri
                .split("://")
                .next()
                .ok_or_else(|| StorageError::InvalidUri(uri.to_string()))?;
            Err(StorageError::UnsupportedScheme(scheme.to_string()))
        } else {
            // Treat as local path (backward compatibility)
            Ok(StorageUri::LocalPath(uri.to_string()))
        }
    }

    pub fn to_local_path(&self) -> &str {
        match self {
            StorageUri::LocalPath(path) => path,
            StorageUri::FileUri(path) => path,
        }
    }

    pub fn exists(&self) -> bool {
        match self {
            StorageUri::LocalPath(path) | StorageUri::FileUri(path) => Path::new(path).exists(),
        }
    }

    pub fn resolve_relative_path(&self, relative_path: &str) -> String {
        let base_path = self.to_local_path();

        if relative_path.starts_with('/') {
            // Absolute path - return as-is
            relative_path.to_string()
        } else {
            // Relative path - join with base
            format!("{}/{}", base_path.trim_end_matches('/'), relative_path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_local_path() {
        let uri = StorageUri::parse("/tmp/images").unwrap();
        assert_eq!(uri, StorageUri::LocalPath("/tmp/images".to_string()));
        assert_eq!(uri.to_local_path(), "/tmp/images");
    }

    #[test]
    fn test_parse_file_uri() {
        let uri = StorageUri::parse("file:///tmp/images").unwrap();
        assert_eq!(uri, StorageUri::FileUri("/tmp/images".to_string()));
        assert_eq!(uri.to_local_path(), "/tmp/images");
    }

    #[test]
    fn test_parse_invalid_file_uri() {
        // Relative path in file:// URI should fail
        let result = StorageUri::parse("file://tmp/images");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), StorageError::InvalidUri(_)));
    }

    #[test]
    fn test_parse_unsupported_scheme() {
        let result = StorageUri::parse("s3://bucket/path");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StorageError::UnsupportedScheme(_)
        ));
    }

    #[test]
    fn test_resolve_relative_path() {
        let uri = StorageUri::parse("/tmp/images").unwrap();

        // Relative path
        assert_eq!(
            uri.resolve_relative_path("test.jpg"),
            "/tmp/images/test.jpg"
        );

        // Absolute path
        assert_eq!(
            uri.resolve_relative_path("/absolute/path.jpg"),
            "/absolute/path.jpg"
        );

        // Path with slash at end
        let uri_with_slash = StorageUri::parse("/tmp/images/").unwrap();
        assert_eq!(
            uri_with_slash.resolve_relative_path("test.jpg"),
            "/tmp/images/test.jpg"
        );
    }

    #[test]
    fn test_file_uri_resolve_relative_path() {
        let uri = StorageUri::parse("file:///tmp/images").unwrap();
        assert_eq!(
            uri.resolve_relative_path("test.jpg"),
            "/tmp/images/test.jpg"
        );
    }
}
