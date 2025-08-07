use crate::storage::{StorageError, StorageUri};
use serde::Deserialize;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Environment variable error: {0}")]
    EnvVar(#[from] envy::Error),
    #[error("Invalid configuration: {0}")]
    Validation(String),
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,

    pub image_base_dir: String,

    pub llm_api_url: String,

    #[serde(default = "default_llm_model_name")]
    pub llm_model_name: String,

    #[serde(default = "default_request_timeout_seconds")]
    pub request_timeout_seconds: u64,

    #[serde(default = "default_processing_timeout_minutes")]
    pub processing_timeout_minutes: u64,

    #[serde(default = "default_queue_size")]
    pub queue_size: usize,

    #[serde(default = "default_throttle_requests_per_minute")]
    pub throttle_requests_per_minute: u32,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();

        let config: Config = envy::from_env()?;
        config.validate()?;
        Ok(config)
    }

    pub fn get_storage_uri(&self) -> Result<StorageUri, StorageError> {
        StorageUri::parse(&self.image_base_dir)
    }

    #[cfg(test)]
    fn from_env_no_dotenv() -> Result<Self, ConfigError> {
        let config: Config = envy::from_env()?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        // Validate image base directory or URI format
        let storage_uri = self.get_storage_uri()?;

        // Check if the storage location exists (for local paths and file:// URIs)
        if !storage_uri.exists() {
            return Err(ConfigError::Validation(format!(
                "Image base directory does not exist: {}",
                self.image_base_dir
            )));
        }

        // Validate LLM API URL format
        if !self.llm_api_url.starts_with("http://") && !self.llm_api_url.starts_with("https://") {
            return Err(ConfigError::Validation(format!(
                "LLM API URL must start with http:// or https://: {}",
                self.llm_api_url
            )));
        }

        // Validate reasonable queue size
        if self.queue_size == 0 || self.queue_size > 10000 {
            return Err(ConfigError::Validation(format!(
                "Queue size must be between 1 and 10000, got: {}",
                self.queue_size
            )));
        }

        // Validate throttle rate
        if self.throttle_requests_per_minute == 0 {
            return Err(ConfigError::Validation(
                "Throttle requests per minute must be greater than 0".into(),
            ));
        }

        Ok(())
    }

    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.request_timeout_seconds)
    }

    pub fn processing_timeout(&self) -> Duration {
        Duration::from_secs(self.processing_timeout_minutes * 60)
    }

    pub fn throttle_interval(&self) -> Duration {
        Duration::from_secs(60 / self.throttle_requests_per_minute as u64)
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3000
}

fn default_llm_model_name() -> String {
    "llava:7b".to_string()
}

fn default_request_timeout_seconds() -> u64 {
    30
}

fn default_processing_timeout_minutes() -> u64 {
    5
}

fn default_queue_size() -> usize {
    100
}

