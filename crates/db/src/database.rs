//! Database API with migrations and basic operations.

use crate::backend;
use caliberate_core::config::{DbConfig, FtsConfig};
use caliberate_core::error::{CoreError, CoreResult};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use tracing::info;

const SCHEMA_VERSION: i64 = 5;

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
pub struct SeriesEntry {
    pub name: String,
    pub index: f64,
}

#[derive(Debug, Clone)]
pub struct IdentifierEntry {
    pub id_type: String,
    pub value: String,
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

#[derive(Debug, Clone)]
pub struct BookExtras {
    pub sort: Option<String>,
    pub timestamp: Option<String>,
    pub pubdate: Option<String>,
    pub author_sort: Option<String>,
    pub uuid: Option<String>,
    pub has_cover: bool,
    pub last_modified: Option<String>,
    pub publisher: Option<String>,
    pub rating: Option<i64>,
    pub languages: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct NoteRecord {
    pub id: i64,
    pub book_id: i64,
    pub text: String,
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
                CREATE TABLE IF NOT EXISTS authors (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL UNIQUE
                );
                CREATE TABLE IF NOT EXISTS books_authors_link (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book_id INTEGER NOT NULL,
                    author_id INTEGER NOT NULL,
                    FOREIGN KEY(book_id) REFERENCES books(id),
                    FOREIGN KEY(author_id) REFERENCES authors(id)
                );
                CREATE INDEX IF NOT EXISTS idx_books_authors_book_id ON books_authors_link(book_id);
                CREATE INDEX IF NOT EXISTS idx_books_authors_author_id ON books_authors_link(author_id);
                CREATE TABLE IF NOT EXISTS tags (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL UNIQUE
                );
                CREATE TABLE IF NOT EXISTS books_tags_link (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book_id INTEGER NOT NULL,
                    tag_id INTEGER NOT NULL,
                    FOREIGN KEY(book_id) REFERENCES books(id),
                    FOREIGN KEY(tag_id) REFERENCES tags(id)
                );
                CREATE INDEX IF NOT EXISTS idx_books_tags_book_id ON books_tags_link(book_id);
                CREATE INDEX IF NOT EXISTS idx_books_tags_tag_id ON books_tags_link(tag_id);
                CREATE TABLE IF NOT EXISTS series (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL UNIQUE
                );
                CREATE TABLE IF NOT EXISTS books_series_link (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book_id INTEGER NOT NULL,
                    series_id INTEGER NOT NULL,
                    series_index REAL NOT NULL DEFAULT 0,
                    FOREIGN KEY(book_id) REFERENCES books(id),
                    FOREIGN KEY(series_id) REFERENCES series(id)
                );
                CREATE INDEX IF NOT EXISTS idx_books_series_book_id ON books_series_link(book_id);
                CREATE INDEX IF NOT EXISTS idx_books_series_series_id ON books_series_link(series_id);
                CREATE TABLE IF NOT EXISTS identifiers (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book_id INTEGER NOT NULL,
                    identifier_type TEXT NOT NULL,
                    identifier_value TEXT NOT NULL,
                    FOREIGN KEY(book_id) REFERENCES books(id)
                );
                CREATE INDEX IF NOT EXISTS idx_identifiers_book_id ON identifiers(book_id);
                CREATE INDEX IF NOT EXISTS idx_identifiers_type ON identifiers(identifier_type);
                CREATE UNIQUE INDEX IF NOT EXISTS idx_identifiers_unique ON identifiers(book_id, identifier_type);
                CREATE TABLE IF NOT EXISTS comments (
                    book_id INTEGER PRIMARY KEY,
                    text TEXT NOT NULL,
                    FOREIGN KEY(book_id) REFERENCES books(id)
                );
                CREATE TABLE IF NOT EXISTS notes (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book_id INTEGER NOT NULL,
                    text TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY(book_id) REFERENCES books(id)
                );
                CREATE INDEX IF NOT EXISTS idx_notes_book_id ON notes(book_id);
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
                CREATE INDEX IF NOT EXISTS idx_assets_stored_path ON assets(stored_path);
                CREATE TABLE IF NOT EXISTS publishers (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL UNIQUE
                );
                CREATE TABLE IF NOT EXISTS books_publishers_link (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book_id INTEGER NOT NULL,
                    publisher_id INTEGER NOT NULL,
                    FOREIGN KEY(book_id) REFERENCES books(id),
                    FOREIGN KEY(publisher_id) REFERENCES publishers(id)
                );
                CREATE INDEX IF NOT EXISTS idx_books_publishers_book_id ON books_publishers_link(book_id);
                CREATE INDEX IF NOT EXISTS idx_books_publishers_publisher_id ON books_publishers_link(publisher_id);
                CREATE TABLE IF NOT EXISTS ratings (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    rating INTEGER NOT NULL UNIQUE
                );
                CREATE TABLE IF NOT EXISTS books_ratings_link (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book_id INTEGER NOT NULL,
                    rating_id INTEGER NOT NULL,
                    FOREIGN KEY(book_id) REFERENCES books(id),
                    FOREIGN KEY(rating_id) REFERENCES ratings(id)
                );
                CREATE INDEX IF NOT EXISTS idx_books_ratings_book_id ON books_ratings_link(book_id);
                CREATE INDEX IF NOT EXISTS idx_books_ratings_rating_id ON books_ratings_link(rating_id);
                CREATE TABLE IF NOT EXISTS languages (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    lang_code TEXT NOT NULL UNIQUE
                );
                CREATE TABLE IF NOT EXISTS books_languages_link (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book_id INTEGER NOT NULL,
                    language_id INTEGER NOT NULL,
                    item_order INTEGER NOT NULL DEFAULT 0,
                    FOREIGN KEY(book_id) REFERENCES books(id),
                    FOREIGN KEY(language_id) REFERENCES languages(id)
                );
                CREATE INDEX IF NOT EXISTS idx_books_languages_book_id ON books_languages_link(book_id);
                CREATE INDEX IF NOT EXISTS idx_books_languages_language_id ON books_languages_link(language_id);",
            )
            .map_err(|err| {
                CoreError::Io(
                    "create books table".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        self.ensure_book_columns()?;
        if self.fts.enabled {
            self.ensure_fts_schema()?;
        }
        Ok(())
    }

    fn ensure_book_columns(&self) -> CoreResult<()> {
        let columns = [
            ("sort", "ALTER TABLE books ADD COLUMN sort TEXT"),
            ("timestamp", "ALTER TABLE books ADD COLUMN timestamp TEXT"),
            ("pubdate", "ALTER TABLE books ADD COLUMN pubdate TEXT"),
            (
                "author_sort",
                "ALTER TABLE books ADD COLUMN author_sort TEXT",
            ),
            ("uuid", "ALTER TABLE books ADD COLUMN uuid TEXT"),
            (
                "has_cover",
                "ALTER TABLE books ADD COLUMN has_cover INTEGER NOT NULL DEFAULT 0",
            ),
            (
                "last_modified",
                "ALTER TABLE books ADD COLUMN last_modified TEXT",
            ),
        ];
        let mut stmt = self
            .conn
            .prepare("PRAGMA table_info(books)")
            .map_err(|err| {
                CoreError::Io(
                    "read books schema".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let existing = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|err| {
                CoreError::Io(
                    "read books columns".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?
            .collect::<Result<std::collections::BTreeSet<_>, _>>()
            .map_err(|err| {
                CoreError::Io(
                    "read books columns".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        for (column, ddl) in columns {
            if !existing.contains(column) {
                self.conn.execute(ddl, []).map_err(|err| {
                    CoreError::Io(
                        format!("add books column {column}"),
                        std::io::Error::new(std::io::ErrorKind::Other, err),
                    )
                })?;
            }
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

    pub fn update_book_title(&mut self, book_id: i64, title: &str) -> CoreResult<bool> {
        let updated = self
            .conn
            .execute(
                "UPDATE books SET title = ?1 WHERE id = ?2",
                params![title, book_id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "update book title".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(updated > 0)
    }

    pub fn update_book_sort(&mut self, book_id: i64, sort: &str) -> CoreResult<bool> {
        let updated = self
            .conn
            .execute(
                "UPDATE books SET sort = ?1 WHERE id = ?2",
                params![sort, book_id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "update book sort".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(updated > 0)
    }

    pub fn update_book_author_sort(&mut self, book_id: i64, author_sort: &str) -> CoreResult<bool> {
        let updated = self
            .conn
            .execute(
                "UPDATE books SET author_sort = ?1 WHERE id = ?2",
                params![author_sort, book_id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "update book author sort".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(updated > 0)
    }

    pub fn update_book_timestamp(&mut self, book_id: i64, timestamp: &str) -> CoreResult<bool> {
        let updated = self
            .conn
            .execute(
                "UPDATE books SET timestamp = ?1 WHERE id = ?2",
                params![timestamp, book_id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "update book timestamp".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(updated > 0)
    }

    pub fn update_book_pubdate(&mut self, book_id: i64, pubdate: &str) -> CoreResult<bool> {
        let updated = self
            .conn
            .execute(
                "UPDATE books SET pubdate = ?1 WHERE id = ?2",
                params![pubdate, book_id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "update book pubdate".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(updated > 0)
    }

    pub fn update_book_last_modified(
        &mut self,
        book_id: i64,
        last_modified: &str,
    ) -> CoreResult<bool> {
        let updated = self
            .conn
            .execute(
                "UPDATE books SET last_modified = ?1 WHERE id = ?2",
                params![last_modified, book_id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "update book last modified".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(updated > 0)
    }

    pub fn update_book_uuid(&mut self, book_id: i64, uuid: &str) -> CoreResult<bool> {
        let updated = self
            .conn
            .execute(
                "UPDATE books SET uuid = ?1 WHERE id = ?2",
                params![uuid, book_id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "update book uuid".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(updated > 0)
    }

    pub fn update_book_has_cover(&mut self, book_id: i64, has_cover: bool) -> CoreResult<bool> {
        let updated = self
            .conn
            .execute(
                "UPDATE books SET has_cover = ?1 WHERE id = ?2",
                params![if has_cover { 1 } else { 0 }, book_id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "update book cover flag".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(updated > 0)
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
            .prepare(
                "SELECT DISTINCT b.id, b.title, b.format, b.path
                 FROM books b
                 LEFT JOIN books_authors_link bal ON bal.book_id = b.id
                 LEFT JOIN authors a ON a.id = bal.author_id
                 LEFT JOIN books_tags_link btl ON btl.book_id = b.id
                 LEFT JOIN tags t ON t.id = btl.tag_id
                 LEFT JOIN books_series_link bsl ON bsl.book_id = b.id
                 LEFT JOIN series s ON s.id = bsl.series_id
                 WHERE b.title LIKE ?1
                    OR a.name LIKE ?1
                    OR t.name LIKE ?1
                    OR s.name LIKE ?1
                 ORDER BY b.id",
            )
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

    pub fn add_book_authors(&mut self, book_id: i64, authors: &[String]) -> CoreResult<()> {
        if authors.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction().map_err(|err| {
            CoreError::Io(
                "begin author link transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        for author in authors {
            let id = tx
                .execute(
                    "INSERT OR IGNORE INTO authors (name) VALUES (?1)",
                    params![author],
                )
                .map_err(|err| {
                    CoreError::Io(
                        "insert author".to_string(),
                        std::io::Error::new(std::io::ErrorKind::Other, err),
                    )
                })?;
            let author_id: i64 = if id == 0 {
                tx.query_row(
                    "SELECT id FROM authors WHERE name = ?1",
                    params![author],
                    |row| row.get(0),
                )
                .map_err(|err| {
                    CoreError::Io(
                        "lookup author".to_string(),
                        std::io::Error::new(std::io::ErrorKind::Other, err),
                    )
                })?
            } else {
                tx.last_insert_rowid()
            };
            tx.execute(
                "INSERT OR IGNORE INTO books_authors_link (book_id, author_id) VALUES (?1, ?2)",
                params![book_id, author_id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "insert book author link".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        }
        tx.commit().map_err(|err| {
            CoreError::Io(
                "commit author link transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        Ok(())
    }

    pub fn replace_book_authors(&mut self, book_id: i64, authors: &[String]) -> CoreResult<()> {
        let tx = self.conn.transaction().map_err(|err| {
            CoreError::Io(
                "begin author replace transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        tx.execute(
            "DELETE FROM books_authors_link WHERE book_id = ?1",
            params![book_id],
        )
        .map_err(|err| {
            CoreError::Io(
                "clear book authors".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        let mut unique = std::collections::BTreeSet::new();
        for author in authors
            .iter()
            .map(|value| value.trim())
            .filter(|v| !v.is_empty())
        {
            unique.insert(author.to_string());
        }
        for author in unique {
            let inserted = tx
                .execute(
                    "INSERT OR IGNORE INTO authors (name) VALUES (?1)",
                    params![author],
                )
                .map_err(|err| {
                    CoreError::Io(
                        "insert author".to_string(),
                        std::io::Error::new(std::io::ErrorKind::Other, err),
                    )
                })?;
            let author_id: i64 = if inserted == 0 {
                tx.query_row(
                    "SELECT id FROM authors WHERE name = ?1",
                    params![author],
                    |row| row.get(0),
                )
                .map_err(|err| {
                    CoreError::Io(
                        "lookup author".to_string(),
                        std::io::Error::new(std::io::ErrorKind::Other, err),
                    )
                })?
            } else {
                tx.last_insert_rowid()
            };
            tx.execute(
                "INSERT OR IGNORE INTO books_authors_link (book_id, author_id) VALUES (?1, ?2)",
                params![book_id, author_id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "insert book author link".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        }
        tx.commit().map_err(|err| {
            CoreError::Io(
                "commit author replace transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        Ok(())
    }

    pub fn list_book_authors(&self, book_id: i64) -> CoreResult<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT a.name
                 FROM authors a
                 JOIN books_authors_link bal ON bal.author_id = a.id
                 WHERE bal.book_id = ?1
                 ORDER BY a.name",
            )
            .map_err(|err| {
                CoreError::Io(
                    "prepare list authors".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let rows = stmt
            .query_map(params![book_id], |row| row.get(0))
            .map_err(|err| {
                CoreError::Io(
                    "query list authors".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let mut authors = Vec::new();
        for row in rows {
            authors.push(row.map_err(|err| {
                CoreError::Io(
                    "read list authors".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?);
        }
        Ok(authors)
    }

    pub fn upsert_tag(&self, name: &str) -> CoreResult<i64> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
                params![name],
            )
            .map_err(|err| {
                CoreError::Io(
                    "insert tag".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        self.conn
            .query_row(
                "SELECT id FROM tags WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .map_err(|err| {
                CoreError::Io(
                    "lookup tag".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })
    }

    pub fn add_book_tags(&mut self, book_id: i64, tags: &[String]) -> CoreResult<()> {
        if tags.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction().map_err(|err| {
            CoreError::Io(
                "begin tag link transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        for tag in tags {
            let id = tx
                .execute(
                    "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
                    params![tag],
                )
                .map_err(|err| {
                    CoreError::Io(
                        "insert tag".to_string(),
                        std::io::Error::new(std::io::ErrorKind::Other, err),
                    )
                })?;
            let tag_id: i64 = if id == 0 {
                tx.query_row("SELECT id FROM tags WHERE name = ?1", params![tag], |row| {
                    row.get(0)
                })
                .map_err(|err| {
                    CoreError::Io(
                        "lookup tag".to_string(),
                        std::io::Error::new(std::io::ErrorKind::Other, err),
                    )
                })?
            } else {
                tx.last_insert_rowid()
            };
            tx.execute(
                "INSERT OR IGNORE INTO books_tags_link (book_id, tag_id) VALUES (?1, ?2)",
                params![book_id, tag_id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "insert book tag link".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        }
        tx.commit().map_err(|err| {
            CoreError::Io(
                "commit tag link transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        Ok(())
    }

    pub fn replace_book_tags(&mut self, book_id: i64, tags: &[String]) -> CoreResult<()> {
        let tx = self.conn.transaction().map_err(|err| {
            CoreError::Io(
                "begin tag replace transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        tx.execute(
            "DELETE FROM books_tags_link WHERE book_id = ?1",
            params![book_id],
        )
        .map_err(|err| {
            CoreError::Io(
                "clear book tags".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        let mut unique = std::collections::BTreeSet::new();
        for tag in tags
            .iter()
            .map(|value| value.trim())
            .filter(|v| !v.is_empty())
        {
            unique.insert(tag.to_string());
        }
        for tag in unique {
            let inserted = tx
                .execute(
                    "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
                    params![tag],
                )
                .map_err(|err| {
                    CoreError::Io(
                        "insert tag".to_string(),
                        std::io::Error::new(std::io::ErrorKind::Other, err),
                    )
                })?;
            let tag_id: i64 = if inserted == 0 {
                tx.query_row("SELECT id FROM tags WHERE name = ?1", params![tag], |row| {
                    row.get(0)
                })
                .map_err(|err| {
                    CoreError::Io(
                        "lookup tag".to_string(),
                        std::io::Error::new(std::io::ErrorKind::Other, err),
                    )
                })?
            } else {
                tx.last_insert_rowid()
            };
            tx.execute(
                "INSERT OR IGNORE INTO books_tags_link (book_id, tag_id) VALUES (?1, ?2)",
                params![book_id, tag_id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "insert book tag link".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        }
        tx.commit().map_err(|err| {
            CoreError::Io(
                "commit tag replace transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        Ok(())
    }

    pub fn list_book_tags(&self, book_id: i64) -> CoreResult<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT t.name
                 FROM tags t
                 JOIN books_tags_link btl ON btl.tag_id = t.id
                 WHERE btl.book_id = ?1
                 ORDER BY t.name",
            )
            .map_err(|err| {
                CoreError::Io(
                    "prepare list tags".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let rows = stmt
            .query_map(params![book_id], |row| row.get(0))
            .map_err(|err| {
                CoreError::Io(
                    "query list tags".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let mut tags = Vec::new();
        for row in rows {
            tags.push(row.map_err(|err| {
                CoreError::Io(
                    "read list tags".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?);
        }
        Ok(tags)
    }

    pub fn upsert_series(&self, name: &str) -> CoreResult<i64> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO series (name) VALUES (?1)",
                params![name],
            )
            .map_err(|err| {
                CoreError::Io(
                    "insert series".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        self.conn
            .query_row(
                "SELECT id FROM series WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .map_err(|err| {
                CoreError::Io(
                    "lookup series".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })
    }

    pub fn set_book_series(&mut self, book_id: i64, name: &str, index: f64) -> CoreResult<()> {
        let tx = self.conn.transaction().map_err(|err| {
            CoreError::Io(
                "begin series link transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        let series_id = {
            let inserted = tx
                .execute(
                    "INSERT OR IGNORE INTO series (name) VALUES (?1)",
                    params![name],
                )
                .map_err(|err| {
                    CoreError::Io(
                        "insert series".to_string(),
                        std::io::Error::new(std::io::ErrorKind::Other, err),
                    )
                })?;
            if inserted == 0 {
                tx.query_row(
                    "SELECT id FROM series WHERE name = ?1",
                    params![name],
                    |row| row.get(0),
                )
                .map_err(|err| {
                    CoreError::Io(
                        "lookup series".to_string(),
                        std::io::Error::new(std::io::ErrorKind::Other, err),
                    )
                })?
            } else {
                tx.last_insert_rowid()
            }
        };
        tx.execute(
            "DELETE FROM books_series_link WHERE book_id = ?1",
            params![book_id],
        )
        .map_err(|err| {
            CoreError::Io(
                "clear book series link".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        tx.execute(
            "INSERT INTO books_series_link (book_id, series_id, series_index) VALUES (?1, ?2, ?3)",
            params![book_id, series_id, index],
        )
        .map_err(|err| {
            CoreError::Io(
                "insert book series link".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        tx.commit().map_err(|err| {
            CoreError::Io(
                "commit series link transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        Ok(())
    }

    pub fn clear_book_series(&mut self, book_id: i64) -> CoreResult<()> {
        self.conn
            .execute(
                "DELETE FROM books_series_link WHERE book_id = ?1",
                params![book_id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "clear book series".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    pub fn get_book_series(&self, book_id: i64) -> CoreResult<Option<SeriesEntry>> {
        self.conn
            .query_row(
                "SELECT s.name, bsl.series_index
                 FROM books_series_link bsl
                 JOIN series s ON s.id = bsl.series_id
                 WHERE bsl.book_id = ?1",
                params![book_id],
                |row| {
                    Ok(SeriesEntry {
                        name: row.get(0)?,
                        index: row.get(1)?,
                    })
                },
            )
            .optional()
            .map_err(|err| {
                CoreError::Io(
                    "query book series".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })
    }

    pub fn add_book_identifiers(
        &mut self,
        book_id: i64,
        identifiers: &[(String, String)],
    ) -> CoreResult<()> {
        if identifiers.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction().map_err(|err| {
            CoreError::Io(
                "begin identifier transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        for (id_type, value) in identifiers {
            tx.execute(
                "INSERT OR REPLACE INTO identifiers (book_id, identifier_type, identifier_value)
                 VALUES (?1, ?2, ?3)",
                params![book_id, id_type, value],
            )
            .map_err(|err| {
                CoreError::Io(
                    "insert identifier".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        }
        tx.commit().map_err(|err| {
            CoreError::Io(
                "commit identifier transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        Ok(())
    }

    pub fn replace_book_identifiers(
        &mut self,
        book_id: i64,
        identifiers: &[(String, String)],
    ) -> CoreResult<()> {
        let tx = self.conn.transaction().map_err(|err| {
            CoreError::Io(
                "begin identifier replace transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        tx.execute(
            "DELETE FROM identifiers WHERE book_id = ?1",
            params![book_id],
        )
        .map_err(|err| {
            CoreError::Io(
                "clear identifiers".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        for (id_type, value) in identifiers {
            if id_type.trim().is_empty() || value.trim().is_empty() {
                continue;
            }
            tx.execute(
                "INSERT OR REPLACE INTO identifiers (book_id, identifier_type, identifier_value)\n                 VALUES (?1, ?2, ?3)",
                params![book_id, id_type, value],
            )
            .map_err(|err| {
                CoreError::Io(
                    "insert identifier".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        }
        tx.commit().map_err(|err| {
            CoreError::Io(
                "commit identifier replace transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        Ok(())
    }

    pub fn list_book_identifiers(&self, book_id: i64) -> CoreResult<Vec<IdentifierEntry>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT identifier_type, identifier_value
                 FROM identifiers
                 WHERE book_id = ?1
                 ORDER BY identifier_type",
            )
            .map_err(|err| {
                CoreError::Io(
                    "prepare list identifiers".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let rows = stmt
            .query_map(params![book_id], |row| {
                Ok(IdentifierEntry {
                    id_type: row.get(0)?,
                    value: row.get(1)?,
                })
            })
            .map_err(|err| {
                CoreError::Io(
                    "query list identifiers".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let mut identifiers = Vec::new();
        for row in rows {
            identifiers.push(row.map_err(|err| {
                CoreError::Io(
                    "read list identifiers".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?);
        }
        Ok(identifiers)
    }

    pub fn set_book_comment(&self, book_id: i64, comment: &str) -> CoreResult<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO comments (book_id, text) VALUES (?1, ?2)",
                params![book_id, comment],
            )
            .map_err(|err| {
                CoreError::Io(
                    "insert comment".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    pub fn get_book_comment(&self, book_id: i64) -> CoreResult<Option<String>> {
        self.conn
            .query_row(
                "SELECT text FROM comments WHERE book_id = ?1",
                params![book_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| {
                CoreError::Io(
                    "query comment".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })
    }

    pub fn clear_book_comment(&mut self, book_id: i64) -> CoreResult<()> {
        self.conn
            .execute("DELETE FROM comments WHERE book_id = ?1", params![book_id])
            .map_err(|err| {
                CoreError::Io(
                    "clear comment".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    pub fn add_note(&mut self, book_id: i64, text: &str, created_at: &str) -> CoreResult<i64> {
        self.conn
            .execute(
                "INSERT INTO notes (book_id, text, created_at) VALUES (?1, ?2, ?3)",
                params![book_id, text, created_at],
            )
            .map_err(|err| {
                CoreError::Io(
                    "insert note".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_notes_for_book(&self, book_id: i64) -> CoreResult<Vec<NoteRecord>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, book_id, text, created_at FROM notes WHERE book_id = ?1 ORDER BY id",
            )
            .map_err(|err| {
                CoreError::Io(
                    "prepare list notes".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let rows = stmt
            .query_map(params![book_id], |row| {
                Ok(NoteRecord {
                    id: row.get(0)?,
                    book_id: row.get(1)?,
                    text: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .map_err(|err| {
                CoreError::Io(
                    "query list notes".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let mut notes = Vec::new();
        for row in rows {
            notes.push(row.map_err(|err| {
                CoreError::Io(
                    "read list notes".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?);
        }
        Ok(notes)
    }

    pub fn delete_note(&mut self, note_id: i64) -> CoreResult<bool> {
        let deleted = self
            .conn
            .execute("DELETE FROM notes WHERE id = ?1", params![note_id])
            .map_err(|err| {
                CoreError::Io(
                    "delete note".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(deleted > 0)
    }

    pub fn set_book_publisher(&mut self, book_id: i64, name: &str) -> CoreResult<()> {
        let tx = self.conn.transaction().map_err(|err| {
            CoreError::Io(
                "begin publisher transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        let inserted = tx
            .execute(
                "INSERT OR IGNORE INTO publishers (name) VALUES (?1)",
                params![name],
            )
            .map_err(|err| {
                CoreError::Io(
                    "insert publisher".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let publisher_id: i64 = if inserted == 0 {
            tx.query_row(
                "SELECT id FROM publishers WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .map_err(|err| {
                CoreError::Io(
                    "lookup publisher".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?
        } else {
            tx.last_insert_rowid()
        };
        tx.execute(
            "DELETE FROM books_publishers_link WHERE book_id = ?1",
            params![book_id],
        )
        .map_err(|err| {
            CoreError::Io(
                "clear book publisher".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        tx.execute(
            "INSERT OR IGNORE INTO books_publishers_link (book_id, publisher_id) VALUES (?1, ?2)",
            params![book_id, publisher_id],
        )
        .map_err(|err| {
            CoreError::Io(
                "insert book publisher".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        tx.commit().map_err(|err| {
            CoreError::Io(
                "commit publisher transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        Ok(())
    }

    pub fn clear_book_publisher(&mut self, book_id: i64) -> CoreResult<()> {
        self.conn
            .execute(
                "DELETE FROM books_publishers_link WHERE book_id = ?1",
                params![book_id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "clear book publisher".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    pub fn set_book_rating(&mut self, book_id: i64, rating: i64) -> CoreResult<()> {
        if !(0..=10).contains(&rating) {
            return Err(CoreError::ConfigValidate(
                "rating must be between 0 and 10".to_string(),
            ));
        }
        let tx = self.conn.transaction().map_err(|err| {
            CoreError::Io(
                "begin rating transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        if rating == 0 {
            tx.execute(
                "DELETE FROM books_ratings_link WHERE book_id = ?1",
                params![book_id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "clear book rating".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
            tx.commit().map_err(|err| {
                CoreError::Io(
                    "commit rating transaction".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
            return Ok(());
        }
        let inserted = tx
            .execute(
                "INSERT OR IGNORE INTO ratings (rating) VALUES (?1)",
                params![rating],
            )
            .map_err(|err| {
                CoreError::Io(
                    "insert rating".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let rating_id: i64 = if inserted == 0 {
            tx.query_row(
                "SELECT id FROM ratings WHERE rating = ?1",
                params![rating],
                |row| row.get(0),
            )
            .map_err(|err| {
                CoreError::Io(
                    "lookup rating".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?
        } else {
            tx.last_insert_rowid()
        };
        tx.execute(
            "DELETE FROM books_ratings_link WHERE book_id = ?1",
            params![book_id],
        )
        .map_err(|err| {
            CoreError::Io(
                "clear book rating".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        tx.execute(
            "INSERT OR IGNORE INTO books_ratings_link (book_id, rating_id) VALUES (?1, ?2)",
            params![book_id, rating_id],
        )
        .map_err(|err| {
            CoreError::Io(
                "insert book rating".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        tx.commit().map_err(|err| {
            CoreError::Io(
                "commit rating transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        Ok(())
    }

    pub fn set_book_languages(&mut self, book_id: i64, languages: &[String]) -> CoreResult<()> {
        let tx = self.conn.transaction().map_err(|err| {
            CoreError::Io(
                "begin language transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        tx.execute(
            "DELETE FROM books_languages_link WHERE book_id = ?1",
            params![book_id],
        )
        .map_err(|err| {
            CoreError::Io(
                "clear book languages".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        for (order, lang) in languages
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .enumerate()
        {
            let inserted = tx
                .execute(
                    "INSERT OR IGNORE INTO languages (lang_code) VALUES (?1)",
                    params![lang],
                )
                .map_err(|err| {
                    CoreError::Io(
                        "insert language".to_string(),
                        std::io::Error::new(std::io::ErrorKind::Other, err),
                    )
                })?;
            let language_id: i64 = if inserted == 0 {
                tx.query_row(
                    "SELECT id FROM languages WHERE lang_code = ?1",
                    params![lang],
                    |row| row.get(0),
                )
                .map_err(|err| {
                    CoreError::Io(
                        "lookup language".to_string(),
                        std::io::Error::new(std::io::ErrorKind::Other, err),
                    )
                })?
            } else {
                tx.last_insert_rowid()
            };
            tx.execute(
                "INSERT OR IGNORE INTO books_languages_link (book_id, language_id, item_order) VALUES (?1, ?2, ?3)",
                params![book_id, language_id, order as i64],
            )
            .map_err(|err| {
                CoreError::Io(
                    "insert book language".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        }
        tx.commit().map_err(|err| {
            CoreError::Io(
                "commit language transaction".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        Ok(())
    }

    pub fn get_book_extras(&self, book_id: i64) -> CoreResult<BookExtras> {
        let (sort, timestamp, pubdate, author_sort, uuid, has_cover, last_modified): (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            i64,
            Option<String>,
        ) = self
            .conn
            .query_row(
                "SELECT sort, timestamp, pubdate, author_sort, uuid, has_cover, last_modified
                 FROM books WHERE id = ?1",
                params![book_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .map_err(|err| {
                CoreError::Io(
                    "query book extras".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let publisher = self
            .conn
            .query_row(
                "SELECT p.name
                 FROM books_publishers_link bpl
                 JOIN publishers p ON p.id = bpl.publisher_id
                 WHERE bpl.book_id = ?1",
                params![book_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| {
                CoreError::Io(
                    "query publisher".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let rating = self
            .conn
            .query_row(
                "SELECT r.rating
                 FROM books_ratings_link brl
                 JOIN ratings r ON r.id = brl.rating_id
                 WHERE brl.book_id = ?1",
                params![book_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| {
                CoreError::Io(
                    "query rating".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let mut stmt = self
            .conn
            .prepare(
                "SELECT l.lang_code
                 FROM books_languages_link bll
                 JOIN languages l ON l.id = bll.language_id
                 WHERE bll.book_id = ?1
                 ORDER BY bll.item_order",
            )
            .map_err(|err| {
                CoreError::Io(
                    "prepare languages".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let rows = stmt
            .query_map(params![book_id], |row| row.get(0))
            .map_err(|err| {
                CoreError::Io(
                    "query languages".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let mut languages = Vec::new();
        for row in rows {
            languages.push(row.map_err(|err| {
                CoreError::Io(
                    "read languages".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?);
        }

        Ok(BookExtras {
            sort,
            timestamp,
            pubdate,
            author_sort,
            uuid,
            has_cover: has_cover != 0,
            last_modified,
            publisher,
            rating,
            languages,
        })
    }
}
