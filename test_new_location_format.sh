#!/bin/bash
# Test script for new location format

# Set environment variables
export IMAGE_BASE_DIR=/tmp/test-images
export LLM_API_URL=http://localhost:11434/v1/chat/completions
export LLM_MODEL_NAME=llava:13b
export HOST=127.0.0.1
export PORT=3001

echo "Testing new location format..."

# Start the service in the background
cargo run &
SERVER_PID=$!

# Wait for server to start
sleep 3

# Test with new location format
echo "Sending validation request with new location format..."
curl -X POST http://localhost:3001/validate \
  -H "Content-Type: application/json" \
  -d '{
    "processing-id": "location-test-001",
    "image-path": "test.jpg",
    "analysis-request": {
      "content": "A simple test image or placeholder",
      "location": {
        "long": -0.266108,
        "lat": 51.492191,
        "max_distance": 100.0
      }
    }
  }' && echo

# Wait a bit for processing
sleep 2

# Check status
echo "Checking status..."
curl -s http://localhost:3001/status/location-test-001 | jq . && echo

# Get results
echo "Getting results..."
curl -s http://localhost:3001/results/location-test-001 | jq . && echo

# Clean up
kill $SERVER_PID
echo "Test completed"