fn default_throttle_requests_per_minute() -> u32 {
    60
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_config_defaults() {
        // Clear any existing env vars that might interfere
        env::remove_var("QUEUE_SIZE");
        env::remove_var("HOST");
        env::remove_var("PORT");
        env::remove_var("LLM_MODEL_NAME");
        env::remove_var("REQUEST_TIMEOUT_SECONDS");
        env::remove_var("PROCESSING_TIMEOUT_MINUTES");
        env::remove_var("THROTTLE_REQUESTS_PER_MINUTE");
        env::remove_var("IMAGE_BASE_DIR");
        env::remove_var("LLM_API_URL");

        // Set minimal required env vars
        env::set_var("IMAGE_BASE_DIR", "/tmp");
        env::set_var("LLM_API_URL", "http://localhost:8080");
        env::set_var("QUEUE_SIZE", "100");

        let config = Config::from_env_no_dotenv().expect("Failed to load config with defaults");

        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 3000);
        assert_eq!(config.llm_model_name, "llava:7b");
        assert_eq!(config.request_timeout_seconds, 30);
        assert_eq!(config.processing_timeout_minutes, 5);
        assert_eq!(config.queue_size, 100);
        assert_eq!(config.throttle_requests_per_minute, 60);
    }

    #[test]
    fn test_config_validation_invalid_url() {
        // Manually create config with invalid URL to avoid env conflicts
        let config = Config {
            host: "127.0.0.1".to_string(),
            port: 3000,
            image_base_dir: "/tmp".to_string(),
            llm_api_url: "invalid-url".to_string(), // This should cause validation to fail
            llm_model_name: "llava:7b".to_string(),
            request_timeout_seconds: 30,
            processing_timeout_minutes: 5,
            queue_size: 100,
            throttle_requests_per_minute: 60,
        };

        let result = config.validate();
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("must start with http:// or https://"));
    }

    #[test]
    fn test_config_validation_invalid_queue_size() {
        // Use a different environment variable scope to avoid conflicts
        let test_env_vars = vec![
            ("TEST_IMAGE_BASE_DIR", "/tmp"),
            ("TEST_LLM_API_URL", "http://localhost:8080"),
            ("TEST_QUEUE_SIZE", "0"),
        ];

        // Set test-specific env vars
        for (key, value) in &test_env_vars {
            env::set_var(key, value);
        }

        // Manually create config with test values to avoid env conflicts
        let config = Config {
            host: "127.0.0.1".to_string(),
            port: 3000,
            image_base_dir: "/tmp".to_string(),
            llm_api_url: "http://localhost:8080".to_string(),
            llm_model_name: "llava:7b".to_string(),
            request_timeout_seconds: 30,
            processing_timeout_minutes: 5,
            queue_size: 0, // This should cause validation to fail
            throttle_requests_per_minute: 60,
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Queue size must be between"));

        // Clean up
        for (key, _) in &test_env_vars {
            env::remove_var(key);
        }
    }

    #[test]
    fn test_helper_methods() {
        // Clear any existing env vars that might interfere
        env::remove_var("QUEUE_SIZE");
        env::remove_var("HOST");
        env::remove_var("PORT");
        env::remove_var("LLM_MODEL_NAME");
        env::remove_var("REQUEST_TIMEOUT_SECONDS");
        env::remove_var("PROCESSING_TIMEOUT_MINUTES");
        env::remove_var("THROTTLE_REQUESTS_PER_MINUTE");
        env::remove_var("IMAGE_BASE_DIR");
        env::remove_var("LLM_API_URL");

        env::set_var("IMAGE_BASE_DIR", "/tmp");
        env::set_var("LLM_API_URL", "http://localhost:8080");
        env::set_var("QUEUE_SIZE", "100");

        let config = Config::from_env_no_dotenv().expect("Failed to load config");

        assert_eq!(config.request_timeout(), Duration::from_secs(30));
        assert_eq!(config.processing_timeout(), Duration::from_secs(300)); // 5 minutes
        assert_eq!(config.server_address(), "127.0.0.1:3000");
    }

    #[test]
    fn test_config_with_file_uri() {
        let config = Config {
            host: "127.0.0.1".to_string(),
            port: 3000,
            image_base_dir: "file:///tmp".to_string(),
            llm_api_url: "http://localhost:8080".to_string(),
            llm_model_name: "llava:7b".to_string(),
            request_timeout_seconds: 30,
            processing_timeout_minutes: 5,
            queue_size: 100,
            throttle_requests_per_minute: 60,
        };

        // Should validate successfully
        let result = config.validate();
        assert!(result.is_ok(), "Config validation failed: {:?}", result);

        // Should parse storage URI correctly
        let storage_uri = config.get_storage_uri().unwrap();
        assert_eq!(storage_uri.to_local_path(), "/tmp");
    }

    #[test]
    fn test_config_with_unsupported_uri() {
        let config = Config {
            host: "127.0.0.1".to_string(),
            port: 3000,
            image_base_dir: "s3://bucket/path".to_string(),
            llm_api_url: "http://localhost:8080".to_string(),
            llm_model_name: "llava:7b".to_string(),
            request_timeout_seconds: 30,
            processing_timeout_minutes: 5,
            queue_size: 100,
            throttle_requests_per_minute: 60,
        };

        // Should fail validation due to unsupported scheme
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unsupported URI scheme"));
    }

    #[test]
    fn test_config_with_invalid_file_uri() {
        let config = Config {
            host: "127.0.0.1".to_string(),
            port: 3000,
            image_base_dir: "file://relative/path".to_string(), // Invalid - must be absolute
            llm_api_url: "http://localhost:8080".to_string(),
            llm_model_name: "llava:7b".to_string(),
            request_timeout_seconds: 30,
            processing_timeout_minutes: 5,
            queue_size: 100,
            throttle_requests_per_minute: 60,
        };

        // Should fail validation due to invalid URI format
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid URI format"));
    }
}
