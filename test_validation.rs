use image_checker::validation::llm::{validate_image_content, LlmClient};
use std::time::Duration;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::init();

    println!("Testing LLM validation with Ollama...");

    // Create LLM client
    let client = LlmClient::new(
        "http://localhost:11434/v1/chat/completions".to_string(),
        "llava:13b".to_string(),
        Duration::from_secs(30),
    );

    // Test with our test image
    let image_path = "/tmp/test-images/test.jpg";
    let content_description = "A test image with placeholder content";

    println!("Validating image: {}", image_path);
    println!("Expected content: {}", content_description);

    match validate_image_content(&client, image_path, content_description).await {
        Ok(is_valid) => {
            println!(
                "Validation result: {}",
                if is_valid { "ACCEPTED" } else { "REJECTED" }
            );
        }
        Err(e) => {
            println!("Validation failed: {}", e);
        }
    }
}
