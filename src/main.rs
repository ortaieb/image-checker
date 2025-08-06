use image_checker::handlers::{
    check_status, get_results, handle_404, health_check, queue_stats, submit_validation,
};
use image_checker::{Config, ProcessingQueue};

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::str::FromStr;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use tracing::{error, info, warn, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Initialize logging
    init_logging();

    info!(
        "Starting Image Checker service v{}",
        env!("CARGO_PKG_VERSION")
    );

    // Load configuration
    let config = match Config::from_env() {
        Ok(config) => {
            info!("Configuration loaded successfully");
            info!("  +- Image base directory: {}", config.image_base_dir);
            info!("  +---------- LLM API URL: {}", config.llm_api_url);
            info!("  +------------ LLM MODEL: {}", config.llm_model_name);
            info!("  +------------Queue size: {}", config.queue_size);
            config
        }
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    // Create processing queue
    let queue = ProcessingQueue::new(&config);
    info!(
        "Processing queue initialized with size: {}",
        config.queue_size
    );

    // Build the application router
    let app = build_router(queue.clone());

    // Parse server address
    let addr = match SocketAddr::from_str(&config.server_address()) {
        Ok(addr) => addr,
        Err(e) => {
            error!("Invalid server address {}: {}", config.server_address(), e);
            std::process::exit(1);
        }
    };

    info!("Server starting on {}", addr);

    // Start the server with graceful shutdown
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => {
            info!("Server bound to {}", addr);
            listener
        }
        Err(e) => {
            error!("Failed to bind to {}: {}", addr, e);
            std::process::exit(1);
        }
    };

    // Start server with graceful shutdown
    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(queue))
        .await
    {
        error!("Server error: {}", e);
        std::process::exit(1);
    }

    info!("Image Checker service stopped");
}

fn init_logging() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "image_checker=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

fn build_router(queue: ProcessingQueue) -> Router {
    Router::new()
        // API routes
        .route("/validate", post(submit_validation))
        .route("/status/:id", get(check_status))
        .route("/results/:id", get(get_results))
        // Health and monitoring routes
        .route("/health", get(health_check))
        .route("/stats", get(queue_stats))
        // 404 handler
        .fallback(handle_404)
        // Add shared state
        .with_state(queue)
        // Add middleware
        .layer(
            ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                        .on_request(DefaultOnRequest::new().level(Level::INFO))
                        .on_response(DefaultOnResponse::new().level(Level::INFO)),
                )
                .layer(CorsLayer::permissive()), // Allow CORS for development
        )
}

async fn shutdown_signal(queue: ProcessingQueue) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, starting graceful shutdown");
        },
        _ = terminate => {
            info!("Received SIGTERM, starting graceful shutdown");
        },
    }

    // Signal the queue to stop processing new requests
    info!("Shutting down processing queue...");
    queue.shutdown().await;

    // Give some time for in-flight requests to complete
    warn!("Waiting 10 seconds for in-flight requests to complete...");
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    info!("Graceful shutdown complete");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_check_route() {
        let config = Config {
            host: "127.0.0.1".to_string(),
            port: 3000,
            image_base_dir: "/tmp".to_string(),
            llm_api_url: "http://localhost:8080".to_string(),
            llm_model_name: "llava:7b".to_string(),
            request_timeout_seconds: 30,
            processing_timeout_minutes: 5,
            queue_size: 100,
            throttle_requests_per_minute: 60,
        };

        let queue = ProcessingQueue::new(&config);
        let app = build_router(queue);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_404_handler() {
        let config = Config {
            host: "127.0.0.1".to_string(),
            port: 3000,
            image_base_dir: "/tmp".to_string(),
            llm_api_url: "http://localhost:8080".to_string(),
            llm_model_name: "llava:7b".to_string(),
            request_timeout_seconds: 30,
            processing_timeout_minutes: 5,
            queue_size: 100,
            throttle_requests_per_minute: 60,
        };

        let queue = ProcessingQueue::new(&config);
        let app = build_router(queue);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
