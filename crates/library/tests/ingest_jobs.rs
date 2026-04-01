use caliberate_assets::storage::LocalAssetStore;
use caliberate_core::config::ControlPlane;
use caliberate_library::ingest::jobs::{IngestJobQueue, IngestJobStatus};
use caliberate_library::ingest::{IngestOutcome, Ingestor};
use std::fs;
use std::time::{Duration, Instant};
use tempfile::tempdir;

#[test]
fn ingest_job_queue_completes() {
    let source_dir = tempdir().expect("source dir");
    let library_dir = tempdir().expect("library dir");

    let source_path = source_dir.path().join("queued.epub");
    fs::write(&source_path, b"queued data").expect("write source");

    let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/control-plane.toml");
    let mut config = ControlPlane::load_from_path(&config_path).expect("load config");
    config.paths.library_dir = library_dir.path().to_path_buf();
    config.assets.compress_raw_assets = false;

    let store = LocalAssetStore::from_config(&config);
    let ingestor = Ingestor::new(std::sync::Arc::new(store), config);
    let queue = IngestJobQueue::new(ingestor, 1, 4);

    let handle = queue
        .enqueue(source_path.clone(), None, false)
        .expect("enqueue job");

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if Instant::now() > deadline {
            panic!("job did not complete in time");
        }
        let status = queue.status(handle.id).expect("status");
        match status {
            IngestJobStatus::Completed(outcome) => {
                let IngestOutcome::Ingested(result) = outcome else {
                    panic!("job outcome was skipped");
                };
                assert!(result.asset.stored_path.exists());
                break;
            }
            IngestJobStatus::Failed(err) => {
                panic!("job failed: {err}");
            }
            _ => {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}
