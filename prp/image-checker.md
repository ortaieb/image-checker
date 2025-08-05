# Image-Checker PRP - Advanced AI Image Validation Service

## Goal
Build a high-performance Rust-based async web service that validates images against multiple criteria (content, location, timestamp) using AI vision models and EXIF metadata analysis, with robust queue management for parallel processing.

## Why
- **User Value**: Provides automated image validation for location-based verification, content compliance, and timestamp verification
- **Business Impact**: Enables scalable image verification workflows without manual intervention
- **Integration Need**: Serves as a reusable microservice for applications requiring trusted image validation
- **Performance Critical**: Handles multiple concurrent validation requests with controlled resource usage

## What
An async Rust web service that:
- Accepts image validation requests via JSON API
- Extracts EXIF metadata (GPS coordinates, timestamps) using pure Rust libraries
- Sends images to LLaVa model for content analysis via HTTP
- Returns structured validation results with detailed reasoning
- Manages request queues with configurable throttling and cancellation
- Provides status tracking endpoints for long-running validations

### Success Criteria
- [ ] Processes image validation requests with <2s response time for queue acceptance
- [ ] Extracts GPS coordinates from EXIF with ±10m accuracy validation
- [ ] Integrates with LLaVa model for content validation via HTTP API
- [ ] Handles queue overflow gracefully with retryable error responses
- [ ] Supports request cancellation after configurable timeout
- [ ] Validates location within specified distance threshold
- [ ] Validates timestamps within specified time windows
- [ ] Returns structured JSON responses matching example format

## All Needed Context

### Documentation & References
```yaml
# CRITICAL READING - Core Dependencies and Patterns

- url: "https://docs.rs/kamadak-exif/latest/exif/"
  why: "EXIF metadata extraction - GPS coordinates and timestamps"
  critical: "Use Reader::read_from_container for image files, get_field() for specific tags"

- url: "https://docs.rs/axum/latest/axum/"
  why: "Async web framework - latest 0.8.0 patterns and routing"
  critical: "Path syntax changed to /{param}, no more #[async_trait] needed"

- url: "https://github.com/EricLBuehler/mistral.rs"
  why: "LLaVa integration - multimodal AI HTTP client"
  critical: "Provides HTTP server at localhost:8080, Rust async API available"

- url: "https://github.com/softprops/envy"
  why: "Type-safe environment variable configuration with serde"
  critical: "Use envy::from_env::<Config>() with #[derive(Deserialize)]"

- file: "examples/example01-just_summary.md"
  why: "Expected JSON request/response format for content-only validation"

- file: "examples/example02-validate_content_and_metadata.md" 
  why: "GPS coordinate and timestamp validation format"

- file: "examples/example04-invalid_data.md"
  why: "Error handling and rejection response format"

- docfile: "CLAUDE.md"
  why: "Project conventions, testing patterns, file organization rules"
```

### Current Codebase Tree
```bash
image-checker/
├── CLAUDE.md                    # Project conventions and rules
├── initial.md                   # Feature requirements
├── examples/                    # Expected behavior examples
│   ├── example01-just_summary.md
│   ├── example02-validate_content_and_metadata.md
│   ├── example03-validate_content_and_metadata.md
│   └── example04-invalid_data.md
└── prp/
    └── templates/
        └── prp_base.md
```

### Desired Codebase Tree
```bash
image-checker/
├── Cargo.toml                   # Dependencies and project config
├── src/
│   ├── main.rs                  # Application entry point and server setup
│   ├── config.rs                # Environment-based configuration
│   ├── models.rs                # Request/response data structures
│   ├── handlers.rs              # HTTP request handlers
│   ├── queue.rs                 # Async processing queue management
│   ├── validation/
│   │   ├── mod.rs              # Validation module interface
│   │   ├── exif.rs             # EXIF metadata extraction and validation
│   │   ├── llm.rs              # LLaVa model integration
│   │   └── processor.rs        # Main validation logic coordinator
│   └── utils.rs                # Distance calculation and utilities
├── tests/
│   ├── integration_tests.rs     # API endpoint testing
│   └── validation_tests.rs      # Core validation logic testing
└── .env.example                 # Environment variable template
```

