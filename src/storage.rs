use std::path::{Path, PathBuf};
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
    LocalPath(PathBuf),
    FileUri(PathBuf),
    // Future: S3Uri(String), GcsUri(String), etc.
}

impl StorageUri {
    /// Parses a URI string into a StorageUri enum variant
    ///
    /// # Examples
    /// ```rust
    /// let uri = StorageUri::parse("/tmp/images")?;
    /// let file_uri = StorageUri::parse("file:///tmp/images")?;
    /// ```
    pub fn parse(uri: &str) -> Result<Self, StorageError> {
        if let Some(path_str) = uri.strip_prefix("file://") {
            // Handle file:// URI
            // Validate that it's an absolute path
            if !path_str.starts_with('/') {
                return Err(StorageError::InvalidUri(format!(
                    "file:// URI must use absolute paths: {uri}"
                )));
            }

            Ok(StorageUri::FileUri(PathBuf::from(path_str)))
        } else if uri.contains("://") {
            // Other URI schemes - extract scheme name for better error reporting
            let scheme = uri
                .split_once("://")
                .map(|(scheme, _)| scheme)
                .ok_or_else(|| StorageError::InvalidUri(uri.to_string()))?;
            Err(StorageError::UnsupportedScheme(scheme.to_string()))
        } else {
            // Treat as local path (backward compatibility)
            Ok(StorageUri::LocalPath(PathBuf::from(uri)))
        }
    }

    /// Returns the local filesystem path for this URI as a string slice
    /// Maintains backward compatibility by returning &str
    #[must_use]
    pub fn to_local_path(&self) -> &str {
        match self {
            StorageUri::LocalPath(path) | StorageUri::FileUri(path) => {
                // This is safe because we construct PathBuf from valid UTF-8 strings
                path.to_str().expect("Path should be valid UTF-8")
            }
        }
    }

    /// Checks if the path exists on the filesystem
    #[must_use]
    pub fn exists(&self) -> bool {
        match self {
            StorageUri::LocalPath(path) | StorageUri::FileUri(path) => path.exists(),
        }
    }

    /// Resolves a relative path against this URI's base path
    /// Returns a String for backward compatibility
    ///
    /// # Examples
    /// ```rust
    /// let uri = StorageUri::parse("/tmp/images")?;
    /// assert_eq!(uri.resolve_relative_path("test.jpg"), "/tmp/images/test.jpg");
    /// assert_eq!(uri.resolve_relative_path("/absolute/path.jpg"), "/absolute/path.jpg");
    /// ```
    #[must_use]
    pub fn resolve_relative_path(&self, relative_path: &str) -> String {
        if relative_path.starts_with('/') {
            // Absolute path - return as-is
            relative_path.to_string()
        } else {
            // Relative path - join with base using PathBuf for correctness
            // then convert back to String for compatibility
            let base_path = match self {
                StorageUri::LocalPath(path) | StorageUri::FileUri(path) => path,
            };

            let resolved = base_path.join(relative_path);
            resolved.to_string_lossy().into_owned()
        }
    }

    /// Internal method to get the PathBuf
    #[must_use]
    fn as_path_buf(&self) -> &PathBuf {
        match self {
            StorageUri::LocalPath(path) | StorageUri::FileUri(path) => path,
        }
    }
}

impl AsRef<Path> for StorageUri {
    fn as_ref(&self) -> &Path {
        self.as_path_buf().as_path()
    }
}

