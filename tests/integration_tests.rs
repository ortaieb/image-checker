use image_checker::handlers::*;
use image_checker::{Config, ProcessingQueue};

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use serde_json::json;
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

fn create_test_app() -> Router {
    let config = create_test_config();
    let queue = ProcessingQueue::new(&config);

    Router::new()
        .route("/validate", post(submit_validation))
        .route("/status/:id", get(check_status))
        .route("/results/:id", get(get_results))
        .route("/health", get(health_check))
        .route("/stats", get(queue_stats))
        .with_state(queue)
}

#[tokio::test]
async fn test_submit_validation_endpoint() {
    let app = create_test_app();

    let request_body = json!({
        "processing-id": "test001",
        "image-path": "/tmp/test.jpg",
        "analysis-request": {
            "content": "Three birds on a wire"
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

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(response_json["processing-id"], "test001");
    assert_eq!(response_json["status"], "accepted");
}

#[tokio::test]
async fn test_submit_validation_missing_fields() {
    let app = create_test_app();

    // Test missing processing-id
    let request_body = json!({
        "processing-id": "",
        "image-path": "/tmp/test.jpg",
        "analysis-request": {
            "content": "test"
        }
    });

    let response = app
        .clone()
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

    // Test missing content
    let request_body = json!({
        "processing-id": "test002",
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
async fn test_status_endpoint() {
    let app = create_test_app();

    // Submit a request first
    let request_body = json!({
        "processing-id": "test003",
        "image-path": "/tmp/test.jpg",
        "analysis-request": {
            "content": "test content"
        }
    });

    let _submit_response = app
        .clone()
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

    // Check status
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/status/test003")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(response_json["processing-id"], "test003");
    assert!(["accepted", "in_progress", "completed", "failed"]
        .contains(&response_json["status"].as_str().unwrap()));
}

#[tokio::test]
async fn test_status_not_found() {
    let app = create_test_app();

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
async fn test_results_not_found() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/results/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_health_endpoint() {
    let app = create_test_app();

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

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(response_json["status"], "healthy");
    assert!(response_json["version"].is_string());
    assert!(response_json["queue_stats"].is_object());
}

#[tokio::test]
async fn test_stats_endpoint() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(response_json["total"].is_number());
    assert!(response_json["accepted"].is_number());
    assert!(response_json["in_progress"].is_number());
    assert!(response_json["completed"].is_number());
    assert!(response_json["failed"].is_number());
}

#[tokio::test]
async fn test_validation_request_with_location_and_datetime() {
    let app = create_test_app();

    let request_body = json!({
        "processing-id": "test004",
        "image-path": "/tmp/test.jpg",
        "analysis-request": {
            "content": "Pub sign The Ale and Hops",
            "location": {
                "long": -0.266108,
                "lat": 51.492191,
                "max_distance": 100.0
            },
            "datetime": "image was taken not more than 10 minutes after 2025-08-01T15:23:00Z+1"
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

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(response_json["processing-id"], "test004");
    assert_eq!(response_json["status"], "accepted");
}

#[tokio::test]
async fn test_validation_request_with_null_image() {
    let app = create_test_app();

    let request_body = json!({
        "processing-id": "test005",
        "image": null,
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

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
