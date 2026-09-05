//! Content server and OPDS endpoints.

pub mod auth;
pub mod http;
pub mod opds;

use caliberate_core::config::ControlPlane;
use caliberate_core::error::CoreResult;
use caliberate_db::database::Database;
use caliberate_library::calibre::CalibreLibraryBackend;
use caliberate_library::catalog::LibraryCatalog;

#[derive(Clone)]
pub enum ServerLibrarySource {
    ConfiguredDatabase,
    AttachedCalibre(CalibreLibraryBackend),
}

#[derive(Clone)]
pub struct ServerState {
    pub config: ControlPlane,
    pub source: ServerLibrarySource,
}

impl ServerState {
    pub fn new(config: ControlPlane) -> Self {
        Self {
            config,
            source: ServerLibrarySource::ConfiguredDatabase,
        }
    }

    pub fn with_attached_calibre(config: ControlPlane, backend: CalibreLibraryBackend) -> Self {
        Self {
            config,
            source: ServerLibrarySource::AttachedCalibre(backend),
        }
    }

    pub fn with_catalog<T>(
        &self,
        operation: impl FnOnce(&LibraryCatalog<'_>) -> CoreResult<T>,
    ) -> CoreResult<T> {
        match &self.source {
            ServerLibrarySource::ConfiguredDatabase => {
                let db = Database::open_with_fts(&self.config.db, &self.config.fts)?;
                let catalog = LibraryCatalog::new(&db);
                operation(&catalog)
            }
            ServerLibrarySource::AttachedCalibre(backend) => {
                let catalog = LibraryCatalog::new(backend);
                operation(&catalog)
            }
        }
    }

    pub fn attached_calibre_root(&self) -> Option<&std::path::Path> {
        match &self.source {
            ServerLibrarySource::AttachedCalibre(backend) => Some(backend.library_root()),
            ServerLibrarySource::ConfiguredDatabase => None,
        }
    }

    pub fn source_label(&self) -> String {
        match &self.source {
            ServerLibrarySource::ConfiguredDatabase => "configured database".to_string(),
            ServerLibrarySource::AttachedCalibre(backend) => {
                format!(
                    "attached Calibre library root={}",
                    backend.library_root().display()
                )
            }
        }
    }
}

pub async fn run(config: &ControlPlane) -> CoreResult<()> {
    run_with_source(config, ServerLibrarySource::ConfiguredDatabase).await
}

pub async fn run_with_source(config: &ControlPlane, source: ServerLibrarySource) -> CoreResult<()> {
    http::run(ServerState {
        config: config.clone(),
        source,
    })
    .await
}
