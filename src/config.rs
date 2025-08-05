use serde::Deserialize;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Environment variable error: {0}")]
    EnvVar(#[from] envy::Error),
    #[error("Invalid configuration: {0}")]
    Validation(String),
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

    fn validate(&self) -> Result<(), ConfigError> {
        // Validate image base directory exists or can be created
        if !Path::new(&self.image_base_dir).exists() {
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
        
        // Set minimal required env vars
        env::set_var("IMAGE_BASE_DIR", "/tmp");
        env::set_var("LLM_API_URL", "http://localhost:8080");
        env::set_var("QUEUE_SIZE", "100");

        let config = Config::from_env().expect("Failed to load config with defaults");

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
        // Clear any existing env vars that might interfere
        env::remove_var("QUEUE_SIZE");
        env::remove_var("HOST");
        env::remove_var("PORT");
        env::remove_var("LLM_MODEL_NAME");
        env::remove_var("REQUEST_TIMEOUT_SECONDS");
        env::remove_var("PROCESSING_TIMEOUT_MINUTES");
        env::remove_var("THROTTLE_REQUESTS_PER_MINUTE");
        
        env::set_var("IMAGE_BASE_DIR", "/tmp");
        env::set_var("LLM_API_URL", "invalid-url");
        env::set_var("QUEUE_SIZE", "100");

        let result = Config::from_env();
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("must start with http:// or https://"));
    }

    #[test]
    fn test_config_validation_invalid_queue_size() {
        env::set_var("IMAGE_BASE_DIR", "/tmp");
        env::set_var("LLM_API_URL", "http://localhost:8080");
        env::set_var("QUEUE_SIZE", "0");

        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Queue size must be between"));
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
        
        env::set_var("IMAGE_BASE_DIR", "/tmp");
        env::set_var("LLM_API_URL", "http://localhost:8080");
        env::set_var("QUEUE_SIZE", "100");

        let config = Config::from_env().expect("Failed to load config");

        assert_eq!(config.request_timeout(), Duration::from_secs(30));
        assert_eq!(config.processing_timeout(), Duration::from_secs(300)); // 5 minutes
        assert_eq!(config.server_address(), "127.0.0.1:3000");
    }
}
