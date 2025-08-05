use crate::config::Config;
use crate::models::{
    ProcessingStatus, ValidationRequest, ValidationResponse,
};
use crate::validation::ValidationProcessor;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock, Semaphore};
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

#[derive(Debug)]
pub enum QueueItem {
    ValidationRequest(ValidationRequest),
    StatusQuery(String, tokio::sync::oneshot::Sender<ProcessingStatus>),
    ResultQuery(
        String,
        tokio::sync::oneshot::Sender<Option<ValidationResponse>>,
    ),
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct ProcessingRecord {
    pub status: ProcessingStatus,
    pub submitted_at: Instant,
    pub started_at: Option<Instant>,
    pub completed_at: Option<Instant>,
    pub result: Option<ValidationResponse>,
}

impl Default for ProcessingRecord {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessingRecord {
    pub fn new() -> Self {
        Self {
            status: ProcessingStatus::Accepted,
            submitted_at: Instant::now(),
            started_at: None,
            completed_at: None,
            result: None,
        }
    }

    pub fn start_processing(&mut self) {
        self.status = ProcessingStatus::InProgress;
        self.started_at = Some(Instant::now());
    }

    pub fn complete_with_result(&mut self, result: ValidationResponse) {
        self.status = ProcessingStatus::Completed;
        self.completed_at = Some(Instant::now());
        self.result = Some(result);
    }

    pub fn fail(&mut self) {
        self.status = ProcessingStatus::Failed;
        self.completed_at = Some(Instant::now());
    }

    pub fn is_expired(&self, timeout: Duration) -> bool {
        self.submitted_at.elapsed() > timeout
    }
}

#[derive(Clone)]
pub struct ProcessingQueue {
    sender: mpsc::Sender<QueueItem>,
    status_map: Arc<RwLock<HashMap<String, ProcessingRecord>>>,
    throttle_semaphore: Arc<Semaphore>,
}

impl ProcessingQueue {
    pub fn new(config: &Config) -> Self {
        let (sender, receiver) = mpsc::channel(config.queue_size);
        let status_map = Arc::new(RwLock::new(HashMap::new()));
        let throttle_semaphore =
            Arc::new(Semaphore::new(config.throttle_requests_per_minute as usize));

        let queue = ProcessingQueue {
            sender,
            status_map: status_map.clone(),
            throttle_semaphore: throttle_semaphore.clone(),
        };

        // Start the worker task
        let worker_config = config.clone();
        let worker_status_map = status_map.clone();
        let worker_throttle = throttle_semaphore.clone();

        tokio::spawn(async move {
            Self::worker_task(receiver, worker_config, worker_status_map, worker_throttle).await;
        });

        // Start cleanup task for expired records
        let cleanup_status_map = status_map.clone();
        let cleanup_timeout = config.processing_timeout();

        tokio::spawn(async move {
            Self::cleanup_task(cleanup_status_map, cleanup_timeout).await;
        });

        queue
    }

    pub async fn submit_validation(&self, request: ValidationRequest) -> Result<(), QueueError> {
        // Check if queue is full
        if self.sender.is_closed() {
            return Err(QueueError::QueueClosed);
        }

        // Add to status tracking
        {
            let mut status_map = self.status_map.write().await;
            status_map.insert(request.processing_id.clone(), ProcessingRecord::new());
        }

        // Send to processing queue
        self.sender
            .send(QueueItem::ValidationRequest(request))
            .await
            .map_err(|_| QueueError::QueueFull)?;

        Ok(())
    }

    pub async fn get_status(&self, processing_id: &str) -> ProcessingStatus {
        let status_map = self.status_map.read().await;

        status_map
            .get(processing_id)
            .map(|record| record.status.clone())
            .unwrap_or(ProcessingStatus::NotFound)
    }

    pub async fn get_result(&self, processing_id: &str) -> Option<ValidationResponse> {
        let status_map = self.status_map.read().await;

        status_map
            .get(processing_id)
            .and_then(|record| record.result.clone())
    }

    pub async fn shutdown(&self) {
        if let Err(e) = self.sender.send(QueueItem::Shutdown).await {
            warn!("Failed to send shutdown signal: {}", e);
        }
    }

    async fn worker_task(
        mut receiver: mpsc::Receiver<QueueItem>,
        config: Config,
        status_map: Arc<RwLock<HashMap<String, ProcessingRecord>>>,
        throttle_semaphore: Arc<Semaphore>,
    ) {
        info!("Processing queue worker started");

        let processor = ValidationProcessor::new(&config);

        while let Some(item) = receiver.recv().await {
            match item {
                QueueItem::ValidationRequest(request) => {
                    Self::process_validation_request(
                        request,
                        &processor,
                        &config,
                        &status_map,
                        &throttle_semaphore,
                    )
                    .await;
                }
                QueueItem::Shutdown => {
                    info!("Received shutdown signal, stopping worker");
                    break;
                }
                _ => {
                    warn!("Received unexpected queue item in worker");
                }
            }
        }

        info!("Processing queue worker stopped");
    }

