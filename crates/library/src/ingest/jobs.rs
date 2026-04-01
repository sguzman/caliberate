//! Background ingest job queue.

use super::{IngestOutcome, IngestRequest, Ingestor};
use caliberate_core::config::{ControlPlane, IngestMode};
use caliberate_core::error::{CoreError, CoreResult};
use crossbeam_channel::{Receiver, Sender, bounded};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use tracing::{error, info};

pub type IngestJobId = u64;

#[derive(Debug, Clone)]
pub struct IngestJobHandle {
    pub id: IngestJobId,
}

#[derive(Debug, Clone)]
pub struct IngestJobRequest {
    pub id: IngestJobId,
    pub source_path: PathBuf,
    pub mode: Option<IngestMode>,
    pub archive_reference: bool,
}

#[derive(Debug, Clone)]
pub enum IngestJobStatus {
    Queued,
    Running,
    Completed(IngestOutcome),
    Failed(String),
}

pub struct IngestJobQueue {
    sender: Sender<IngestJobRequest>,
    status: Arc<Mutex<HashMap<IngestJobId, IngestJobStatus>>>,
    next_id: AtomicU64,
    _workers: Vec<JoinHandle<()>>,
}

impl IngestJobQueue {
    pub fn new(ingestor: Ingestor, worker_count: usize, queue_capacity: usize) -> Self {
        let (sender, receiver) = bounded(queue_capacity.max(1));
        let status = Arc::new(Mutex::new(HashMap::new()));
        let mut workers = Vec::with_capacity(worker_count.max(1));
        for worker_id in 0..worker_count.max(1) {
            let worker_ingestor = ingestor.clone();
            let worker_status = Arc::clone(&status);
            let worker_receiver = receiver.clone();
            workers.push(
                thread::Builder::new()
                    .name(format!("ingest-worker-{worker_id}"))
                    .spawn(move || {
                        worker_loop(worker_id, worker_ingestor, worker_receiver, worker_status)
                    })
                    .expect("spawn ingest worker"),
            );
        }

        Self {
            sender,
            status,
            next_id: AtomicU64::new(1),
            _workers: workers,
        }
    }

    pub fn from_config(ingestor: Ingestor, config: &ControlPlane) -> Option<Self> {
        if !config.ingest.background_enabled {
            return None;
        }
        Some(Self::new(
            ingestor,
            config.ingest.background_workers as usize,
            config.ingest.background_queue_capacity as usize,
        ))
    }

    pub fn enqueue(
        &self,
        source_path: PathBuf,
        mode: Option<IngestMode>,
        archive_reference: bool,
    ) -> CoreResult<IngestJobHandle> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = IngestJobRequest {
            id,
            source_path,
            mode,
            archive_reference,
        };
        {
            let mut guard = self
                .status
                .lock()
                .map_err(|_| CoreError::ConfigValidate("job status poisoned".to_string()))?;
            guard.insert(id, IngestJobStatus::Queued);
        }
        self.sender
            .send(request)
            .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
        info!(component = "ingest", job_id = id, "queued ingest job");
        Ok(IngestJobHandle { id })
    }

    pub fn status(&self, id: IngestJobId) -> Option<IngestJobStatus> {
        let guard = self.status.lock().ok()?;
        guard.get(&id).cloned()
    }
}

fn worker_loop(
    worker_id: usize,
    ingestor: Ingestor,
    receiver: Receiver<IngestJobRequest>,
    status: Arc<Mutex<HashMap<IngestJobId, IngestJobStatus>>>,
) {
    info!(component = "ingest", worker_id, "ingest worker started");
    while let Ok(request) = receiver.recv() {
        update_status(&status, request.id, IngestJobStatus::Running);
        info!(
            component = "ingest",
            worker_id,
            job_id = request.id,
            path = %request.source_path.display(),
            "processing ingest job"
        );
        let outcome = if request.archive_reference {
            ingestor.ingest_archive_reference(IngestRequest {
                source_path: &request.source_path,
                mode: request.mode,
            })
        } else {
            ingestor.ingest(IngestRequest {
                source_path: &request.source_path,
                mode: request.mode,
            })
        };
        match outcome {
            Ok(result) => {
                update_status(&status, request.id, IngestJobStatus::Completed(result));
            }
            Err(err) => {
                error!(
                    component = "ingest",
                    worker_id,
                    job_id = request.id,
                    error = %err,
                    "ingest job failed"
                );
                update_status(
                    &status,
                    request.id,
                    IngestJobStatus::Failed(err.to_string()),
                );
            }
        }
    }
    info!(
        component = "ingest",
        worker_id, "ingest worker shutting down"
    );
}

fn update_status(
    status: &Arc<Mutex<HashMap<IngestJobId, IngestJobStatus>>>,
    id: IngestJobId,
    next: IngestJobStatus,
) {
    if let Ok(mut guard) = status.lock() {
        guard.insert(id, next);
    }
}
