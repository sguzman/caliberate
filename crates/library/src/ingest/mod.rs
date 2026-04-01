//! Ingest pipeline and import policies.

pub mod jobs;

use caliberate_assets::storage::{
    AssetRecord, AssetStore, DuplicateSkipReason, StorageMode, StoreOutcome,
};
use caliberate_core::config::{ControlPlane, IngestMode};
use caliberate_core::error::{CoreError, CoreResult};
use caliberate_metadata::extract::{
    ArchivePreview, BasicMetadata, extract_archive_entry_to_temp, extract_archive_preview,
    extract_basic,
};
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
    pub archive_preview: Option<ArchivePreview>,
}

#[derive(Debug, Clone)]
pub struct IngestSkip {
    pub metadata: BasicMetadata,
    pub archive_preview: Option<ArchivePreview>,
    pub existing_path: std::path::PathBuf,
    pub reason: DuplicateSkipReason,
}

#[derive(Debug, Clone)]
pub enum IngestOutcome {
    Ingested(IngestResult),
    Skipped(IngestSkip),
}

pub struct Ingestor {
    store: Arc<dyn AssetStore>,
    config: ControlPlane,
}

impl Clone for Ingestor {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
            config: self.config.clone(),
        }
    }
}

impl Ingestor {
    pub fn new(store: Arc<dyn AssetStore>, config: ControlPlane) -> Self {
        Self { store, config }
    }

    pub fn ingest(&self, request: IngestRequest<'_>) -> CoreResult<IngestOutcome> {
        let mode = request.mode.unwrap_or(self.config.ingest.default_mode);
        let storage_mode = match mode {
            IngestMode::Copy => StorageMode::Copy,
            IngestMode::Reference => StorageMode::Reference,
        };

        let metadata = extract_basic(request.source_path, &self.config.formats)?;
        let asset_outcome = self.store.store(request.source_path, storage_mode)?;

        info!(
            component = "ingest",
            mode = ?mode,
            title = %metadata.title,
            format = %metadata.format,
            "ingest complete"
        );

        match asset_outcome {
            StoreOutcome::Stored(asset) => Ok(IngestOutcome::Ingested(IngestResult {
                metadata,
                asset,
                archive_preview: None,
            })),
            StoreOutcome::Skipped(skip) => Ok(IngestOutcome::Skipped(IngestSkip {
                metadata,
                archive_preview: None,
                existing_path: skip.existing_path,
                reason: skip.reason,
            })),
        }
    }

    pub fn ingest_archive_reference(
        &self,
        request: IngestRequest<'_>,
    ) -> CoreResult<IngestOutcome> {
        if !self.config.ingest.archive_reference_enabled {
            return Err(CoreError::ConfigValidate(
                "archive reference ingestion disabled".to_string(),
            ));
        }

        let preview = extract_archive_preview(request.source_path, &self.config.formats)?;
        let outcome = self.ingest(request)?;
        Ok(match outcome {
            IngestOutcome::Ingested(mut result) => {
                result.archive_preview = Some(preview);
                IngestOutcome::Ingested(result)
            }
            IngestOutcome::Skipped(mut skip) => {
                skip.archive_preview = Some(preview);
                IngestOutcome::Skipped(skip)
            }
        })
    }

    pub fn extract_archive_on_demand(
        &self,
        archive_path: &Path,
        entry_name: &str,
    ) -> CoreResult<std::path::PathBuf> {
        if !self.config.ingest.archive_reference_enabled {
            return Err(CoreError::ConfigValidate(
                "archive reference ingestion disabled".to_string(),
            ));
        }
        extract_archive_entry_to_temp(
            archive_path,
            entry_name,
            &self.config.paths.tmp_dir,
            &self.config.formats,
        )
    }
}