### Known Gotchas & Library Quirks
```rust
// CRITICAL: kamadak-exif GPS coordinate extraction
// GPS coordinates stored as degrees/minutes/seconds rationals
// Must convert to decimal degrees: degrees + minutes/60 + seconds/3600

// CRITICAL: Axum 0.8.0 syntax changes (January 2025)
// OLD: "/:id" -> NEW: "/{id}"
// OLD: "/*path" -> NEW: "/{*path}"
// No more #[async_trait] needed for return-position impl Trait

// CRITICAL: mistral.rs multimodal integration
// Requires spawning HTTP server process or using Rust API directly
// Image data must be base64 encoded for HTTP API calls
// Rate limiting necessary - models can be resource intensive

// CRITICAL: Tokio channel queue management
// Use bounded channels to prevent memory exhaustion
// Single consumer pattern prevents parallel resource conflicts
// Channel closure signals graceful shutdown

// CRITICAL: Distance calculation from GPS coordinates
// Use haversine formula for accurate distance on sphere
// Earth radius: 6371 kilometers
// Account for coordinate precision limitations in EXIF
```

## Implementation Blueprint

### Data Models and Structure
Create type-safe data structures matching the JSON API format from examples:

```rust
// Core request/response models with serde serialization
#[derive(Deserialize)]
struct ValidationRequest {
    processing_id: String,
    image_path: String,
    analysis_request: AnalysisRequest,
}

#[derive(Deserialize)]
struct AnalysisRequest {
    content: String,
    location: Option<String>,  // "not more than 100m from coordinates (lat, lon)"
    datetime: Option<String>,  // "not more than 10 minutes after timestamp"
}

#[derive(Serialize)]
struct ValidationResponse {
    processing_id: String,
    results: ValidationResults,
}

// Environment-based configuration with defaults
#[derive(Deserialize)]
struct Config {
    image_base_dir: String,
    llm_api_url: String,
    llm_model_name: String,
    request_timeout_seconds: u64,
    queue_size: usize,
    throttle_requests_per_minute: u32,
    processing_timeout_minutes: u64,
}
```

### Task List for Implementation

```yaml
Task 1: Setup Project Structure
CREATE Cargo.toml:
  - DEPENDENCIES: axum = "0.8", tokio = { version = "1", features = ["full"] }
  - DEPENDENCIES: serde = { version = "1.0", features = ["derive"] }
  - DEPENDENCIES: kamadak-exif = "0.5", reqwest = { version = "0.11", features = ["json"] }
  - DEPENDENCIES: envy = "0.4", dotenvy = "0.15"
  - DEV-DEPENDENCIES: tokio-test = "0.4"

CREATE .env.example:
  - TEMPLATE: All required environment variables with example values
  - INCLUDE: IMAGE_BASE_DIR, LLM_API_URL, QUEUE_SIZE, etc.

Task 2: Configuration System
CREATE src/config.rs:
  - PATTERN: Use envy::from_env::<Config>() for type-safe loading
  - VALIDATION: Ensure required fields present, validate URLs and paths
  - DEFAULTS: Sensible fallbacks for optional configuration

Task 3: Data Models
CREATE src/models.rs:
  - MIRROR: JSON structures from examples/ directory exactly
  - SERDE: Derive Serialize/Deserialize for all API types
  - VALIDATION: Custom serde deserializers for coordinate parsing

Task 4: EXIF Metadata Extraction
CREATE src/validation/exif.rs:
  - CORE: Use kamadak-exif Reader::read_from_container()
  - GPS: Extract GPSLatitude/GPSLongitude and convert to decimal degrees
  - TIMESTAMP: Extract DateTime and DateTimeOriginal tags
  - ERROR: Handle missing metadata gracefully with detailed errors

Task 5: Distance Calculation Utilities  
CREATE src/utils.rs:
  - HAVERSINE: Implement accurate distance calculation between GPS coordinates
  - PARSING: Parse location strings like "not more than 100m from coordinates (lat, lon)"
  - VALIDATION: Distance threshold checking with configurable tolerance

Task 6: LLM Integration
CREATE src/validation/llm.rs:
  - HTTP CLIENT: Use reqwest for async HTTP calls to LLaVa API
  - ENCODING: Base64 encode images for API transmission
  - PROMPT: Construct validation prompts for content analysis
  - RETRY: Implement retry logic with exponential backoff

Task 7: Queue Management System
CREATE src/queue.rs:
  - CHANNEL: Use tokio::sync::mpsc for bounded processing queue
  - WORKER: Single consumer task for sequential processing
  - TRACKING: Request status tracking with HashMap<String, ProcessingStatus>
  - CANCELLATION: Timeout-based request cancellation

Task 8: Validation Processor
CREATE src/validation/processor.rs:
  - COORDINATOR: Main validation logic orchestrating EXIF and LLM checks
  - ASYNC: Parallel execution of independent validation steps
  - RESULTS: Aggregate validation results with detailed reasoning
  - ERROR: Comprehensive error handling and user-friendly messages

Task 9: HTTP Handlers
CREATE src/handlers.rs:
  - POST /validate: Accept validation requests, return processing-id
  - GET /status/{id}: Check processing status
  - GET /results/{id}: Retrieve validation results
  - ERROR: Proper HTTP status codes and error responses

Task 10: Main Application
CREATE src/main.rs:
  - SETUP: Load configuration, initialize queue system
  - ROUTING: Configure Axum routes with proper middleware
  - GRACEFUL: Signal handling for graceful shutdown
  - LOGGING: Structured logging for debugging and monitoring

Task 11: Integration Tests
CREATE tests/integration_tests.rs:
  - API: Test all endpoints with sample data
  - QUEUE: Test queue overflow and status tracking
  - ERROR: Test error conditions and response formats
  - E2E: End-to-end validation workflow tests

Task 12: Unit Tests
ADD to each module (#[cfg(test)]):
  - EXIF: Test GPS coordinate extraction and conversion
  - DISTANCE: Test haversine distance calculations
  - CONFIG: Test environment variable loading
  - VALIDATION: Test individual validation components
```

