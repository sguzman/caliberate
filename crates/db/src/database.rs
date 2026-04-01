//! Database API with migrations and basic operations.

use crate::backend;
use caliberate_core::config::{DbConfig, FtsConfig};
use caliberate_core::error::{CoreError, CoreResult};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use tracing::info;

const SCHEMA_VERSION: i64 = 3;

#[derive(Debug)]
pub struct Database {
    conn: Connection,
    fts: FtsConfig,
}

#[derive(Debug, Clone)]
pub struct BookRecord {
    pub id: i64,
    pub title: String,
    pub format: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct DeleteSummary {
    pub assets_deleted: usize,
    pub book_deleted: bool,
}

#[derive(Debug, Clone)]
pub struct AssetRow {
    pub id: i64,
    pub book_id: i64,
    pub storage_mode: String,
    pub stored_path: String,
    pub source_path: Option<String>,
    pub size_bytes: u64,
    pub stored_size_bytes: u64,
    pub checksum: Option<String>,
    pub is_compressed: bool,
    pub created_at: String,
}

impl Database {
    pub fn open(config: &DbConfig) -> CoreResult<Self> {
        Self::open_with_fts(config, &FtsConfig::default())
    }

    pub fn open_with_fts(config: &DbConfig, fts: &FtsConfig) -> CoreResult<Self> {
        let conn = backend::sqlite::open(config)?;
        let db = Self {
            conn,
            fts: fts.clone(),
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_path<P: AsRef<Path>>(path: P, busy_timeout_ms: u64) -> CoreResult<Self> {
        Self::open_path_with_fts(path, busy_timeout_ms, &FtsConfig::default())
    }

    pub fn open_path_with_fts<P: AsRef<Path>>(
        path: P,
        busy_timeout_ms: u64,
        fts: &FtsConfig,
    ) -> CoreResult<Self> {
        let conn = backend::sqlite::open_with_timeout(path, busy_timeout_ms)?;
        let db = Self {
            conn,
            fts: fts.clone(),
        };
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
            if self.fts.enabled && self.fts.rebuild_on_migrate {
                self.rebuild_fts()?;
            }
            info!(
                component = "db",
                version = SCHEMA_VERSION,
                "schema migrated"
            );
        }
        if self.fts.enabled {
            self.ensure_fts_schema()?;
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
                );
                CREATE TABLE IF NOT EXISTS assets (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book_id INTEGER NOT NULL,
                    storage_mode TEXT NOT NULL,
                    stored_path TEXT NOT NULL,
                    source_path TEXT,
                    size_bytes INTEGER NOT NULL,
                    stored_size_bytes INTEGER NOT NULL,
                    checksum TEXT,
                    is_compressed INTEGER NOT NULL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY(book_id) REFERENCES books(id)
                );
                CREATE INDEX IF NOT EXISTS idx_assets_book_id ON assets(book_id);
                CREATE INDEX IF NOT EXISTS idx_assets_stored_path ON assets(stored_path);",
            )
            .map_err(|err| {
                CoreError::Io(
                    "create books table".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        if self.fts.enabled {
            self.ensure_fts_schema()?;
        }
        Ok(())
    }

    pub fn add_book(
        &self,
        title: &str,
        format: &str,
        path: &str,
        created_at: &str,
    ) -> CoreResult<i64> {
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

    pub fn get_book(&self, id: i64) -> CoreResult<Option<BookRecord>> {
        self.conn
            .query_row(
                "SELECT id, title, format, path FROM books WHERE id = ?1",
                params![id],
                |row| {
                    Ok(BookRecord {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        format: row.get(2)?,
                        path: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(|err| {
                CoreError::Io(
                    "query book".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })
    }

    pub fn search_books(&self, query: &str) -> CoreResult<Vec<BookRecord>> {
        if self.fts.enabled && query.chars().count() >= self.fts.min_query_len {
            if let Ok(results) = self.search_books_fts(query) {
                return Ok(results);
            }
        }
        self.search_books_like(query)
    }

    pub fn search_books_like(&self, query: &str) -> CoreResult<Vec<BookRecord>> {
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

    pub fn search_books_fts(&self, query: &str) -> CoreResult<Vec<BookRecord>> {
        let limit = self.fts.result_limit as i64;
        let mut stmt = self
            .conn
            .prepare(
                "SELECT b.id, b.title, b.format, b.path
                 FROM books_fts f
                 JOIN books b ON b.id = f.rowid
                 WHERE books_fts MATCH ?1
                 LIMIT ?2",
            )
            .map_err(|err| {
                CoreError::Io(
                    "prepare fts search".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let rows = stmt
            .query_map(params![query, limit], |row| {
                Ok(BookRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    format: row.get(2)?,
                    path: row.get(3)?,
                })
            })
            .map_err(|err| {
                CoreError::Io(
                    "query fts search".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|err| {
                CoreError::Io(
                    "read fts search".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?);
        }
        Ok(results)
    }

    pub fn add_asset(
        &self,
        book_id: i64,
        storage_mode: &str,
        stored_path: &str,
        source_path: Option<&str>,
        size_bytes: u64,
        stored_size_bytes: u64,
        checksum: Option<&str>,
        is_compressed: bool,
        created_at: &str,
    ) -> CoreResult<i64> {
        self.conn
            .execute(
                "INSERT INTO assets (book_id, storage_mode, stored_path, source_path, size_bytes, stored_size_bytes, checksum, is_compressed, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    book_id,
                    storage_mode,
                    stored_path,
                    source_path,
                    size_bytes as i64,
                    stored_size_bytes as i64,
                    checksum,
                    if is_compressed { 1 } else { 0 },
                    created_at
                ],
            )
            .map_err(|err| {
                CoreError::Io(
                    "insert asset".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_assets(&self) -> CoreResult<Vec<AssetRow>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, book_id, storage_mode, stored_path, source_path, size_bytes, stored_size_bytes, checksum, is_compressed, created_at
                 FROM assets ORDER BY id",
            )
            .map_err(|err| {
                CoreError::Io(
                    "prepare list assets".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let rows = stmt
            .query_map([], |row| {
                let is_compressed: i64 = row.get(8)?;
                Ok(AssetRow {
                    id: row.get(0)?,
                    book_id: row.get(1)?,
                    storage_mode: row.get(2)?,
                    stored_path: row.get(3)?,
                    source_path: row.get(4)?,
                    size_bytes: row.get::<_, i64>(5)? as u64,
                    stored_size_bytes: row.get::<_, i64>(6)? as u64,
                    checksum: row.get(7)?,
                    is_compressed: is_compressed != 0,
                    created_at: row.get(9)?,
                })
            })
            .map_err(|err| {
                CoreError::Io(
                    "query list assets".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|err| {
                CoreError::Io(
                    "read list assets".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?);
        }
        Ok(results)
    }

    pub fn list_assets_for_book(&self, book_id: i64) -> CoreResult<Vec<AssetRow>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, book_id, storage_mode, stored_path, source_path, size_bytes, stored_size_bytes, checksum, is_compressed, created_at
                 FROM assets WHERE book_id = ?1 ORDER BY id",
            )
            .map_err(|err| {
                CoreError::Io(
                    "prepare list assets for book".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let rows = stmt
            .query_map(params![book_id], |row| {
                let is_compressed: i64 = row.get(8)?;
                Ok(AssetRow {
                    id: row.get(0)?,
                    book_id: row.get(1)?,
                    storage_mode: row.get(2)?,
                    stored_path: row.get(3)?,
                    source_path: row.get(4)?,
                    size_bytes: row.get::<_, i64>(5)? as u64,
                    stored_size_bytes: row.get::<_, i64>(6)? as u64,
                    checksum: row.get(7)?,
                    is_compressed: is_compressed != 0,
                    created_at: row.get(9)?,
                })
            })
            .map_err(|err| {
                CoreError::Io(
                    "query list assets for book".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|err| {
                CoreError::Io(
                    "read list assets for book".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?);
        }
        Ok(results)
    }

    pub fn delete_assets(&mut self, ids: &[i64]) -> CoreResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let tx = self.conn.transaction().map_err(|err| {
            CoreError::Io(
                "begin asset deletion transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        let mut deleted = 0;
        for id in ids {
            deleted += tx
                .execute("DELETE FROM assets WHERE id = ?1", params![id])
                .map_err(|err| {
                    CoreError::Io(
                        "delete asset".to_string(),
                        std::io::Error::new(std::io::ErrorKind::Other, err),
                    )
                })?;
        }
        tx.commit().map_err(|err| {
            CoreError::Io(
                "commit asset deletion".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        Ok(deleted)
    }

    pub fn delete_book_with_assets(&mut self, book_id: i64) -> CoreResult<DeleteSummary> {
        let tx = self.conn.transaction().map_err(|err| {
            CoreError::Io(
                "begin book deletion transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        let assets_deleted = tx
            .execute("DELETE FROM assets WHERE book_id = ?1", params![book_id])
            .map_err(|err| {
                CoreError::Io(
                    "delete book assets".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let book_deleted = tx
            .execute("DELETE FROM books WHERE id = ?1", params![book_id])
            .map_err(|err| {
                CoreError::Io(
                    "delete book".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?
            > 0;
        tx.commit().map_err(|err| {
            CoreError::Io(
                "commit book deletion".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        Ok(DeleteSummary {
            assets_deleted: assets_deleted as usize,
            book_deleted,
        })
    }

    pub fn ensure_fts_schema(&self) -> CoreResult<()> {
        if self.fts.tokenizer != "unicode61" {
            return Err(CoreError::ConfigValidate(
                "fts.tokenizer must be 'unicode61'".to_string(),
            ));
        }
        let ddl = format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS books_fts USING fts5(
                title,
                format,
                path,
                content='books',
                content_rowid='id',
                tokenize='{}'
            );
            CREATE TRIGGER IF NOT EXISTS books_ai AFTER INSERT ON books BEGIN
                INSERT INTO books_fts(rowid, title, format, path)
                VALUES (new.id, new.title, new.format, new.path);
            END;
            CREATE TRIGGER IF NOT EXISTS books_ad AFTER DELETE ON books BEGIN
                INSERT INTO books_fts(books_fts, rowid, title, format, path)
                VALUES('delete', old.id, old.title, old.format, old.path);
            END;
            CREATE TRIGGER IF NOT EXISTS books_au AFTER UPDATE ON books BEGIN
                INSERT INTO books_fts(books_fts, rowid, title, format, path)
                VALUES('delete', old.id, old.title, old.format, old.path);
                INSERT INTO books_fts(rowid, title, format, path)
                VALUES (new.id, new.title, new.format, new.path);
            END;",
            self.fts.tokenizer
        );
        self.conn.execute_batch(&ddl).map_err(|err| {
            CoreError::Io(
                "create fts schema".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        Ok(())
    }

    pub fn rebuild_fts(&self) -> CoreResult<()> {
        if !self.fts.enabled {
            return Err(CoreError::ConfigValidate("fts is disabled".to_string()));
        }
        self.ensure_fts_schema()?;
        self.conn
            .execute("INSERT INTO books_fts(books_fts) VALUES('rebuild')", [])
            .map_err(|err| {
                CoreError::Io(
                    "rebuild fts".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    pub fn fts_count(&self) -> CoreResult<i64> {
        let count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM books_fts", [], |row| row.get(0))
            .map_err(|err| {
                CoreError::Io(
                    "read fts count".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(count)
    }
}
