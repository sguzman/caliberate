//! Database API with migrations and basic operations.

use crate::backend;
use caliberate_core::config::DbConfig;
use caliberate_core::error::{CoreError, CoreResult};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use tracing::info;

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug)]
pub struct Database {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct BookRecord {
    pub id: i64,
    pub title: String,
    pub format: String,
    pub path: String,
}

impl Database {
    pub fn open(config: &DbConfig) -> CoreResult<Self> {
        let conn = backend::sqlite::open(config)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_path<P: AsRef<Path>>(path: P, busy_timeout_ms: u64) -> CoreResult<Self> {
        let conn = backend::sqlite::open_with_timeout(path, busy_timeout_ms)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn migrate(&self) -> CoreResult<()> {
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY);",
            )
            .map_err(|err| {
                CoreError::Io(
                    "create schema migrations table".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;

        let current_version: Option<i64> = self
            .conn
            .query_row(
                "SELECT version FROM schema_migrations ORDER BY version DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| {
                CoreError::Io(
                    "read schema version".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;

        if current_version.unwrap_or(0) < SCHEMA_VERSION {
            self.apply_schema()?;
            self.conn
                .execute(
                    "INSERT INTO schema_migrations (version) VALUES (?1)",
                    params![SCHEMA_VERSION],
                )
                .map_err(|err| {
                    CoreError::Io(
                        "write schema version".to_string(),
                        std::io::Error::new(std::io::ErrorKind::Other, err),
                    )
                })?;
            info!(component = "db", version = SCHEMA_VERSION, "schema migrated");
        }

        Ok(())
    }

    fn apply_schema(&self) -> CoreResult<()> {
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS books (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    title TEXT NOT NULL,
                    format TEXT NOT NULL,
                    path TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );",
            )
            .map_err(|err| {
                CoreError::Io(
                    "create books table".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    pub fn add_book(&self, title: &str, format: &str, path: &str, created_at: &str) -> CoreResult<i64> {
        self.conn
            .execute(
                "INSERT INTO books (title, format, path, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![title, format, path, created_at],
            )
            .map_err(|err| {
                CoreError::Io(
                    "insert book".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_books(&self) -> CoreResult<Vec<BookRecord>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, title, format, path FROM books ORDER BY id")
            .map_err(|err| {
                CoreError::Io(
                    "prepare list books".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let rows = stmt
            .query_map([], |row| {
                Ok(BookRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    format: row.get(2)?,
                    path: row.get(3)?,
                })
            })
            .map_err(|err| {
                CoreError::Io(
                    "query list books".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|err| {
                CoreError::Io(
                    "read list books".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?);
        }
        Ok(results)
    }

    pub fn search_books(&self, query: &str) -> CoreResult<Vec<BookRecord>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, title, format, path FROM books WHERE title LIKE ?1 ORDER BY id")
            .map_err(|err| {
                CoreError::Io(
                    "prepare search books".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let pattern = format!("%{}%", query);
        let rows = stmt
            .query_map([pattern], |row| {
                Ok(BookRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    format: row.get(2)?,
                    path: row.get(3)?,
                })
            })
            .map_err(|err| {
                CoreError::Io(
                    "query search books".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|err| {
                CoreError::Io(
                    "read search books".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?);
        }
        Ok(results)
    }
}