### Per-Task Pseudocode

```rust
// Task 4: EXIF GPS Extraction - CRITICAL IMPLEMENTATION DETAILS
fn extract_gps_coordinates(image_path: &str) -> Result<(f64, f64), ExifError> {
    // PATTERN: Always validate file exists first
    let file = std::fs::File::open(image_path)?;
    let mut reader = std::io::BufReader::new(&file);
    
    // CRITICAL: Use kamadak-exif Reader pattern
    let exif_reader = exif::Reader::new();
    let exif = exif_reader.read_from_container(&mut reader)?;
    
    // GOTCHA: GPS stored as degrees/minutes/seconds rationals
    let lat_deg = extract_gps_rational(&exif, Tag::GPSLatitude)?;
    let lat_ref = extract_gps_ref(&exif, Tag::GPSLatitudeRef)?;
    let lon_deg = extract_gps_rational(&exif, Tag::GPSLongitude)?;
    let lon_ref = extract_gps_ref(&exif, Tag::GPSLongitudeRef)?;
    
    // CONVERT: DMS to decimal degrees
    let latitude = dms_to_decimal(lat_deg) * if lat_ref == "S" { -1.0 } else { 1.0 };
    let longitude = dms_to_decimal(lon_deg) * if lon_ref == "W" { -1.0 } else { 1.0 };
    
    Ok((latitude, longitude))
}

// Task 7: Queue Management - CRITICAL ASYNC PATTERN  
async fn start_processing_queue(config: Config) -> ProcessingQueue {
    // PATTERN: Bounded channel prevents memory exhaustion
    let (tx, mut rx) = tokio::sync::mpsc::channel::<ValidationRequest>(config.queue_size);
    let status_map = Arc::new(Mutex::new(HashMap::new()));
    
    // CRITICAL: Single consumer prevents resource conflicts
    let worker_status = status_map.clone();
    tokio::spawn(async move {
        while let Some(request) = rx.recv().await {
            // PATTERN: Update status before processing
            update_status(&worker_status, &request.processing_id, ProcessingStatus::InProgress).await;
            
            // THROTTLING: Respect rate limits
            rate_limiter.acquire().await;
            
            // PROCESSING: Handle with timeout and cancellation
            let result = tokio::time::timeout(
                Duration::from_secs(config.processing_timeout_seconds),
                process_validation_request(request, &config)
            ).await;
            
            // CRITICAL: Always update status on completion
            match result {
                Ok(Ok(validation_result)) => {
                    update_status(&worker_status, &request.processing_id, 
                                ProcessingStatus::Completed(validation_result)).await;
                }
                _ => {
                    update_status(&worker_status, &request.processing_id, 
                                ProcessingStatus::Failed("timeout or error".into())).await;
                }
            }
        }
    });
    
    ProcessingQueue { sender: tx, status: status_map }
}
```

