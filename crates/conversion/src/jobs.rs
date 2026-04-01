//! Conversion job orchestration.

use crate::pipeline::{ConversionReport, convert_file};
use crate::settings::ConversionSettings;
use caliberate_core::error::{CoreError, CoreResult};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::info;

pub type ConversionJobId = u64;

#[derive(Debug, Clone)]
pub struct ConversionRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    pub settings: ConversionSettings,
}

#[derive(Debug, Clone)]
pub enum ConversionJobStatus {
    Queued,
    Running,
    Completed(ConversionReport),
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct ConversionJob {
    pub id: ConversionJobId,
    pub request: ConversionRequest,
}

#[derive(Debug, Clone)]
pub struct ConversionJobSummary {
    pub id: ConversionJobId,
    pub status: ConversionJobStatus,
    pub duration: Duration,
}

#[derive(Clone)]
pub struct ConversionJobRunner {
    next_id: Arc<AtomicU64>,
    statuses: Arc<Mutex<Vec<ConversionJobSummary>>>,
}

impl ConversionJobRunner {
    pub fn new() -> Self {
        Self {
            next_id: Arc::new(AtomicU64::new(1)),
            statuses: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn run(&self, request: ConversionRequest) -> CoreResult<ConversionJobSummary> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let started = Instant::now();
        info!(
            component = "conversion",
            job_id = id,
            "conversion job started"
        );
        let status = match convert_file(&request.input, &request.output, &request.settings) {
            Ok(report) => ConversionJobStatus::Completed(report),
            Err(err) => ConversionJobStatus::Failed(err.to_string()),
        };
        let summary = ConversionJobSummary {
            id,
            status: status.clone(),
            duration: started.elapsed(),
        };
        if let Ok(mut guard) = self.statuses.lock() {
            guard.push(summary.clone());
        }
        info!(
            component = "conversion",
            job_id = id,
            duration_ms = summary.duration.as_millis(),
            "conversion job finished"
        );
        match status {
            ConversionJobStatus::Failed(err) => Err(CoreError::ConfigValidate(err)),
            _ => Ok(summary),
        }
    }

    pub fn list(&self) -> Vec<ConversionJobSummary> {
        self.statuses
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

pub fn build_request(
    input: &Path,
    output: &Path,
    settings: ConversionSettings,
) -> ConversionRequest {
    ConversionRequest {
        input: input.to_path_buf(),
        output: output.to_path_buf(),
        settings,
    }
}
