//! Maintenance helpers for relocating catalog paths without bypassing Caliberate's SQLite setup.

use super::{Database, sqlite_error};
use caliberate_core::error::{CoreError, CoreResult};
use rusqlite::{Connection, params};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationalPathStatus {
    pub books: u64,
    pub assets: u64,
    pub sources: u64,
}

impl OperationalPathStatus {
    pub fn total(&self) -> u64 {
        self.books + self.assets + self.sources
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationalPathRebaseReport {
    pub before: OperationalPathStatus,
    pub after: OperationalPathStatus,
    pub books_updated: usize,
    pub assets_updated: usize,
    pub sources_updated: usize,
}

impl Database {
    pub fn operational_path_status(&self, marker: &str) -> CoreResult<OperationalPathStatus> {
        let needle = migration_needle(marker)?;
        status_for_connection(&self.conn, &needle)
    }

    pub fn rebase_operational_paths(
        &mut self,
        marker: &str,
        replacement_root: &Path,
    ) -> CoreResult<OperationalPathRebaseReport> {
        let needle = migration_needle(marker)?;
        let replacement_root = normalized_root(replacement_root)?;
        let before = status_for_connection(&self.conn, &needle)?;

        let tx = self
            .conn
            .transaction()
            .map_err(|err| sqlite_error("begin operational path rebase", err))?;

        let books_updated = tx
            .execute(
                "UPDATE books
                 SET path = ?1 || '/' ||
                     replace(substr(path, instr(path, ?2) + length(?2)), char(92), '/')
                 WHERE instr(path, ?2) > 0",
                params![replacement_root, needle],
            )
            .map_err(|err| sqlite_error("rebase book paths", err))?;

        let assets_updated = tx
            .execute(
                "UPDATE assets
                 SET stored_path = ?1 || '/' ||
                     replace(substr(stored_path, instr(stored_path, ?2) + length(?2)), char(92), '/')
                 WHERE instr(stored_path, ?2) > 0",
                params![replacement_root, needle],
            )
            .map_err(|err| sqlite_error("rebase asset paths", err))?;

        let sources_updated = tx
            .execute(
                "UPDATE library_sources
                 SET locator = ?1 || '/' ||
                     replace(substr(locator, instr(locator, ?2) + length(?2)), char(92), '/')
                 WHERE instr(locator, ?2) > 0",
                params![replacement_root, needle],
            )
            .map_err(|err| sqlite_error("rebase library source locators", err))?;

        let after = status_for_connection(&tx, &needle)?;
        if after.total() != 0 {
            return Err(CoreError::ConfigValidate(format!(
                "operational path rebase left {} old paths; transaction rolled back",
                after.total()
            )));
        }

        tx.commit()
            .map_err(|err| sqlite_error("commit operational path rebase", err))?;

        Ok(OperationalPathRebaseReport {
            before,
            after,
            books_updated,
            assets_updated,
            sources_updated,
        })
    }
}

fn migration_needle(marker: &str) -> CoreResult<String> {
    let marker = marker
        .trim()
        .trim_end_matches(|ch| ch == '/' || ch == '\\');
    if marker.is_empty() {
        return Err(CoreError::ConfigValidate(
            "path migration marker cannot be empty".to_string(),
        ));
    }
    Ok(format!("{marker}\\"))
}

fn normalized_root(path: &Path) -> CoreResult<String> {
    let value = path.to_string_lossy().replace('\\', "/");
    let value = value.trim_end_matches('/');
    if value.is_empty() {
        return Err(CoreError::ConfigValidate(
            "replacement root cannot be empty".to_string(),
        ));
    }
    Ok(value.to_string())
}

fn status_for_connection(conn: &Connection, needle: &str) -> CoreResult<OperationalPathStatus> {
    Ok(OperationalPathStatus {
        books: count_matching(
            conn,
            "SELECT COUNT(*) FROM books WHERE instr(path, ?1) > 0",
            needle,
            "count old book paths",
        )?,
        assets: count_matching(
            conn,
            "SELECT COUNT(*) FROM assets WHERE instr(stored_path, ?1) > 0",
            needle,
            "count old asset paths",
        )?,
        sources: count_matching(
            conn,
            "SELECT COUNT(*) FROM library_sources WHERE instr(locator, ?1) > 0",
            needle,
            "count old library source locators",
        )?,
    })
}

fn count_matching(
    conn: &Connection,
    sql: &str,
    needle: &str,
    label: &str,
) -> CoreResult<u64> {
    let count = conn
        .query_row(sql, params![needle], |row| row.get::<_, i64>(0))
        .map_err(|err| sqlite_error(label, err))?;
    u64::try_from(count)
        .map_err(|_| CoreError::ConfigValidate(format!("{label} returned a negative count")))
}