### Integration Points
```yaml
CONFIGURATION:
  - pattern: "IMAGE_BASE_DIR=/app/images"
  - pattern: "LLM_API_URL=http://localhost:8080/v1/chat/completions"
  - pattern: "QUEUE_SIZE=100"
  - pattern: "THROTTLE_REQUESTS_PER_MINUTE=60"

ROUTES:
  - add to: src/main.rs
  - pattern: "app.route('/validate', post(handlers::submit_validation))"
  - pattern: "app.route('/status/{id}', get(handlers::check_status))"
  - pattern: "app.route('/results/{id}', get(handlers::get_results))"

ERROR_HANDLING:
  - pattern: "Custom error types implementing std::error::Error"
  - pattern: "HTTP status codes: 202 Accepted, 429 Too Many Requests, 404 Not Found"
  - pattern: "Structured error responses matching examples/example04-invalid_data.md"
```

## Validation Loop

### Level 1: Syntax & Style
```bash
# Run these FIRST - fix any errors before proceeding
cargo fmt --check        # Rust formatting
cargo clippy -- -D warnings  # Linting with warnings as errors

# Expected: No errors. If errors, READ the clippy suggestions and fix.
```

### Level 2: Unit Tests
```rust
// CREATE src/validation/exif.rs tests
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_gps_extraction_with_valid_exif() {
        // Test with known GPS coordinates
        let (lat, lon) = extract_gps_coordinates("tests/fixtures/gps_image.jpg").unwrap();
        assert!((lat - 51.491079).abs() < 0.000001);
        assert!((lon + 0.269590).abs() < 0.000001);
    }
    
    #[test]
    fn test_distance_calculation() {
        let coord1 = (51.491079, -0.269590);
        let coord2 = (51.492191, -0.266108);
        let distance = haversine_distance(coord1, coord2);
        assert!(distance < 300.0); // Should be less than 300m
    }
    
    #[test]
    fn test_missing_gps_data() {
        let result = extract_gps_coordinates("tests/fixtures/no_gps_image.jpg");
        assert!(result.is_err());
    }
}
```

```bash
# Run and iterate until passing:
cargo test --lib
# If failing: Read error messages, understand root cause, fix code, re-run
```

### Level 3: Integration Test
```bash
# Start the service in background
RUST_LOG=debug cargo run &
SERVICE_PID=$!

# Wait for startup
sleep 2

# Test validation endpoint
curl -X POST http://localhost:3000/validate \
  -H "Content-Type: application/json" \
  -d '{
    "processing-id": "test001",
    "image-path": "/app/images/test.jpg",
    "analysis-request": {
      "content": "Three birds on a wire"
    }
  }'

# Expected: {"processing-id": "test001", "status": "accepted"}

# Test status check
curl http://localhost:3000/status/test001
# Expected: {"processing-id": "test001", "status": "in_progress"} or "completed"

# Cleanup
kill $SERVICE_PID
```

## Final Validation Checklist
- [ ] All tests pass: `cargo test --all-features`
- [ ] No linting errors: `cargo fmt --check && cargo clippy -- -D warnings`
- [ ] Integration test successful: HTTP endpoints respond correctly
- [ ] Queue overflow handled: Returns 429 status when queue full
- [ ] GPS extraction works: Can extract coordinates from EXIF data
- [ ] Distance validation accurate: Haversine formula within ±1m precision
- [ ] LLM integration functional: Can communicate with LLaVa API
- [ ] Error cases handled: Missing files, invalid data, timeouts
- [ ] Configuration flexible: All settings via environment variables
- [ ] Graceful shutdown: Processes in-flight requests before exit

---

## Anti-Patterns to Avoid
- ❌ Don't use `unwrap()` in production code - use proper error handling
- ❌ Don't block the async runtime - use async/await for I/O operations
- ❌ Don't ignore EXIF precision limitations - GPS can have ±10m accuracy
- ❌ Don't make parallel LLM requests - respect rate limits and resource usage
- ❌ Don't store images in memory - stream processing for large files
- ❌ Don't hardcode paths or URLs - use environment configuration
- ❌ Don't forget request cancellation - implement proper timeout handling

---

## PRP Confidence Score: 9/10

**Reasoning**: This PRP provides comprehensive context including:
- ✅ Specific library documentation and version details (Axum 0.8.0, kamadak-exif)
- ✅ Real-world integration examples (mistral.rs for LLaVa)
- ✅ Detailed implementation pseudocode with critical gotchas
- ✅ Executable validation steps with expected outputs
- ✅ Complete project structure and file organization
- ✅ Type-safe configuration patterns with environment variables
- ✅ Async queue management patterns for resource control

**Minor Risk**: LLaVa integration complexity might require iteration on the HTTP client details, but mistral.rs documentation provides a solid foundation for implementation.