    async fn process_validation_request(
        request: ValidationRequest,
        processor: &ValidationProcessor,
        config: &Config,
        status_map: &Arc<RwLock<HashMap<String, ProcessingRecord>>>,
        throttle_semaphore: &Arc<Semaphore>,
    ) {
        let processing_id = request.processing_id.clone();

        debug!("Starting processing for request: {}", processing_id);

        // Update status to in_progress
        {
            let mut status_map = status_map.write().await;
            if let Some(record) = status_map.get_mut(&processing_id) {
                record.start_processing();
            }
        }

        // Acquire throttle permit
        let _permit = throttle_semaphore
            .acquire()
            .await
            .expect("Semaphore closed");

        // Process with timeout
        let processing_timeout = config.processing_timeout();
        let result = timeout(
            processing_timeout,
            processor.validate_request(request.clone()),
        )
        .await;

        // Update status with result
        {
            let mut status_map = status_map.write().await;
            if let Some(record) = status_map.get_mut(&processing_id) {
                match result {
                    Ok(Ok(validation_result)) => {
                        let response = ValidationResponse {
                            processing_id: processing_id.clone(),
                            results: validation_result,
                        };
                        record.complete_with_result(response);
                        info!("Completed processing for request: {}", processing_id);
                    }
                    Ok(Err(e)) => {
                        error!("Processing failed for request {}: {}", processing_id, e);
                        record.fail();
                    }
                    Err(_) => {
                        warn!("Processing timed out for request: {}", processing_id);
                        record.fail();
                    }
                }
            }
        }

        // Add delay for throttling
        let throttle_interval = Duration::from_secs(60) / config.throttle_requests_per_minute;
        sleep(throttle_interval).await;
    }

    async fn cleanup_task(
        status_map: Arc<RwLock<HashMap<String, ProcessingRecord>>>,
        timeout: Duration,
    ) {
        info!("Cleanup task started");

        let mut cleanup_interval = tokio::time::interval(Duration::from_secs(300)); // Every 5 minutes

        loop {
            cleanup_interval.tick().await;

            let mut status_map = status_map.write().await;
            let initial_count = status_map.len();

            // Remove expired records
            status_map.retain(|id, record| {
                if record.is_expired(timeout) {
                    debug!("Removing expired record: {}", id);
                    false
                } else {
                    true
                }
            });

            let removed_count = initial_count - status_map.len();
            if removed_count > 0 {
                info!("Cleaned up {} expired records", removed_count);
            }
        }
    }

    pub async fn get_queue_stats(&self) -> QueueStats {
        let status_map = self.status_map.read().await;

        let mut stats = QueueStats::default();

        for record in status_map.values() {
            match record.status {
                ProcessingStatus::Accepted => stats.accepted += 1,
                ProcessingStatus::InProgress => stats.in_progress += 1,
                ProcessingStatus::Completed => stats.completed += 1,
                ProcessingStatus::Failed => stats.failed += 1,
                ProcessingStatus::NotFound => {} // Should not happen in the map
            }
        }

        stats.total = status_map.len();
        stats.available_permits = self.throttle_semaphore.available_permits();

        stats
    }
}

#[derive(Debug, Default, serde::Serialize)]
pub struct QueueStats {
    pub total: usize,
    pub accepted: usize,
    pub in_progress: usize,
    pub completed: usize,
    pub failed: usize,
    pub available_permits: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("Queue is full")]
    QueueFull,
    #[error("Queue is closed")]
    QueueClosed,
    #[error("Request not found")]
    NotFound,
    #[error("Internal error: {0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Resolution, ValidationResults};

    #[tokio::test]
    async fn test_processing_record_lifecycle() {
        let mut record = ProcessingRecord::new();

        assert_eq!(record.status, ProcessingStatus::Accepted);
        assert!(record.started_at.is_none());

        record.start_processing();
        assert_eq!(record.status, ProcessingStatus::InProgress);
        assert!(record.started_at.is_some());

        let response = ValidationResponse {
            processing_id: "test".to_string(),
            results: ValidationResults {
                resolution: Resolution::Accepted,
                reasons: None,
            },
        };

        record.complete_with_result(response);
        assert_eq!(record.status, ProcessingStatus::Completed);
        assert!(record.completed_at.is_some());
        assert!(record.result.is_some());
    }

    #[tokio::test]
    async fn test_processing_record_expiration() {
        let record = ProcessingRecord::new();

        // Should not be expired with long timeout
        assert!(!record.is_expired(Duration::from_secs(3600)));

        // Should be expired with very short timeout
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(record.is_expired(Duration::from_millis(1)));
    }

    // Integration tests with full queue processing should be in tests/ directory
    // as they require more complex setup and coordination
}
