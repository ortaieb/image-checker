use crate::models::{
    ProcessingRequest, ProcessingStatus, StatusResponse, ValidationRequest, ValidationResponse,
};
use crate::queue::{ProcessingQueue, QueueError, QueueStats};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    Json as JsonExtractor,
};
use serde::Serialize;
use tracing::{debug, error, warn};

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SubmitResponse {
    #[serde(rename = "processing-id")]
    pub processing_id: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub queue_stats: QueueStats,
}

pub async fn submit_validation(
    State(queue): State<ProcessingQueue>,
    JsonExtractor(request): JsonExtractor<ValidationRequest>,
) -> Result<(StatusCode, Json<SubmitResponse>), (StatusCode, Json<ApiResponse<()>>)> {
    // Generate processing request with auto-generated ID
    let processing_request = ProcessingRequest::from_request(request);

    debug!(
        "Received validation request, assigned ID: {}",
        processing_request.processing_id
    );

    if processing_request.analysis_request.content.is_empty() {
        warn!("Validation request missing content description");
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(
                "content description is required".to_string(),
            )),
        ));
    }

    // Check if image path is provided
    if processing_request.get_image_path().is_none() {
        warn!("Validation request missing image path");
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("image path is required".to_string())),
        ));
    }

    // Submit to processing queue
    match queue.submit_validation(processing_request.clone()).await {
        Ok(()) => {
            debug!(
                "Successfully queued validation request: {}",
                processing_request.processing_id
            );
            Ok((
                StatusCode::ACCEPTED,
                Json(SubmitResponse {
                    processing_id: processing_request.processing_id,
                    status: "accepted".to_string(),
                }),
            ))
        }
        Err(QueueError::QueueFull) => {
            warn!(
                "Queue is full, rejecting request: {}",
                processing_request.processing_id
            );
            Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(ApiResponse::error(
                    "queue is full, please retry later".to_string(),
                )),
            ))
        }
        Err(QueueError::QueueClosed) => {
            error!(
                "Queue is closed, rejecting request: {}",
                processing_request.processing_id
            );
            Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiResponse::error("service is shutting down".to_string())),
            ))
        }
        Err(e) => {
            error!(
                "Queue error for request {}: {}",
                processing_request.processing_id, e
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("internal server error".to_string())),
            ))
        }
    }
}

pub async fn check_status(
    State(queue): State<ProcessingQueue>,
    Path(processing_id): Path<String>,
) -> Result<Json<StatusResponse>, (StatusCode, Json<ApiResponse<()>>)> {
    debug!("Checking status for: {}", processing_id);

    let status = queue.get_status(&processing_id).await;

    match status {
        ProcessingStatus::NotFound => {
            debug!("Processing ID not found: {}", processing_id);
            Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse::error("processing ID not found".to_string())),
            ))
        }
        _ => {
            debug!("Status for {}: {:?}", processing_id, status);
            Ok(Json(StatusResponse {
                processing_id,
                status,
            }))
        }
    }
}

pub async fn get_results(
    State(queue): State<ProcessingQueue>,
    Path(processing_id): Path<String>,
) -> Result<Json<ValidationResponse>, (StatusCode, Json<ApiResponse<()>>)> {
    debug!("Getting results for: {}", processing_id);

    // First check if the processing ID exists
    let status = queue.get_status(&processing_id).await;

    match status {
        ProcessingStatus::NotFound => {
            debug!("Processing ID not found: {}", processing_id);
            Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse::error("processing ID not found".to_string())),
            ))
        }
        ProcessingStatus::Accepted | ProcessingStatus::InProgress => {
            debug!("Results not ready for: {}", processing_id);
            Err((
                StatusCode::ACCEPTED,
                Json(ApiResponse::error("processing not complete".to_string())),
            ))
        }
        ProcessingStatus::Failed => {
            debug!("Processing failed for: {}", processing_id);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("processing failed".to_string())),
            ))
        }
        ProcessingStatus::Completed => match queue.get_result(&processing_id).await {
            Some(result) => {
                debug!("Returning results for: {}", processing_id);
                Ok(Json(result))
            }
            None => {
                error!("Results missing for completed request: {}", processing_id);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::error("results not available".to_string())),
                ))
            }
        },
    }
}

pub async fn health_check(State(queue): State<ProcessingQueue>) -> Json<HealthResponse> {
    debug!("Health check requested");

    let queue_stats = queue.get_queue_stats().await;

    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        queue_stats,
    })
}

pub async fn queue_stats(State(queue): State<ProcessingQueue>) -> Json<QueueStats> {
    debug!("Queue stats requested");

    Json(queue.get_queue_stats().await)
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub details: Option<String>,
}

pub async fn handle_404() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "endpoint not found".to_string(),
            details: Some("check the API documentation for available endpoints".to_string()),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        Router,
    };
    use tower::util::ServiceExt;

    fn create_test_config() -> Config {
        Config {
            host: "127.0.0.1".to_string(),
            port: 3000,
            image_base_dir: "/tmp".to_string(),
            llm_api_url: "http://localhost:8080".to_string(),
            llm_model_name: "llava:7b".to_string(),
            request_timeout_seconds: 30,
            processing_timeout_minutes: 5,
            queue_size: 100,
            throttle_requests_per_minute: 60,
        }
    }

    #[tokio::test]
    async fn test_submit_validation_missing_content() {
        let config = create_test_config();
        let queue = ProcessingQueue::new(&config);

        let app = Router::new()
            .route("/validate", axum::routing::post(submit_validation))
            .with_state(queue);

        let request_body = serde_json::json!({
            "image-path": "/tmp/test.jpg",
            "analysis-request": {
                "content": ""
            }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/validate")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_submit_validation_valid_request() {
        let config = create_test_config();
        let queue = ProcessingQueue::new(&config);

        let app = Router::new()
            .route("/validate", axum::routing::post(submit_validation))
            .with_state(queue);

        let request_body = serde_json::json!({
            "image-path": "/tmp/test.jpg",
            "analysis-request": {
                "content": "test content"
            }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/validate")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn test_check_status_not_found() {
        let config = create_test_config();
        let queue = ProcessingQueue::new(&config);

        let app = Router::new()
            .route("/status/:id", axum::routing::get(check_status))
            .with_state(queue);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/status/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_health_check() {
        let config = create_test_config();
        let queue = ProcessingQueue::new(&config);

        let app = Router::new()
            .route("/health", axum::routing::get(health_check))
            .with_state(queue);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