impl std::fmt::Display for StorageUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageUri::LocalPath(path) => write!(f, "{}", path.display()),
            StorageUri::FileUri(path) => write!(f, "file://{}", path.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_local_path() {
        let uri = StorageUri::parse("/tmp/images").unwrap();
        assert_eq!(uri, StorageUri::LocalPath(PathBuf::from("/tmp/images")));
        assert_eq!(uri.to_local_path(), "/tmp/images");
    }

    #[test]
    fn test_parse_file_uri() {
        let uri = StorageUri::parse("file:///tmp/images").unwrap();
        assert_eq!(uri, StorageUri::FileUri(PathBuf::from("/tmp/images")));
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

    // Additional comprehensive tests
    #[test]
    fn test_parse_empty_string() {
        let uri = StorageUri::parse("").unwrap();
        assert_eq!(uri, StorageUri::LocalPath(PathBuf::from("")));
        assert_eq!(uri.to_local_path(), "");
    }

    #[test]
    fn test_parse_relative_local_path() {
        let uri = StorageUri::parse("relative/path").unwrap();
        assert_eq!(uri, StorageUri::LocalPath(PathBuf::from("relative/path")));
        assert_eq!(uri.to_local_path(), "relative/path");
    }

    #[test]
    fn test_parse_malformed_uri_without_scheme_separator() {
        // Without "://" it should be treated as a local path
        let result = StorageUri::parse("not:a:uri");
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            StorageUri::LocalPath(PathBuf::from("not:a:uri"))
        );
    }

    #[test]
    fn test_parse_file_uri_with_extra_slashes() {
        let uri = StorageUri::parse("file:////tmp///images//").unwrap();
        assert_eq!(uri, StorageUri::FileUri(PathBuf::from("//tmp///images//")));
        // PathBuf preserves the exact string
        assert_eq!(uri.to_local_path(), "//tmp///images//");
    }

    #[test]
    fn test_unsupported_scheme_error_message() {
        let result = StorageUri::parse("s3://bucket/path");
        match result {
            Err(StorageError::UnsupportedScheme(scheme)) => {
                assert_eq!(scheme, "s3");
            }
            _ => panic!("Expected UnsupportedScheme error"),
        }
    }

    #[test]
    fn test_invalid_uri_error_message() {
        let result = StorageUri::parse("file://relative/path");
        match result {
            Err(StorageError::InvalidUri(msg)) => {
                assert!(msg.contains("file:// URI must use absolute paths"));
                assert!(msg.contains("file://relative/path"));
            }
            _ => panic!("Expected InvalidUri error"),
        }
    }

    #[test]
    fn test_as_ref_trait() {
        let uri = StorageUri::parse("/tmp/images").unwrap();
        let path_ref: &Path = uri.as_ref();
        assert_eq!(path_ref, Path::new("/tmp/images"));
    }

    #[test]
    fn test_display_local_path() {
        let uri = StorageUri::LocalPath(PathBuf::from("/tmp/test"));
        assert_eq!(format!("{}", uri), "/tmp/test");
    }

    #[test]
    fn test_display_file_uri() {
        let uri = StorageUri::FileUri(PathBuf::from("/tmp/test"));
        assert_eq!(format!("{}", uri), "file:///tmp/test");
    }

    #[test]
    fn test_resolve_relative_path_with_dots() {
        let uri = StorageUri::parse("/tmp/images").unwrap();

        // Current directory
        assert_eq!(
            uri.resolve_relative_path("./test.jpg"),
            "/tmp/images/./test.jpg"
        );

        // Parent directory
        assert_eq!(
            uri.resolve_relative_path("../test.jpg"),
            "/tmp/images/../test.jpg"
        );
    }

    #[test]
    fn test_resolve_empty_relative_path() {
        let uri = StorageUri::parse("/tmp/images").unwrap();
        assert_eq!(uri.resolve_relative_path(""), "/tmp/images/");
    }

    #[test]
    fn test_clone_trait() {
        let uri = StorageUri::parse("/tmp/test").unwrap();
        let cloned = uri.clone();
        assert_eq!(uri, cloned);
    }

    #[test]
    fn test_debug_trait() {
        let uri = StorageUri::LocalPath(PathBuf::from("/tmp/test"));
        let debug_str = format!("{:?}", uri);
        assert!(debug_str.contains("LocalPath"));
        assert!(debug_str.contains("/tmp/test"));
    }

    #[test]
    fn test_io_error_conversion() {
        use std::io;

        let io_error = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let storage_error: StorageError = io_error.into();

        assert!(matches!(storage_error, StorageError::Io(_)));
        assert!(storage_error.to_string().contains("IO error"));
    }

    #[test]
    fn test_error_display() {
        let error = StorageError::UnsupportedScheme("gcs".to_string());
        assert_eq!(error.to_string(), "Unsupported URI scheme: gcs");

        let error = StorageError::InvalidUri("bad://uri".to_string());
        assert_eq!(error.to_string(), "Invalid URI format: bad://uri");

        let error = StorageError::PathNotFound("/missing".to_string());
        assert_eq!(error.to_string(), "Path does not exist: /missing");
    }

    #[test]
    fn test_parse_scheme_edge_cases() {
        // Scheme with numbers
        let result = StorageUri::parse("http2://example.com");
        assert!(result.is_err());
        match result {
            Err(StorageError::UnsupportedScheme(s)) => assert_eq!(s, "http2"),
            _ => panic!("Expected UnsupportedScheme error"),
        }

        // Scheme with hyphens
        let result = StorageUri::parse("my-scheme://example.com");
        assert!(result.is_err());
        match result {
            Err(StorageError::UnsupportedScheme(s)) => assert_eq!(s, "my-scheme"),
            _ => panic!("Expected UnsupportedScheme error"),
        }

        // Empty scheme
        let result = StorageUri::parse("://example.com");
        assert!(result.is_err());
        match result {
            Err(StorageError::UnsupportedScheme(s)) => assert!(s.is_empty()),
            _ => panic!("Expected UnsupportedScheme error"),
        }
    }
}
