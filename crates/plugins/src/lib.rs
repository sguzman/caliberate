//! Plugin discovery and execution.

pub mod discovery;
pub mod registry;
pub mod sandbox;

use std::path::Path;

pub trait PluginHook {
    fn on_ingest(&self, _path: &Path) {}
    fn on_metadata_update(&self, _book_id: i64) {}
}
