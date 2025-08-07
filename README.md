# Image-Checker AI Agent

A high-performance Rust-based async web service that validates images against multiple criteria using AI vision models and EXIF metadata analysis. The service provides content validation, location verification, and timestamp checking with robust queue management for parallel processing.

## 🚀 Features

### Core Functionality
- **AI-Powered Content Validation** - Uses LLaVa multimodal AI to verify image content matches descriptions
- **EXIF Metadata Processing** - Extracts GPS coordinates, timestamps, and camera information
- **Location Verification** - Validates GPS coordinates against specified locations using Haversine distance
- **Timestamp Validation** - Checks image capture time against specified time windows
- **Async Queue Management** - Parallel processing with status tracking and throttling
- **Production-Ready** - Comprehensive error handling, logging, metrics, and graceful shutdown

### API Capabilities
- RESTful HTTP API with JSON request/response
- Real-time processing status tracking
- Queue statistics and health monitoring
- Configurable timeout and retry mechanisms
- CORS support for web applications

## 📋 Table of Contents

- [Installation](#installation)
- [Configuration](#configuration)
- [API Reference](#api-reference)
- [Usage Examples](#usage-examples)
- [Architecture](#architecture)
- [Development](#development)
- [Testing](#testing)
- [Deployment](#deployment)
- [Troubleshooting](#troubleshooting)

## 🛠 Installation

### Prerequisites

- **Rust 1.79.0+** - Install from [rustup.rs](https://rustup.rs/)
- **LLaVa Model Server** - Running on accessible endpoint (e.g., Ollama with LLaVa)
- **System Dependencies** - Standard build tools for your platform

### Build from Source

```bash
# Clone the repository
git clone <repository-url>
cd image-checker

# Build release binary
cargo build --release

# The binary will be available at ./target/release/image-checker
```

### Quick Start

```bash
# Create image directory
mkdir -p /tmp/images

# Create environment configuration
cat > .env << EOF
HOST=127.0.0.1
PORT=3000
IMAGE_BASE_DIR=/tmp/images
LLM_API_URL=http://localhost:11434
LLM_MODEL_NAME=llava:7b
REQUEST_TIMEOUT_SECONDS=30
PROCESSING_TIMEOUT_MINUTES=5
QUEUE_SIZE=100
THROTTLE_REQUESTS_PER_MINUTE=60
EOF

# Start the service
./target/release/image-checker
```

## ⚙️ Configuration

The service uses environment variables for configuration. Create a `.env` file or set these variables:

### Required Configuration

| Variable | Description | Example |
|----------|-------------|---------|
| `IMAGE_BASE_DIR` | Directory containing images to validate | `/tmp/images` |
| `LLM_API_URL` | URL of the LLaVa API endpoint | `http://localhost:11434` |

### Optional Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HOST` | `127.0.0.1` | Server bind address |
| `PORT` | `3000` | Server port |
| `LLM_MODEL_NAME` | `llava:7b` | Name of the LLaVa model to use |
| `REQUEST_TIMEOUT_SECONDS` | `30` | HTTP request timeout |
| `PROCESSING_TIMEOUT_MINUTES` | `5` | Maximum processing time per request |
| `QUEUE_SIZE` | `100` | Maximum concurrent requests in queue |
| `THROTTLE_REQUESTS_PER_MINUTE` | `60` | Rate limiting threshold |

### Configuration Example

```bash
# Production configuration
HOST=0.0.0.0
PORT=8080
IMAGE_BASE_DIR=/opt/images
LLM_API_URL=https://llava-api.example.com
LLM_MODEL_NAME=llava:13b
REQUEST_TIMEOUT_SECONDS=60
PROCESSING_TIMEOUT_MINUTES=10
QUEUE_SIZE=500
THROTTLE_REQUESTS_PER_MINUTE=120
```

## 📚 API Reference

### Base URL
```
http://localhost:3000
```

### Endpoints

#### 1. Submit Validation Request

**POST** `/validate`

Submit an image for validation against specified criteria.

**Request Body:**
```json
{
  "processing-id": "unique-request-id",
  "image-path": "path/to/image.jpg",
  "analysis-request": {
    "content": "Description of expected image content",
    "location": {
      "long": -0.266108,
      "lat": 51.492191,
      "max_distance": 100.0
    },
    "datetime": {
      "start": "2025-08-01T15:23:00+01:00",
      "duration": 10
    }
  }
}
```

**Alternative with base64 image:**
```json
{
  "processing-id": "unique-request-id",
  "image": "data:image/jpeg;base64,/9j/4AAQSkZJRgABA...",
  "analysis-request": {
    "content": "Description of expected image content"
  }
}
```

**Response:**
```json
{
  "processing-id": "unique-request-id",
  "status": "accepted"
}
```

**Location Constraint Format:**
The `location` field is optional but if provided, all three fields are required:
- `long` (f64): Longitude in decimal degrees (-180.0 to 180.0)
- `lat` (f64): Latitude in decimal degrees (-90.0 to 90.0)  
- `max_distance` (f64): Maximum allowed distance from coordinates in meters

**DateTime Constraint Format:**
The `datetime` field is optional but if provided, exactly two out of three fields are required:
- `start` (string): Start time in ISO 8601 format (e.g., "2025-08-01T15:23:00+01:00")
- `end` (string): End time in ISO 8601 format (e.g., "2025-08-01T15:33:00+01:00")
- `duration` (u64): Duration in minutes

Valid combinations:
- `start` + `end`: Define explicit time range
- `start` + `duration`: Start time with duration 
- `end` + `duration`: End time with duration (calculates start time)

**Status Codes:**
- `202 Accepted` - Request queued successfully
- `400 Bad Request` - Invalid request format
- `429 Too Many Requests` - Queue is full
- `503 Service Unavailable` - Service shutting down

#### 2. Check Processing Status

**GET** `/status/{processing-id}`

Check the current status of a validation request.

**Response:**
```json
{
  "processing-id": "unique-request-id",
  "status": "completed"
}
```

**Status Values:**
- `accepted` - Request received and queued
- `in_progress` - Currently being processed
- `completed` - Processing finished successfully
- `failed` - Processing encountered an error

**Status Codes:**
- `200 OK` - Status retrieved successfully
- `404 Not Found` - Processing ID not found

#### 3. Get Validation Results

**GET** `/results/{processing-id}`

Retrieve the results of a completed validation.

**Response (Success):**
```json
{
  "processing-id": "unique-request-id",
  "results": {
    "resolution": "accepted",
    "reasons": null
  }
}
```

**Response (Rejection):**
```json
{
  "processing-id": "unique-request-id",
  "results": {
    "resolution": "rejected",
    "reasons": [
      "image content does not match description: 'Three birds on a wire'",
      "image location 51.489123°N, 0.268456°W is 150.2m from expected location 51.492191°N, 0.266108°W, exceeding 100.0m limit"
    ]
  }
}
```

**Status Codes:**
- `200 OK` - Results retrieved successfully
- `202 Accepted` - Processing not yet complete
- `404 Not Found` - Processing ID not found
- `500 Internal Server Error` - Processing failed

#### 4. Health Check

**GET** `/health`

Get service health status and metrics.

**Response:**
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "queue_stats": {
    "total": 15,
    "accepted": 2,
    "in_progress": 1,
    "completed": 10,
    "failed": 2,
    "available_permits": 45
  }
}
```

#### 5. Queue Statistics

**GET** `/stats`

Get detailed queue processing statistics.

**Response:**
```json
{
  "total": 15,
  "accepted": 2,
  "in_progress": 1,
  "completed": 10,
  "failed": 2,
  "available_permits": 45
}
```

## 💡 Usage Examples

### Basic Content Validation

```bash
curl -X POST http://localhost:3000/validate \
  -H "Content-Type: application/json" \
  -d '{
    "processing-id": "content-check-001",
    "image-path": "photos/sunset.jpg",
    "analysis-request": {
      "content": "A beautiful sunset over the ocean"
    }
  }'
```

### Location and Time Validation

```bash
curl -X POST http://localhost:3000/validate \
  -H "Content-Type: application/json" \
  -d '{
    "processing-id": "location-time-001",
    "image-path": "photos/pub-sign.jpg",
    "analysis-request": {
      "content": "Pub sign The Ale and Hops",
      "location": {
        "long": -0.266108,
        "lat": 51.492191,
        "max_distance": 100.0
      },
      "datetime": {
        "start": "2025-08-01T15:23:00+01:00",
        "duration": 10
      }
    }
  }'
```

### Base64 Image Validation

```bash
curl -X POST http://localhost:3000/validate \
  -H "Content-Type: application/json" \
  -d '{
    "processing-id": "base64-001",
    "image": "data:image/jpeg;base64,/9j/4AAQSkZJRgABAQEAYABgAAD...",
    "analysis-request": {
      "content": "A cat sitting on a windowsill"
    }
  }'
```

### DateTime Constraint Examples

```bash
# Example 1: Start time + Duration (10 minute window from start)
curl -X POST http://localhost:3000/validate \
  -H "Content-Type: application/json" \
  -d '{
    "processing-id": "datetime-001",
    "image-path": "photo.jpg",
    "analysis-request": {
      "content": "A sunset photo",
      "datetime": {
        "start": "2025-08-01T19:30:00+01:00",
        "duration": 10
      }
    }
  }'

# Example 2: Start time + End time (explicit range)
curl -X POST http://localhost:3000/validate \
  -H "Content-Type: application/json" \
  -d '{
    "processing-id": "datetime-002", 
    "image-path": "photo.jpg",
    "analysis-request": {
      "content": "A sunset photo",
      "datetime": {
        "start": "2025-08-01T19:30:00+01:00",
        "end": "2025-08-01T19:40:00+01:00"
      }
    }
  }'

# Example 3: End time + Duration (10 minutes before end)
curl -X POST http://localhost:3000/validate \
  -H "Content-Type: application/json" \
  -d '{
    "processing-id": "datetime-003",
    "image-path": "photo.jpg", 
    "analysis-request": {
      "content": "A sunset photo",
      "datetime": {
        "end": "2025-08-01T19:40:00+01:00",
        "duration": 10
      }
    }
  }'
```

### Complete Validation Flow

```bash
# 1. Submit validation
RESPONSE=$(curl -s -X POST http://localhost:3000/validate \
  -H "Content-Type: application/json" \
  -d '{
    "processing-id": "flow-demo-001",
    "image-path": "test.jpg",
    "analysis-request": {
      "content": "A red bicycle"
    }
  }')

echo "Submitted: $RESPONSE"

# 2. Check status
curl -s http://localhost:3000/status/flow-demo-001 | jq .

# 3. Get results
curl -s http://localhost:3000/results/flow-demo-001 | jq .

# 4. Check queue stats
curl -s http://localhost:3000/stats | jq .
```

### Monitoring and Health Checks

```bash
# Health check
curl -s http://localhost:3000/health | jq .

# Queue statistics
curl -s http://localhost:3000/stats | jq .

# Service monitoring script
while true; do
  STATUS=$(curl -s http://localhost:3000/health | jq -r '.status')
  QUEUE_TOTAL=$(curl -s http://localhost:3000/stats | jq -r '.total')
  echo "$(date): Status=$STATUS, Total Processed=$QUEUE_TOTAL"
  sleep 30
done
```

## 🏗 Architecture

### System Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   HTTP Client   │────│  Image-Checker  │────│   LLaVa API     │
│                 │    │    Service      │    │                 │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                              │
                              │
                       ┌─────────────────┐
                       │  File System    │
                       │  (Images)       │  
                       └─────────────────┘
```

### Component Overview

#### Core Components

1. **HTTP Server (Axum)** - RESTful API endpoints with middleware
2. **Processing Queue** - Async task queue with status tracking
3. **Validation Processor** - Coordinates content and metadata validation
4. **LLM Client** - Interface to LLaVa multimodal AI model
5. **EXIF Extractor** - GPS and timestamp metadata extraction
6. **Configuration Manager** - Environment-based configuration

#### Processing Flow

```
Request → Validation → Queue → Processing → Results
    ↓         ↓          ↓         ↓          ↓
  Parse    Validate   Enqueue   Validate   Store
  JSON     Request    Task      Content    Results
           Format               Metadata
```

#### Validation Pipeline

1. **Input Validation** - Verify request format and required fields
2. **Image Loading** - Load image from path or decode base64
3. **Parallel Processing**:
   - **Content Validation** - Send to LLaVa API for analysis
   - **Metadata Extraction** - Parse EXIF data for GPS/timestamp
4. **Constraint Checking**:
   - **Location Validation** - Calculate distance using Haversine formula
   - **Datetime Validation** - Compare timestamps with time windows
5. **Result Aggregation** - Combine all validation results

### Data Models

#### ValidationRequest
```rust
pub struct ValidationRequest {
    pub processing_id: String,
    pub image_path: Option<String>,
    pub image: Option<String>, // Base64 encoded
    pub analysis_request: AnalysisRequest,
}
```

#### AnalysisRequest
```rust
pub struct AnalysisRequest {
    pub content: String,
    pub location: Option<LocationRequest>,
    pub datetime: Option<DateTimeRequest>,
}

pub struct LocationRequest {
    pub long: f64,    // longitude
    pub lat: f64,     // latitude  
    pub max_distance: f64,  // maximum distance in meters
}

pub struct DateTimeRequest {
    pub start: Option<String>,    // ISO 8601 datetime string
    pub end: Option<String>,      // ISO 8601 datetime string  
    pub duration: Option<u64>,    // duration in minutes
}
```

#### ValidationResponse
```rust
pub struct ValidationResponse {
    pub processing_id: String,
    pub results: ValidationResults,
}

pub struct ValidationResults {
    pub resolution: Resolution, // Accepted | Rejected
    pub reasons: Option<Vec<String>>,
}
```

## 🔧 Development

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Watch mode (requires cargo-watch)
cargo watch -x run
```

### Code Quality

```bash
# Run all tests
cargo test

# Run specific test suite
cargo test --test integration_tests
cargo test --test validation_tests

# Run clippy linter
cargo clippy -- -D warnings

# Format code
cargo fmt

# Check without building
cargo check
```

### Development Server

```bash
# Run with automatic reload
RUST_LOG=debug cargo run

# Run specific environment
cargo run --bin image-checker
```

### Project Structure

```
src/
├── main.rs              # Application entry point
├── lib.rs               # Library root
├── config.rs            # Configuration management
├── models.rs            # Data structures and JSON models
├── handlers.rs          # HTTP request handlers
├── queue.rs             # Async processing queue
├── utils.rs             # Utility functions (distance, formatting)
└── validation/
    ├── mod.rs           # Validation module exports
    ├── processor.rs     # Main validation coordinator
    ├── llm.rs           # LLaVa API integration
    └── exif.rs          # EXIF metadata extraction

tests/
├── integration_tests.rs # API endpoint tests
└── validation_tests.rs  # Business logic tests

examples/
├── example01-just_summary.md
├── example02-validate_content_and_metadata.md
├── example03-validate_content_and_metadata.md
└── example04-invalid_data.md
```

## 🧪 Testing

### Test Suites

1. **Unit Tests** - Individual component testing
2. **Integration Tests** - API endpoint testing  
3. **Validation Tests** - Business logic testing

### Running Tests

```bash
# All tests
cargo test

# Unit tests only
cargo test --lib

# Integration tests only
cargo test --test integration_tests

# Validation tests only
cargo test --test validation_tests

# Test with output
cargo test -- --nocapture

# Test specific module
cargo test config::tests

# Test with coverage (requires cargo-tarpaulin)
cargo tarpaulin --out html
```

### Test Coverage

The test suite covers:
- ✅ All API endpoints (9 integration tests)
- ✅ Request/response parsing and validation
- ✅ EXIF metadata extraction and GPS conversion
- ✅ Distance calculations and coordinate validation
- ✅ Datetime parsing and constraint validation
- ✅ Queue management and status tracking
- ✅ Error handling and edge cases
- ✅ Configuration loading and validation

### Example Test

```rust
#[tokio::test]
async fn test_validation_with_location_constraint() {
    let request = ValidationRequest {
        processing_id: "test-001".to_string(),
        image_path: Some("test.jpg".to_string()),
        analysis_request: AnalysisRequest {
            content: "A red bicycle".to_string(),
            location: Some(LocationRequest {
                long: -0.266108,
                lat: 51.492191,
                max_distance: 50.0,
            }),
            datetime: Some(DateTimeRequest {
                start: Some("2025-08-01T15:23:00+01:00".to_string()),
                end: None,
                duration: Some(10),
            }),
        }
    };
    
    // Test request processing
    assert!(validate_request_format(&request).is_ok());
}
```

## 🚀 Deployment

### Docker Deployment

```dockerfile
FROM rust:1.79 as builder
WORKDIR /usr/src/app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates
COPY --from=builder /usr/src/app/target/release/image-checker /usr/local/bin/
EXPOSE 3000
CMD ["image-checker"]
```

```bash
# Build and run
docker build -t image-checker .
docker run -p 3000:3000 --env-file .env image-checker
```

### Systemd Service

```ini
# /etc/systemd/system/image-checker.service
[Unit]
Description=Image Checker AI Agent
After=network.target

[Service]
Type=simple
User=image-checker
WorkingDirectory=/opt/image-checker
ExecStart=/opt/image-checker/image-checker
Restart=always
RestartSec=10
EnvironmentFile=/opt/image-checker/.env

[Install]
WantedBy=multi-user.target
```

```bash
# Deploy and start
sudo systemctl enable image-checker
sudo systemctl start image-checker
sudo systemctl status image-checker
```

### Production Configuration

```bash
# Production environment variables
HOST=0.0.0.0
PORT=8080
IMAGE_BASE_DIR=/opt/images
LLM_API_URL=https://api.example.com/llava
REQUEST_TIMEOUT_SECONDS=60
PROCESSING_TIMEOUT_MINUTES=10
QUEUE_SIZE=1000
THROTTLE_REQUESTS_PER_MINUTE=300
RUST_LOG=info
```

### Load Balancing

```nginx
# nginx.conf
upstream image_checker {
    server 127.0.0.1:8080;
    server 127.0.0.1:8081;
    server 127.0.0.1:8082;
}

server {
    listen 80;
    server_name api.example.com;
    
    location / {
        proxy_pass http://image_checker;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

## 🔍 Troubleshooting

### Common Issues

#### Service Won't Start

**Problem:** `Failed to bind to 127.0.0.1:3000`
```bash
# Check if port is in use
netstat -tlnp | grep :3000
# Kill existing process
pkill -f image-checker
```

**Problem:** `Image base directory does not exist`
```bash
# Create the directory
mkdir -p /tmp/images
# Or update configuration
export IMAGE_BASE_DIR=/path/to/existing/directory
```

#### LLaVa API Connection Issues

**Problem:** `LLM processing error: Connection refused`
```bash
# Check LLaVa service is running
curl http://localhost:11434/api/version

# Start Ollama with LLaVa model
ollama serve
ollama pull llava:7b
```

**Problem:** `Request timeout after 30s`
```bash
# Increase timeout in configuration
export REQUEST_TIMEOUT_SECONDS=120
```

#### Image Processing Errors

**Problem:** `Image file not found: /path/to/image.jpg`
- Verify the image file exists and is readable
- Check `IMAGE_BASE_DIR` configuration
- Ensure proper file permissions

**Problem:** `Invalid image format: Invalid jpg file format`
- Verify the file is a valid image format (JPEG, PNG, GIF, BMP)
- Check file corruption
- Try with a different image

#### Queue and Performance Issues

**Problem:** `Queue is full, please retry later`
```bash
# Increase queue size
export QUEUE_SIZE=500
# Or check queue statistics
curl http://localhost:3000/stats
```

**Problem:** High memory usage
- Reduce `QUEUE_SIZE` for lower memory footprint
- Implement image size limits
- Monitor with queue statistics

### Debug Logging

```bash
# Enable debug logging
export RUST_LOG=debug

# Specific module logging
export RUST_LOG=image_checker=debug,image_checker::validation=trace

# Log to file
./image-checker 2>&1 | tee service.log
```

### Health Monitoring

```bash
#!/bin/bash
# health-check.sh
HEALTH_URL="http://localhost:3000/health"
STATUS=$(curl -s "$HEALTH_URL" | jq -r '.status // "error"')

if [ "$STATUS" != "healthy" ]; then
    echo "Service unhealthy: $STATUS"
    exit 1
fi

echo "Service is healthy"
exit 0
```

### Performance Monitoring

```bash
# Monitor queue statistics
watch -n 5 'curl -s http://localhost:3000/stats | jq .'

# Monitor system resources
htop -p $(pgrep image-checker)

# Network connections
netstat -tulpn | grep image-checker
```

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📞 Support

For support and questions:
- Create an issue in the repository
- Check the troubleshooting section above
- Review the API documentation and examples

---

**Built with ❤️ in Rust** | **Production-Ready** | **Async & High-Performance**