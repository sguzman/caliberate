//! Ingest pipeline and import policies.

use caliberate_core::config::{ControlPlane, IngestMode};
use caliberate_core::error::{CoreError, CoreResult};
use caliberate_metadata::extract::{extract_basic, BasicMetadata};
use caliberate_assets::storage::{AssetRecord, AssetStore, StorageMode};
use std::path::Path;
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Clone)]
pub struct IngestRequest<'a> {
    pub source_path: &'a Path,
    pub mode: Option<IngestMode>,
}

#[derive(Debug, Clone)]
pub struct IngestResult {
    pub metadata: BasicMetadata,
    pub asset: AssetRecord,
}

pub struct Ingestor {
    store: Arc<dyn AssetStore>,
    config: ControlPlane,
}

impl Ingestor {
    pub fn new(store: Arc<dyn AssetStore>, config: ControlPlane) -> Self {
        Self { store, config }
    }

    pub fn ingest(&self, request: IngestRequest<'_>) -> CoreResult<IngestResult> {
        let mode = request.mode.unwrap_or(self.config.ingest.default_mode);
        let storage_mode = match mode {
            IngestMode::Copy => StorageMode::Copy,
            IngestMode::Reference => StorageMode::Reference,
        };

        let metadata = extract_basic(request.source_path, &self.config.formats)?;
        let asset = self.store.store(request.source_path, storage_mode)?;

        info!(
            component = "ingest",
            mode = ?mode,
            title = %metadata.title,
            format = %metadata.format,
            "ingest complete"
        );

        Ok(IngestResult { metadata, asset })
    }

    pub fn ingest_archive_reference(&self, request: IngestRequest<'_>) -> CoreResult<IngestResult> {
        if !self.config.ingest.archive_reference_enabled {
            return Err(CoreError::ConfigValidate(
                "archive reference ingestion disabled".to_string(),
            ));
        }

        self.ingest(request)
    }
}
