//! Database API with migrations and basic operations.

use crate::backend;
use crate::query::BookQuery;
use caliberate_core::config::{DbConfig, FtsConfig};
use caliberate_core::error::{CoreError, CoreResult};
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde_json::Value as JsonValue;
use std::path::Path;
use tracing::info;

const SCHEMA_VERSION: i64 = 8;

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
pub struct BooksPagesEntry {
    pub book_id: i64,
    pub pages: i64,
    pub algorithm: i64,
    pub format: String,
    pub format_size: i64,
    pub timestamp: String,
    pub needs_scan: bool,
}

#[derive(Debug, Clone)]
pub struct NoteRecord {
    pub id: i64,
    pub book_id: i64,
    pub text: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct CategoryCount {
    pub id: i64,
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone)]
pub struct CustomColumn {
    pub id: i64,
    pub label: String,
    pub name: String,
    pub datatype: String,
    pub mark_for_delete: bool,
    pub editable: bool,
    pub display: String,
    pub is_multiple: bool,
    pub normalized: bool,
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
                    title TEXT NOT NULL DEFAULT 'Unknown' COLLATE NOCASE,
                    sort TEXT COLLATE NOCASE,
                    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    pubdate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    series_index REAL NOT NULL DEFAULT 1.0,
                    author_sort TEXT COLLATE NOCASE,
                    path TEXT NOT NULL DEFAULT '',
                    uuid TEXT,
                    has_cover BOOL DEFAULT 0,
                    last_modified TIMESTAMP NOT NULL DEFAULT '2000-01-01 00:00:00+00:00',
                    format TEXT NOT NULL DEFAULT '',
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE TABLE IF NOT EXISTS authors (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL COLLATE NOCASE,
                    sort TEXT COLLATE NOCASE,
                    link TEXT NOT NULL DEFAULT '',
                    UNIQUE(name)
                );
                CREATE TABLE IF NOT EXISTS books_authors_link (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    author INTEGER NOT NULL,
                    UNIQUE(book, author),
                    FOREIGN KEY(book) REFERENCES books(id),
                    FOREIGN KEY(author) REFERENCES authors(id)
                );
                CREATE INDEX IF NOT EXISTS idx_books_authors_book_id ON books_authors_link(book);
                CREATE INDEX IF NOT EXISTS idx_books_authors_author_id ON books_authors_link(author);
                CREATE TABLE IF NOT EXISTS tags (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL COLLATE NOCASE,
                    link TEXT NOT NULL DEFAULT '',
                    UNIQUE(name)
                );
                CREATE TABLE IF NOT EXISTS books_tags_link (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    tag INTEGER NOT NULL,
                    UNIQUE(book, tag),
                    FOREIGN KEY(book) REFERENCES books(id),
                    FOREIGN KEY(tag) REFERENCES tags(id)
                );
                CREATE INDEX IF NOT EXISTS idx_books_tags_book_id ON books_tags_link(book);
                CREATE INDEX IF NOT EXISTS idx_books_tags_tag_id ON books_tags_link(tag);
                CREATE TABLE IF NOT EXISTS series (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL COLLATE NOCASE,
                    sort TEXT COLLATE NOCASE,
                    link TEXT NOT NULL DEFAULT '',
                    UNIQUE(name)
                );
                CREATE TABLE IF NOT EXISTS books_series_link (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    series INTEGER NOT NULL,
                    UNIQUE(book),
                    FOREIGN KEY(book) REFERENCES books(id),
                    FOREIGN KEY(series) REFERENCES series(id)
                );
                CREATE INDEX IF NOT EXISTS idx_books_series_book_id ON books_series_link(book);
                CREATE INDEX IF NOT EXISTS idx_books_series_series_id ON books_series_link(series);
                CREATE TABLE IF NOT EXISTS identifiers (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    type TEXT NOT NULL DEFAULT 'isbn' COLLATE NOCASE,
                    val TEXT NOT NULL COLLATE NOCASE,
                    UNIQUE(book, type),
                    FOREIGN KEY(book) REFERENCES books(id)
                );
                CREATE INDEX IF NOT EXISTS idx_identifiers_book_id ON identifiers(book);
                CREATE INDEX IF NOT EXISTS idx_identifiers_type ON identifiers(type);
                CREATE TABLE IF NOT EXISTS comments (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    text TEXT NOT NULL COLLATE NOCASE,
                    UNIQUE(book),
                    FOREIGN KEY(book) REFERENCES books(id)
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
                    name TEXT NOT NULL COLLATE NOCASE,
                    sort TEXT COLLATE NOCASE,
                    link TEXT NOT NULL DEFAULT '',
                    UNIQUE(name)
                );
                CREATE TABLE IF NOT EXISTS books_publishers_link (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    publisher INTEGER NOT NULL,
                    UNIQUE(book),
                    FOREIGN KEY(book) REFERENCES books(id),
                    FOREIGN KEY(publisher) REFERENCES publishers(id)
                );
                CREATE INDEX IF NOT EXISTS idx_books_publishers_book_id ON books_publishers_link(book);
                CREATE INDEX IF NOT EXISTS idx_books_publishers_publisher_id ON books_publishers_link(publisher);
                CREATE TABLE IF NOT EXISTS ratings (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    rating INTEGER CHECK(rating > -1 AND rating < 11),
                    link TEXT NOT NULL DEFAULT '',
                    UNIQUE (rating)
                );
                CREATE TABLE IF NOT EXISTS books_ratings_link (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    rating INTEGER NOT NULL,
                    UNIQUE(book, rating),
                    FOREIGN KEY(book) REFERENCES books(id),
                    FOREIGN KEY(rating) REFERENCES ratings(id)
                );
                CREATE INDEX IF NOT EXISTS idx_books_ratings_book_id ON books_ratings_link(book);
                CREATE INDEX IF NOT EXISTS idx_books_ratings_rating_id ON books_ratings_link(rating);
                CREATE TABLE IF NOT EXISTS languages (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    lang_code TEXT NOT NULL COLLATE NOCASE,
                    link TEXT NOT NULL DEFAULT '',
                    UNIQUE(lang_code)
                );
                CREATE TABLE IF NOT EXISTS books_languages_link (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    lang_code INTEGER NOT NULL,
                    item_order INTEGER NOT NULL DEFAULT 0,
                    UNIQUE(book, lang_code),
                    FOREIGN KEY(book) REFERENCES books(id),
                    FOREIGN KEY(lang_code) REFERENCES languages(id)
                );
                CREATE INDEX IF NOT EXISTS idx_books_languages_book_id ON books_languages_link(book);
                CREATE INDEX IF NOT EXISTS idx_books_languages_language_id ON books_languages_link(lang_code);
                CREATE TABLE IF NOT EXISTS books_plugin_data (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    name TEXT NOT NULL,
                    val TEXT NOT NULL,
                    UNIQUE(book, name)
                );
                CREATE TABLE IF NOT EXISTS books_pages_link (
                    book INTEGER PRIMARY KEY,
                    pages INTEGER NOT NULL DEFAULT 0,
                    algorithm INTEGER NOT NULL DEFAULT 0,
                    format TEXT NOT NULL DEFAULT '' COLLATE NOCASE,
                    format_size INTEGER NOT NULL DEFAULT 0,
                    timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    needs_scan INTEGER NOT NULL DEFAULT 0 CHECK(needs_scan IN (0, 1)),
                    FOREIGN KEY(book) REFERENCES books(id) ON DELETE CASCADE
                );
                CREATE TRIGGER IF NOT EXISTS books_pages_link_create_trigger
                    AFTER INSERT ON books
                    FOR EACH ROW
                BEGIN
                    INSERT INTO books_pages_link(book) VALUES(NEW.id);
                END;
                CREATE INDEX IF NOT EXISTS idx_books_pages_link_needs_scan ON books_pages_link(needs_scan);
                CREATE TABLE IF NOT EXISTS conversion_options (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    format TEXT NOT NULL,
                    book INTEGER,
                    data BLOB NOT NULL,
                    UNIQUE(format, book)
                );
                CREATE TABLE IF NOT EXISTS custom_columns (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    label TEXT NOT NULL,
                    name TEXT NOT NULL,
                    datatype TEXT NOT NULL,
                    mark_for_delete INTEGER NOT NULL DEFAULT 0,
                    editable INTEGER NOT NULL DEFAULT 1,
                    display TEXT NOT NULL DEFAULT '{}',
                    is_multiple INTEGER NOT NULL DEFAULT 0,
                    normalized INTEGER NOT NULL,
                    UNIQUE(label)
                );
                CREATE TABLE IF NOT EXISTS data (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    format TEXT NOT NULL COLLATE NOCASE,
                    uncompressed_size INTEGER NOT NULL,
                    name TEXT NOT NULL,
                    UNIQUE(book, format)
                );
                CREATE TABLE IF NOT EXISTS feeds (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    title TEXT NOT NULL,
                    script TEXT NOT NULL,
                    UNIQUE(title)
                );
                CREATE TABLE IF NOT EXISTS library_id (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    uuid TEXT NOT NULL,
                    UNIQUE(uuid)
                );
                CREATE TABLE IF NOT EXISTS metadata_dirtied (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    UNIQUE(book)
                );
                CREATE TABLE IF NOT EXISTS annotations_dirtied (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    UNIQUE(book)
                );
                CREATE TABLE IF NOT EXISTS preferences (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    key TEXT NOT NULL,
                    val TEXT NOT NULL,
                    UNIQUE(key)
                );
                CREATE TABLE IF NOT EXISTS last_read_positions (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    format TEXT NOT NULL COLLATE NOCASE,
                    user TEXT NOT NULL,
                    device TEXT NOT NULL,
                    cfi TEXT NOT NULL,
                    epoch REAL NOT NULL,
                    pos_frac REAL NOT NULL DEFAULT 0,
                    UNIQUE(user, device, book, format)
                );
                CREATE TABLE IF NOT EXISTS annotations (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    format TEXT NOT NULL,
                    user_type TEXT NOT NULL,
                    user TEXT NOT NULL,
                    timestamp REAL NOT NULL,
                    annot_id TEXT NOT NULL,
                    annot_type TEXT NOT NULL,
                    annot_data TEXT NOT NULL,
                    searchable_text TEXT NOT NULL DEFAULT '',
                    UNIQUE(book, user_type, user, format, annot_type, annot_id)
                );
                CREATE VIRTUAL TABLE IF NOT EXISTS annotations_fts
                    USING fts5(searchable_text, content = 'annotations', content_rowid = 'id', tokenize = 'unicode61');
                CREATE VIRTUAL TABLE IF NOT EXISTS annotations_fts_stemmed
                    USING fts5(searchable_text, content = 'annotations', content_rowid = 'id', tokenize = 'porter unicode61');
                CREATE TRIGGER IF NOT EXISTS annotations_fts_insert_trg AFTER INSERT ON annotations
                BEGIN
                    INSERT INTO annotations_fts(rowid, searchable_text) VALUES (NEW.id, NEW.searchable_text);
                    INSERT INTO annotations_fts_stemmed(rowid, searchable_text) VALUES (NEW.id, NEW.searchable_text);
                END;
                CREATE TRIGGER IF NOT EXISTS annotations_fts_delete_trg AFTER DELETE ON annotations
                BEGIN
                    INSERT INTO annotations_fts(annotations_fts, rowid, searchable_text) VALUES('delete', OLD.id, OLD.searchable_text);
                    INSERT INTO annotations_fts_stemmed(annotations_fts_stemmed, rowid, searchable_text) VALUES('delete', OLD.id, OLD.searchable_text);
                END;
                CREATE TRIGGER IF NOT EXISTS annotations_fts_update_trg AFTER UPDATE ON annotations
                BEGIN
                    INSERT INTO annotations_fts(annotations_fts, rowid, searchable_text) VALUES('delete', OLD.id, OLD.searchable_text);
                    INSERT INTO annotations_fts(rowid, searchable_text) VALUES (NEW.id, NEW.searchable_text);
                    INSERT INTO annotations_fts_stemmed(annotations_fts_stemmed, rowid, searchable_text) VALUES('delete', OLD.id, OLD.searchable_text);
                    INSERT INTO annotations_fts_stemmed(rowid, searchable_text) VALUES (NEW.id, NEW.searchable_text);
                END;",
            )
            .map_err(|err| {
                CoreError::Io(
                    "create books table".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        self.ensure_book_columns()?;
        self.ensure_author_columns()?;
        self.ensure_publisher_columns()?;
        self.ensure_series_columns()?;
        self.ensure_tag_columns()?;
        self.ensure_language_columns()?;
        self.ensure_rating_columns()?;
        self.ensure_calibre_link_schema()?;
        self.ensure_calibre_collations()?;
        self.ensure_calibre_views()?;
        self.ensure_calibre_indices()?;
        self.ensure_calibre_triggers()?;
        self.ensure_calibre_pragmas()?;
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
                "series_index",
                "ALTER TABLE books ADD COLUMN series_index REAL NOT NULL DEFAULT 1.0",
            ),
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
        self.ensure_columns("books", &columns, "books")?;
        Ok(())
    }

    fn ensure_author_columns(&self) -> CoreResult<()> {
        let columns = [
            ("sort", "ALTER TABLE authors ADD COLUMN sort TEXT"),
            (
                "link",
                "ALTER TABLE authors ADD COLUMN link TEXT NOT NULL DEFAULT ''",
            ),
        ];
        self.ensure_columns("authors", &columns, "authors")?;
        Ok(())
    }

    fn ensure_publisher_columns(&self) -> CoreResult<()> {
        let columns = [
            ("sort", "ALTER TABLE publishers ADD COLUMN sort TEXT"),
            (
                "link",
                "ALTER TABLE publishers ADD COLUMN link TEXT NOT NULL DEFAULT ''",
            ),
        ];
        self.ensure_columns("publishers", &columns, "publishers")?;
        Ok(())
    }

    fn ensure_series_columns(&self) -> CoreResult<()> {
        let columns = [
            ("sort", "ALTER TABLE series ADD COLUMN sort TEXT"),
            (
                "link",
                "ALTER TABLE series ADD COLUMN link TEXT NOT NULL DEFAULT ''",
            ),
        ];
        self.ensure_columns("series", &columns, "series")?;
        Ok(())
    }

    fn ensure_tag_columns(&self) -> CoreResult<()> {
        let columns = [(
            "link",
            "ALTER TABLE tags ADD COLUMN link TEXT NOT NULL DEFAULT ''",
        )];
        self.ensure_columns("tags", &columns, "tags")?;
        Ok(())
    }

    fn ensure_language_columns(&self) -> CoreResult<()> {
        let columns = [(
            "link",
            "ALTER TABLE languages ADD COLUMN link TEXT NOT NULL DEFAULT ''",
        )];
        self.ensure_columns("languages", &columns, "languages")?;
        Ok(())
    }

    fn ensure_rating_columns(&self) -> CoreResult<()> {
        let columns = [(
            "link",
            "ALTER TABLE ratings ADD COLUMN link TEXT NOT NULL DEFAULT ''",
        )];
        self.ensure_columns("ratings", &columns, "ratings")?;
        Ok(())
    }

    fn ensure_columns(&self, table: &str, columns: &[(&str, &str)], label: &str) -> CoreResult<()> {
        let mut stmt = self
            .conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(|err| {
                CoreError::Io(
                    format!("read {label} schema"),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let existing = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|err| {
                CoreError::Io(
                    format!("read {label} columns"),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?
            .collect::<Result<std::collections::BTreeSet<_>, _>>()
            .map_err(|err| {
                CoreError::Io(
                    format!("read {label} columns"),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        for (column, ddl) in columns {
            if !existing.contains(*column) {
                self.conn.execute(*ddl, []).map_err(|err| {
                    CoreError::Io(
                        format!("add {label} column {column}"),
                        std::io::Error::new(std::io::ErrorKind::Other, err),
                    )
                })?;
            }
        }
        Ok(())
    }

    fn column_exists(&self, table: &str, column: &str) -> CoreResult<bool> {
        let mut stmt = self
            .conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(|err| {
                CoreError::Io(
                    format!("read {table} schema"),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let mut rows = stmt.query([]).map_err(|err| {
            CoreError::Io(
                format!("read {table} columns"),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        while let Some(row) = rows.next().map_err(|err| {
            CoreError::Io(
                format!("read {table} columns"),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })? {
            let name: String = row.get(1).map_err(|err| {
                CoreError::Io(
                    format!("read {table} column name"),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
            if name == column {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn table_sql(&self, table: &str) -> CoreResult<Option<String>> {
        self.conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params![table],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| {
                CoreError::Io(
                    format!("read {table} create sql"),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })
    }

    fn table_sql_contains(&self, table: &str, needle: &str) -> CoreResult<bool> {
        Ok(self
            .table_sql(table)?
            .map(|sql| sql.contains(needle))
            .unwrap_or(false))
    }

    fn ensure_calibre_link_schema(&self) -> CoreResult<()> {
        if self.column_exists("comments", "book_id")? {
            self.migrate_comments_table()?;
        }
        if self.column_exists("identifiers", "identifier_type")? {
            self.migrate_identifiers_table()?;
        }
        if self.column_exists("books_authors_link", "book_id")? {
            self.migrate_books_authors_link()?;
        }
        if self.column_exists("books_tags_link", "book_id")? {
            self.migrate_books_tags_link()?;
        }
        if self.column_exists("books_series_link", "book_id")?
            || self.column_exists("books_series_link", "series_index")?
        {
            self.migrate_books_series_link()?;
        }
        if self.column_exists("books_publishers_link", "book_id")? {
            self.migrate_books_publishers_link()?;
        }
        if self.column_exists("books_ratings_link", "book_id")? {
            self.migrate_books_ratings_link()?;
        }
        if self.column_exists("books_languages_link", "book_id")?
            || self.column_exists("books_languages_link", "language_id")?
        {
            self.migrate_books_languages_link()?;
        }
        self.ensure_link_indices()?;
        Ok(())
    }

    fn ensure_calibre_collations(&self) -> CoreResult<()> {
        if !self.table_sql_contains(
            "books",
            "title TEXT NOT NULL DEFAULT 'Unknown' COLLATE NOCASE",
        )? || !self.table_sql_contains("books", "sort TEXT COLLATE NOCASE")?
            || !self.table_sql_contains("books", "timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP")?
            || !self.table_sql_contains("books", "pubdate TIMESTAMP DEFAULT CURRENT_TIMESTAMP")?
            || !self.table_sql_contains("books", "series_index REAL NOT NULL DEFAULT 1.0")?
            || !self.table_sql_contains("books", "author_sort TEXT COLLATE NOCASE")?
            || !self.table_sql_contains("books", "path TEXT NOT NULL DEFAULT ''")?
            || !self.table_sql_contains(
                "books",
                "last_modified TIMESTAMP NOT NULL DEFAULT '2000-01-01 00:00:00+00:00'",
            )?
        {
            self.rebuild_books_table()?;
        }
        if !self.table_sql_contains("authors", "name TEXT NOT NULL COLLATE NOCASE")?
            || !self.table_sql_contains("authors", "sort TEXT COLLATE NOCASE")?
        {
            self.rebuild_authors_table()?;
        }
        if !self.table_sql_contains("tags", "name TEXT NOT NULL COLLATE NOCASE")? {
            self.rebuild_tags_table()?;
        }
        if !self.table_sql_contains("series", "name TEXT NOT NULL COLLATE NOCASE")?
            || !self.table_sql_contains("series", "sort TEXT COLLATE NOCASE")?
        {
            self.rebuild_series_table()?;
        }
        if !self.table_sql_contains("publishers", "name TEXT NOT NULL COLLATE NOCASE")?
            || !self.table_sql_contains("publishers", "sort TEXT COLLATE NOCASE")?
        {
            self.rebuild_publishers_table()?;
        }
        if !self.table_sql_contains("languages", "lang_code TEXT NOT NULL COLLATE NOCASE")? {
            self.rebuild_languages_table()?;
        }
        if !self.table_sql_contains(
            "identifiers",
            "type TEXT NOT NULL DEFAULT 'isbn' COLLATE NOCASE",
        )? || !self.table_sql_contains("identifiers", "val TEXT NOT NULL COLLATE NOCASE")?
        {
            self.rebuild_identifiers_table()?;
        }
        if !self.table_sql_contains("data", "format TEXT NOT NULL COLLATE NOCASE")? {
            self.rebuild_data_table()?;
        }
        if !self.table_sql_contains("comments", "text TEXT NOT NULL COLLATE NOCASE")? {
            self.rebuild_comments_table()?;
        }
        if !self.table_sql_contains("last_read_positions", "format TEXT NOT NULL COLLATE NOCASE")? {
            self.rebuild_last_read_positions_table()?;
        }
        if !self.table_sql_contains(
            "ratings",
            "rating INTEGER CHECK(rating > -1 AND rating < 11)",
        )? {
            self.rebuild_ratings_table()?;
        }
        if !self.table_sql_contains(
            "books_pages_link",
            "format TEXT NOT NULL DEFAULT '' COLLATE NOCASE",
        )? {
            self.rebuild_books_pages_link_table()?;
        }
        Ok(())
    }

    fn ensure_link_indices(&self) -> CoreResult<()> {
        self.conn
            .execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_books_authors_book_id ON books_authors_link(book);
                 CREATE INDEX IF NOT EXISTS idx_books_authors_author_id ON books_authors_link(author);
                 CREATE INDEX IF NOT EXISTS idx_books_tags_book_id ON books_tags_link(book);
                 CREATE INDEX IF NOT EXISTS idx_books_tags_tag_id ON books_tags_link(tag);
                 CREATE INDEX IF NOT EXISTS idx_books_series_book_id ON books_series_link(book);
                 CREATE INDEX IF NOT EXISTS idx_books_series_series_id ON books_series_link(series);
                 CREATE INDEX IF NOT EXISTS idx_books_publishers_book_id ON books_publishers_link(book);
                 CREATE INDEX IF NOT EXISTS idx_books_publishers_publisher_id ON books_publishers_link(publisher);
                 CREATE INDEX IF NOT EXISTS idx_books_ratings_book_id ON books_ratings_link(book);
                 CREATE INDEX IF NOT EXISTS idx_books_ratings_rating_id ON books_ratings_link(rating);
                 CREATE INDEX IF NOT EXISTS idx_books_languages_book_id ON books_languages_link(book);
                 CREATE INDEX IF NOT EXISTS idx_books_languages_language_id ON books_languages_link(lang_code);
                 CREATE INDEX IF NOT EXISTS idx_identifiers_book_id ON identifiers(book);
                 CREATE INDEX IF NOT EXISTS idx_identifiers_type ON identifiers(type);",
            )
            .map_err(|err| {
                CoreError::Io(
                    "create link indices".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    fn migrate_comments_table(&self) -> CoreResult<()> {
        self.conn
            .execute_batch(
                "BEGIN;
                 CREATE TABLE comments_new (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    text TEXT NOT NULL,
                    UNIQUE(book),
                    FOREIGN KEY(book) REFERENCES books(id)
                 );
                 INSERT INTO comments_new (book, text)
                    SELECT book_id, text FROM comments;
                 DROP TABLE comments;
                 ALTER TABLE comments_new RENAME TO comments;
                 COMMIT;",
            )
            .map_err(|err| {
                CoreError::Io(
                    "migrate comments table".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    fn migrate_identifiers_table(&self) -> CoreResult<()> {
        self.conn
            .execute_batch(
                "BEGIN;
                 CREATE TABLE identifiers_new (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    type TEXT NOT NULL,
                    val TEXT NOT NULL,
                    UNIQUE(book, type),
                    FOREIGN KEY(book) REFERENCES books(id)
                 );
                 INSERT INTO identifiers_new (book, type, val)
                    SELECT book_id, identifier_type, identifier_value FROM identifiers;
                 DROP TABLE identifiers;
                 ALTER TABLE identifiers_new RENAME TO identifiers;
                 COMMIT;",
            )
            .map_err(|err| {
                CoreError::Io(
                    "migrate identifiers table".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    fn migrate_books_authors_link(&self) -> CoreResult<()> {
        self.conn
            .execute_batch(
                "BEGIN;
                 CREATE TABLE books_authors_link_new (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    author INTEGER NOT NULL,
                    UNIQUE(book, author),
                    FOREIGN KEY(book) REFERENCES books(id),
                    FOREIGN KEY(author) REFERENCES authors(id)
                 );
                 INSERT INTO books_authors_link_new (book, author)
                    SELECT book_id, author_id FROM books_authors_link;
                 DROP TABLE books_authors_link;
                 ALTER TABLE books_authors_link_new RENAME TO books_authors_link;
                 COMMIT;",
            )
            .map_err(|err| {
                CoreError::Io(
                    "migrate books_authors_link".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    fn migrate_books_tags_link(&self) -> CoreResult<()> {
        self.conn
            .execute_batch(
                "BEGIN;
                 CREATE TABLE books_tags_link_new (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    tag INTEGER NOT NULL,
                    UNIQUE(book, tag),
                    FOREIGN KEY(book) REFERENCES books(id),
                    FOREIGN KEY(tag) REFERENCES tags(id)
                 );
                 INSERT INTO books_tags_link_new (book, tag)
                    SELECT book_id, tag_id FROM books_tags_link;
                 DROP TABLE books_tags_link;
                 ALTER TABLE books_tags_link_new RENAME TO books_tags_link;
                 COMMIT;",
            )
            .map_err(|err| {
                CoreError::Io(
                    "migrate books_tags_link".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    fn migrate_books_series_link(&self) -> CoreResult<()> {
        self.conn
            .execute_batch(
                "BEGIN;
                 CREATE TABLE books_series_link_new (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    series INTEGER NOT NULL,
                    UNIQUE(book),
                    FOREIGN KEY(book) REFERENCES books(id),
                    FOREIGN KEY(series) REFERENCES series(id)
                 );
                 INSERT INTO books_series_link_new (book, series)
                    SELECT book_id, series_id FROM books_series_link;
                 UPDATE books
                    SET series_index = (
                        SELECT series_index
                        FROM books_series_link
                        WHERE book_id = books.id
                    )
                    WHERE EXISTS (
                        SELECT 1
                        FROM books_series_link
                        WHERE book_id = books.id
                    );
                 DROP TABLE books_series_link;
                 ALTER TABLE books_series_link_new RENAME TO books_series_link;
                 COMMIT;",
            )
            .map_err(|err| {
                CoreError::Io(
                    "migrate books_series_link".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    fn migrate_books_publishers_link(&self) -> CoreResult<()> {
        self.conn
            .execute_batch(
                "BEGIN;
                 CREATE TABLE books_publishers_link_new (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    publisher INTEGER NOT NULL,
                    UNIQUE(book),
                    FOREIGN KEY(book) REFERENCES books(id),
                    FOREIGN KEY(publisher) REFERENCES publishers(id)
                 );
                 INSERT INTO books_publishers_link_new (book, publisher)
                    SELECT book_id, publisher_id FROM books_publishers_link;
                 DROP TABLE books_publishers_link;
                 ALTER TABLE books_publishers_link_new RENAME TO books_publishers_link;
                 COMMIT;",
            )
            .map_err(|err| {
                CoreError::Io(
                    "migrate books_publishers_link".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    fn migrate_books_ratings_link(&self) -> CoreResult<()> {
        self.conn
            .execute_batch(
                "BEGIN;
                 CREATE TABLE books_ratings_link_new (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    rating INTEGER NOT NULL,
                    UNIQUE(book, rating),
                    FOREIGN KEY(book) REFERENCES books(id),
                    FOREIGN KEY(rating) REFERENCES ratings(id)
                 );
                 INSERT INTO books_ratings_link_new (book, rating)
                    SELECT book_id, rating_id FROM books_ratings_link;
                 DROP TABLE books_ratings_link;
                 ALTER TABLE books_ratings_link_new RENAME TO books_ratings_link;
                 COMMIT;",
            )
            .map_err(|err| {
                CoreError::Io(
                    "migrate books_ratings_link".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    fn migrate_books_languages_link(&self) -> CoreResult<()> {
        self.conn
            .execute_batch(
                "BEGIN;
                 CREATE TABLE books_languages_link_new (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    book INTEGER NOT NULL,
                    lang_code INTEGER NOT NULL,
                    item_order INTEGER NOT NULL DEFAULT 0,
                    UNIQUE(book, lang_code),
                    FOREIGN KEY(book) REFERENCES books(id),
                    FOREIGN KEY(lang_code) REFERENCES languages(id)
                 );
                 INSERT INTO books_languages_link_new (book, lang_code, item_order)
                    SELECT book_id, language_id, item_order FROM books_languages_link;
                 DROP TABLE books_languages_link;
                 ALTER TABLE books_languages_link_new RENAME TO books_languages_link;
                 COMMIT;",
            )
            .map_err(|err| {
                CoreError::Io(
                    "migrate books_languages_link".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    fn rebuild_with_foreign_keys_off(&self, sql: &str, label: &str) -> CoreResult<()> {
        self.conn
            .execute_batch("PRAGMA foreign_keys=OFF;")
            .map_err(|err| {
                CoreError::Io(
                    format!("disable foreign keys for {label}"),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let result = self.conn.execute_batch(sql).map_err(|err| {
            CoreError::Io(
                format!("rebuild {label}"),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        });
        self.conn
            .execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|err| {
                CoreError::Io(
                    format!("enable foreign keys for {label}"),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        result
    }

    fn rebuild_books_table(&self) -> CoreResult<()> {
        self.rebuild_with_foreign_keys_off(
            "BEGIN;
             CREATE TABLE books_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL DEFAULT 'Unknown' COLLATE NOCASE,
                sort TEXT COLLATE NOCASE,
                timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                pubdate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                series_index REAL NOT NULL DEFAULT 1.0,
                author_sort TEXT COLLATE NOCASE,
                path TEXT NOT NULL DEFAULT '',
                uuid TEXT,
                has_cover BOOL DEFAULT 0,
                last_modified TIMESTAMP NOT NULL DEFAULT '2000-01-01 00:00:00+00:00',
                format TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             INSERT INTO books_new (
                id,
                title,
                sort,
                timestamp,
                pubdate,
                series_index,
                author_sort,
                path,
                uuid,
                has_cover,
                last_modified,
                format,
                created_at
             )
                SELECT
                    id,
                    title,
                    sort,
                    timestamp,
                    pubdate,
                    series_index,
                    author_sort,
                    path,
                    uuid,
                    has_cover,
                    last_modified,
                    format,
                    created_at
                FROM books;
             DROP TABLE books;
             ALTER TABLE books_new RENAME TO books;
             COMMIT;",
            "books table",
        )
    }

    fn rebuild_authors_table(&self) -> CoreResult<()> {
        self.rebuild_with_foreign_keys_off(
            "BEGIN;
             CREATE TABLE authors_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL COLLATE NOCASE,
                sort TEXT COLLATE NOCASE,
                link TEXT NOT NULL DEFAULT '',
                UNIQUE(name)
             );
             INSERT INTO authors_new (id, name, sort, link)
                SELECT id, name, sort, link FROM authors;
             DROP TABLE authors;
             ALTER TABLE authors_new RENAME TO authors;
             COMMIT;",
            "authors table",
        )
    }

    fn rebuild_tags_table(&self) -> CoreResult<()> {
        self.rebuild_with_foreign_keys_off(
            "BEGIN;
             CREATE TABLE tags_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL COLLATE NOCASE,
                link TEXT NOT NULL DEFAULT '',
                UNIQUE(name)
             );
             INSERT INTO tags_new (id, name, link)
                SELECT id, name, link FROM tags;
             DROP TABLE tags;
             ALTER TABLE tags_new RENAME TO tags;
             COMMIT;",
            "tags table",
        )
    }

    fn rebuild_series_table(&self) -> CoreResult<()> {
        self.rebuild_with_foreign_keys_off(
            "BEGIN;
             CREATE TABLE series_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL COLLATE NOCASE,
                sort TEXT COLLATE NOCASE,
                link TEXT NOT NULL DEFAULT '',
                UNIQUE(name)
             );
             INSERT INTO series_new (id, name, sort, link)
                SELECT id, name, sort, link FROM series;
             DROP TABLE series;
             ALTER TABLE series_new RENAME TO series;
             COMMIT;",
            "series table",
        )
    }

    fn rebuild_publishers_table(&self) -> CoreResult<()> {
        self.rebuild_with_foreign_keys_off(
            "BEGIN;
             CREATE TABLE publishers_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL COLLATE NOCASE,
                sort TEXT COLLATE NOCASE,
                link TEXT NOT NULL DEFAULT '',
                UNIQUE(name)
             );
             INSERT INTO publishers_new (id, name, sort, link)
                SELECT id, name, sort, link FROM publishers;
             DROP TABLE publishers;
             ALTER TABLE publishers_new RENAME TO publishers;
             COMMIT;",
            "publishers table",
        )
    }

    fn rebuild_languages_table(&self) -> CoreResult<()> {
        self.rebuild_with_foreign_keys_off(
            "BEGIN;
             CREATE TABLE languages_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                lang_code TEXT NOT NULL COLLATE NOCASE,
                link TEXT NOT NULL DEFAULT '',
                UNIQUE(lang_code)
             );
             INSERT INTO languages_new (id, lang_code, link)
                SELECT id, lang_code, link FROM languages;
             DROP TABLE languages;
             ALTER TABLE languages_new RENAME TO languages;
             COMMIT;",
            "languages table",
        )
    }

    fn rebuild_identifiers_table(&self) -> CoreResult<()> {
        self.rebuild_with_foreign_keys_off(
            "BEGIN;
             CREATE TABLE identifiers_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                book INTEGER NOT NULL,
                type TEXT NOT NULL DEFAULT 'isbn' COLLATE NOCASE,
                val TEXT NOT NULL COLLATE NOCASE,
                UNIQUE(book, type),
                FOREIGN KEY(book) REFERENCES books(id)
             );
             INSERT INTO identifiers_new (id, book, type, val)
                SELECT id, book, type, val FROM identifiers;
             DROP TABLE identifiers;
             ALTER TABLE identifiers_new RENAME TO identifiers;
             COMMIT;",
            "identifiers table",
        )
    }

    fn rebuild_data_table(&self) -> CoreResult<()> {
        self.rebuild_with_foreign_keys_off(
            "BEGIN;
             CREATE TABLE data_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                book INTEGER NOT NULL,
                format TEXT NOT NULL COLLATE NOCASE,
                uncompressed_size INTEGER NOT NULL,
                name TEXT NOT NULL,
                UNIQUE(book, format)
             );
             INSERT INTO data_new (id, book, format, uncompressed_size, name)
                SELECT id, book, format, uncompressed_size, name FROM data;
             DROP TABLE data;
             ALTER TABLE data_new RENAME TO data;
             COMMIT;",
            "data table",
        )
    }

    fn rebuild_comments_table(&self) -> CoreResult<()> {
        self.rebuild_with_foreign_keys_off(
            "BEGIN;
             CREATE TABLE comments_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                book INTEGER NOT NULL,
                text TEXT NOT NULL COLLATE NOCASE,
                UNIQUE(book),
                FOREIGN KEY(book) REFERENCES books(id)
             );
             INSERT INTO comments_new (id, book, text)
                SELECT id, book, text FROM comments;
             DROP TABLE comments;
             ALTER TABLE comments_new RENAME TO comments;
             COMMIT;",
            "comments table",
        )
    }

    fn rebuild_last_read_positions_table(&self) -> CoreResult<()> {
        self.rebuild_with_foreign_keys_off(
            "BEGIN;
             CREATE TABLE last_read_positions_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                book INTEGER NOT NULL,
                format TEXT NOT NULL COLLATE NOCASE,
                user TEXT NOT NULL,
                device TEXT NOT NULL,
                cfi TEXT NOT NULL,
                epoch REAL NOT NULL,
                pos_frac REAL NOT NULL DEFAULT 0,
                UNIQUE(user, device, book, format)
             );
             INSERT INTO last_read_positions_new (
                id,
                book,
                format,
                user,
                device,
                cfi,
                epoch,
                pos_frac
             )
                SELECT
                    id,
                    book,
                    format,
                    user,
                    device,
                    cfi,
                    epoch,
                    pos_frac
                FROM last_read_positions;
             DROP TABLE last_read_positions;
             ALTER TABLE last_read_positions_new RENAME TO last_read_positions;
             COMMIT;",
            "last_read_positions table",
        )
    }

    fn rebuild_ratings_table(&self) -> CoreResult<()> {
        self.rebuild_with_foreign_keys_off(
            "BEGIN;
             CREATE TABLE ratings_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                rating INTEGER CHECK(rating > -1 AND rating < 11),
                link TEXT NOT NULL DEFAULT '',
                UNIQUE (rating)
             );
             INSERT INTO ratings_new (id, rating, link)
                SELECT id, rating, link FROM ratings;
             DROP TABLE ratings;
             ALTER TABLE ratings_new RENAME TO ratings;
             COMMIT;",
            "ratings table",
        )
    }

    fn rebuild_books_pages_link_table(&self) -> CoreResult<()> {
        self.rebuild_with_foreign_keys_off(
            "BEGIN;
             CREATE TABLE books_pages_link_new (
                book INTEGER PRIMARY KEY,
                pages INTEGER NOT NULL DEFAULT 0,
                algorithm INTEGER NOT NULL DEFAULT 0,
                format TEXT NOT NULL DEFAULT '' COLLATE NOCASE,
                format_size INTEGER NOT NULL DEFAULT 0,
                timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                needs_scan INTEGER NOT NULL DEFAULT 0 CHECK(needs_scan IN (0, 1)),
                FOREIGN KEY (book) REFERENCES books(id) ON DELETE CASCADE
             );
             INSERT INTO books_pages_link_new (
                book,
                pages,
                algorithm,
                format,
                format_size,
                timestamp,
                needs_scan
             )
                SELECT
                    book,
                    pages,
                    algorithm,
                    format,
                    format_size,
                    timestamp,
                    needs_scan
                FROM books_pages_link;
             DROP TABLE books_pages_link;
             ALTER TABLE books_pages_link_new RENAME TO books_pages_link;
             COMMIT;",
            "books_pages_link table",
        )
    }

    fn ensure_calibre_views(&self) -> CoreResult<()> {
        self.conn
            .execute_batch(
                "CREATE VIEW IF NOT EXISTS meta AS
                    SELECT id, title,
                           (SELECT sortconcat(bal.id, name) FROM books_authors_link AS bal JOIN authors ON(author = authors.id) WHERE book = books.id) authors,
                           (SELECT name FROM publishers WHERE publishers.id IN (SELECT publisher from books_publishers_link WHERE book=books.id)) publisher,
                           (SELECT rating FROM ratings WHERE ratings.id IN (SELECT rating from books_ratings_link WHERE book=books.id)) rating,
                           timestamp,
                           (SELECT MAX(uncompressed_size) FROM data WHERE book=books.id) size,
                           (SELECT concat(name) FROM tags WHERE tags.id IN (SELECT tag from books_tags_link WHERE book=books.id)) tags,
                           (SELECT text FROM comments WHERE book=books.id) comments,
                           (SELECT name FROM series WHERE series.id IN (SELECT series FROM books_series_link WHERE book=books.id)) series,
                           series_index,
                           sort,
                           author_sort,
                           (SELECT concat(format) FROM data WHERE data.book=books.id) formats,
                           path,
                           pubdate,
                           uuid
                    FROM books;
                CREATE VIEW IF NOT EXISTS tag_browser_authors AS SELECT
                            id,
                            name,
                            (SELECT COUNT(id) FROM books_authors_link WHERE author=authors.id) count,
                            (SELECT AVG(ratings.rating)
                             FROM books_authors_link AS tl, books_ratings_link AS bl, ratings
                             WHERE tl.author=authors.id AND bl.book=tl.book AND
                             ratings.id = bl.rating AND ratings.rating <> 0) avg_rating,
                             sort AS sort
                        FROM authors;
                CREATE VIEW IF NOT EXISTS tag_browser_filtered_authors AS SELECT
                            id,
                            name,
                            (SELECT COUNT(books_authors_link.id) FROM books_authors_link WHERE
                                author=authors.id AND books_list_filter(book)) count,
                            (SELECT AVG(ratings.rating)
                             FROM books_authors_link AS tl, books_ratings_link AS bl, ratings
                             WHERE tl.author=authors.id AND bl.book=tl.book AND
                             ratings.id = bl.rating AND ratings.rating <> 0 AND
                             books_list_filter(bl.book)) avg_rating,
                             sort AS sort
                        FROM authors;
                CREATE VIEW IF NOT EXISTS tag_browser_filtered_publishers AS SELECT
                            id,
                            name,
                            (SELECT COUNT(books_publishers_link.id) FROM books_publishers_link WHERE
                                publisher=publishers.id AND books_list_filter(book)) count,
                            (SELECT AVG(ratings.rating)
                             FROM books_publishers_link AS tl, books_ratings_link AS bl, ratings
                             WHERE tl.publisher=publishers.id AND bl.book=tl.book AND
                             ratings.id = bl.rating AND ratings.rating <> 0 AND
                             books_list_filter(bl.book)) avg_rating,
                             name AS sort
                        FROM publishers;
                CREATE VIEW IF NOT EXISTS tag_browser_filtered_ratings AS SELECT
                            id,
                            rating,
                            (SELECT COUNT(books_ratings_link.id) FROM books_ratings_link WHERE
                                rating=ratings.id AND books_list_filter(book)) count,
                            (SELECT AVG(ratings.rating)
                             FROM books_ratings_link AS tl, books_ratings_link AS bl, ratings
                             WHERE tl.rating=ratings.id AND bl.book=tl.book AND
                             ratings.id = bl.rating AND ratings.rating <> 0 AND
                             books_list_filter(bl.book)) avg_rating,
                             rating AS sort
                        FROM ratings;
                CREATE VIEW IF NOT EXISTS tag_browser_filtered_series AS SELECT
                            id,
                            name,
                            (SELECT COUNT(books_series_link.id) FROM books_series_link WHERE
                                series=series.id AND books_list_filter(book)) count,
                            (SELECT AVG(ratings.rating)
                             FROM books_series_link AS tl, books_ratings_link AS bl, ratings
                             WHERE tl.series=series.id AND bl.book=tl.book AND
                             ratings.id = bl.rating AND ratings.rating <> 0 AND
                             books_list_filter(bl.book)) avg_rating,
                             (title_sort(name)) AS sort
                        FROM series;
                CREATE VIEW IF NOT EXISTS tag_browser_filtered_tags AS SELECT
                            id,
                            name,
                            (SELECT COUNT(books_tags_link.id) FROM books_tags_link WHERE
                                tag=tags.id AND books_list_filter(book)) count,
                            (SELECT AVG(ratings.rating)
                             FROM books_tags_link AS tl, books_ratings_link AS bl, ratings
                             WHERE tl.tag=tags.id AND bl.book=tl.book AND
                             ratings.id = bl.rating AND ratings.rating <> 0 AND
                             books_list_filter(bl.book)) avg_rating,
                             name AS sort
                        FROM tags;
                CREATE VIEW IF NOT EXISTS tag_browser_publishers AS SELECT
                            id,
                            name,
                            (SELECT COUNT(id) FROM books_publishers_link WHERE publisher=publishers.id) count,
                            (SELECT AVG(ratings.rating)
                             FROM books_publishers_link AS tl, books_ratings_link AS bl, ratings
                             WHERE tl.publisher=publishers.id AND bl.book=tl.book AND
                             ratings.id = bl.rating AND ratings.rating <> 0) avg_rating,
                             name AS sort
                        FROM publishers;
                CREATE VIEW IF NOT EXISTS tag_browser_ratings AS SELECT
                            id,
                            rating,
                            (SELECT COUNT(id) FROM books_ratings_link WHERE rating=ratings.id) count,
                            (SELECT AVG(ratings.rating)
                             FROM books_ratings_link AS tl, books_ratings_link AS bl, ratings
                             WHERE tl.rating=ratings.id AND bl.book=tl.book AND
                             ratings.id = bl.rating AND ratings.rating <> 0) avg_rating,
                             rating AS sort
                        FROM ratings;
                CREATE VIEW IF NOT EXISTS tag_browser_series AS SELECT
                            id,
                            name,
                            (SELECT COUNT(id) FROM books_series_link WHERE series=series.id) count,
                            (SELECT AVG(ratings.rating)
                             FROM books_series_link AS tl, books_ratings_link AS bl, ratings
                             WHERE tl.series=series.id AND bl.book=tl.book AND
                             ratings.id = bl.rating AND ratings.rating <> 0) avg_rating,
                             (title_sort(name)) AS sort
                        FROM series;
                CREATE VIEW IF NOT EXISTS tag_browser_tags AS SELECT
                            id,
                            name,
                            (SELECT COUNT(id) FROM books_tags_link WHERE tag=tags.id) count,
                            (SELECT AVG(ratings.rating)
                             FROM books_tags_link AS tl, books_ratings_link AS bl, ratings
                             WHERE tl.tag=tags.id AND bl.book=tl.book AND
                             ratings.id = bl.rating AND ratings.rating <> 0) avg_rating,
                             name AS sort
                        FROM tags;",
            )
            .map_err(|err| {
                CoreError::Io(
                    "create calibre views".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    fn ensure_calibre_indices(&self) -> CoreResult<()> {
        self.conn
            .execute_batch(
                "CREATE INDEX IF NOT EXISTS authors_idx ON books (author_sort COLLATE NOCASE);
                 CREATE INDEX IF NOT EXISTS books_authors_link_aidx ON books_authors_link (author);
                 CREATE INDEX IF NOT EXISTS books_authors_link_bidx ON books_authors_link (book);
                 CREATE INDEX IF NOT EXISTS books_idx ON books (sort COLLATE NOCASE);
                 CREATE INDEX IF NOT EXISTS books_languages_link_aidx ON books_languages_link (lang_code);
                 CREATE INDEX IF NOT EXISTS books_languages_link_bidx ON books_languages_link (book);
                 CREATE INDEX IF NOT EXISTS books_publishers_link_aidx ON books_publishers_link (publisher);
                 CREATE INDEX IF NOT EXISTS books_publishers_link_bidx ON books_publishers_link (book);
                 CREATE INDEX IF NOT EXISTS books_ratings_link_aidx ON books_ratings_link (rating);
                 CREATE INDEX IF NOT EXISTS books_ratings_link_bidx ON books_ratings_link (book);
                 CREATE INDEX IF NOT EXISTS books_series_link_aidx ON books_series_link (series);
                 CREATE INDEX IF NOT EXISTS books_series_link_bidx ON books_series_link (book);
                 CREATE INDEX IF NOT EXISTS books_tags_link_aidx ON books_tags_link (tag);
                 CREATE INDEX IF NOT EXISTS books_tags_link_bidx ON books_tags_link (book);
                 CREATE INDEX IF NOT EXISTS comments_idx ON comments (book);
                 CREATE INDEX IF NOT EXISTS conversion_options_idx_a ON conversion_options (format COLLATE NOCASE);
                 CREATE INDEX IF NOT EXISTS conversion_options_idx_b ON conversion_options (book);
                 CREATE INDEX IF NOT EXISTS custom_columns_idx ON custom_columns (label);
                 CREATE INDEX IF NOT EXISTS data_idx ON data (book);
                 CREATE INDEX IF NOT EXISTS lrp_idx ON last_read_positions (book);
                 CREATE INDEX IF NOT EXISTS annot_idx ON annotations (book);
                 CREATE INDEX IF NOT EXISTS formats_idx ON data (format);
                 CREATE INDEX IF NOT EXISTS languages_idx ON languages (lang_code COLLATE NOCASE);
                 CREATE INDEX IF NOT EXISTS publishers_idx ON publishers (name COLLATE NOCASE);
                 CREATE INDEX IF NOT EXISTS series_idx ON series (name COLLATE NOCASE);
                 CREATE INDEX IF NOT EXISTS tags_idx ON tags (name COLLATE NOCASE);",
            )
            .map_err(|err| {
                CoreError::Io(
                    "create calibre indices".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    fn ensure_calibre_triggers(&self) -> CoreResult<()> {
        self.conn
            .execute_batch(
                "CREATE TRIGGER IF NOT EXISTS books_delete_trg
                    AFTER DELETE ON books
                    BEGIN
                        DELETE FROM books_authors_link WHERE book=OLD.id;
                        DELETE FROM books_publishers_link WHERE book=OLD.id;
                        DELETE FROM books_ratings_link WHERE book=OLD.id;
                        DELETE FROM books_series_link WHERE book=OLD.id;
                        DELETE FROM books_tags_link WHERE book=OLD.id;
                        DELETE FROM books_languages_link WHERE book=OLD.id;
                        DELETE FROM data WHERE book=OLD.id;
                        DELETE FROM last_read_positions WHERE book=OLD.id;
                        DELETE FROM annotations WHERE book=OLD.id;
                        DELETE FROM comments WHERE book=OLD.id;
                        DELETE FROM conversion_options WHERE book=OLD.id;
                        DELETE FROM books_plugin_data WHERE book=OLD.id;
                        DELETE FROM identifiers WHERE book=OLD.id;
                    END;
                CREATE TRIGGER IF NOT EXISTS books_insert_trg AFTER INSERT ON books
                    BEGIN
                        UPDATE books SET sort=title_sort(NEW.title),uuid=uuid4() WHERE id=NEW.id;
                    END;
                CREATE TRIGGER IF NOT EXISTS books_update_trg
                    AFTER UPDATE ON books
                    BEGIN
                        UPDATE books SET sort=title_sort(NEW.title)
                                     WHERE id=NEW.id AND OLD.title <> NEW.title;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_comments_insert
                    BEFORE INSERT ON comments
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_comments_update
                    BEFORE UPDATE OF book ON comments
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_data_insert
                    BEFORE INSERT ON data
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_data_update
                    BEFORE UPDATE OF book ON data
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_lrp_insert
                    BEFORE INSERT ON last_read_positions
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_lrp_update
                    BEFORE UPDATE OF book ON last_read_positions
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_annot_insert
                    BEFORE INSERT ON annotations
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_annot_update
                    BEFORE UPDATE OF book ON annotations
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_delete_on_authors
                    BEFORE DELETE ON authors
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT COUNT(id) FROM books_authors_link WHERE author=OLD.id) > 0
                            THEN RAISE(ABORT, 'Foreign key violation: authors is still referenced')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_delete_on_languages
                    BEFORE DELETE ON languages
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT COUNT(id) FROM books_languages_link WHERE lang_code=OLD.id) > 0
                            THEN RAISE(ABORT, 'Foreign key violation: language is still referenced')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_delete_on_languages_link
                    BEFORE INSERT ON books_languages_link
                    BEGIN
                      SELECT CASE
                          WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                          THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                          WHEN (SELECT id from languages WHERE id=NEW.lang_code) IS NULL
                          THEN RAISE(ABORT, 'Foreign key violation: lang_code not in languages')
                      END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_delete_on_publishers
                    BEFORE DELETE ON publishers
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT COUNT(id) FROM books_publishers_link WHERE publisher=OLD.id) > 0
                            THEN RAISE(ABORT, 'Foreign key violation: publishers is still referenced')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_delete_on_series
                    BEFORE DELETE ON series
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT COUNT(id) FROM books_series_link WHERE series=OLD.id) > 0
                            THEN RAISE(ABORT, 'Foreign key violation: series is still referenced')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_delete_on_tags
                    BEFORE DELETE ON tags
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT COUNT(id) FROM books_tags_link WHERE tag=OLD.id) > 0
                            THEN RAISE(ABORT, 'Foreign key violation: tags is still referenced')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_insert_books_authors_link
                    BEFORE INSERT ON books_authors_link
                    BEGIN
                      SELECT CASE
                          WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                          THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                          WHEN (SELECT id from authors WHERE id=NEW.author) IS NULL
                          THEN RAISE(ABORT, 'Foreign key violation: author not in authors')
                      END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_insert_books_publishers_link
                    BEFORE INSERT ON books_publishers_link
                    BEGIN
                      SELECT CASE
                          WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                          THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                          WHEN (SELECT id from publishers WHERE id=NEW.publisher) IS NULL
                          THEN RAISE(ABORT, 'Foreign key violation: publisher not in publishers')
                      END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_insert_books_ratings_link
                    BEFORE INSERT ON books_ratings_link
                    BEGIN
                      SELECT CASE
                          WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                          THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                          WHEN (SELECT id from ratings WHERE id=NEW.rating) IS NULL
                          THEN RAISE(ABORT, 'Foreign key violation: rating not in ratings')
                      END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_insert_books_series_link
                    BEFORE INSERT ON books_series_link
                    BEGIN
                      SELECT CASE
                          WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                          THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                          WHEN (SELECT id from series WHERE id=NEW.series) IS NULL
                          THEN RAISE(ABORT, 'Foreign key violation: series not in series')
                      END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_insert_books_tags_link
                    BEFORE INSERT ON books_tags_link
                    BEGIN
                      SELECT CASE
                          WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                          THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                          WHEN (SELECT id from tags WHERE id=NEW.tag) IS NULL
                          THEN RAISE(ABORT, 'Foreign key violation: tag not in tags')
                      END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_update_books_authors_link_a
                    BEFORE UPDATE OF book ON books_authors_link
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_update_books_authors_link_b
                    BEFORE UPDATE OF author ON books_authors_link
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from authors WHERE id=NEW.author) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: author not in authors')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_update_books_languages_link_a
                    BEFORE UPDATE OF book ON books_languages_link
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_update_books_languages_link_b
                    BEFORE UPDATE OF lang_code ON books_languages_link
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from languages WHERE id=NEW.lang_code) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: lang_code not in languages')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_update_books_publishers_link_a
                    BEFORE UPDATE OF book ON books_publishers_link
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_update_books_publishers_link_b
                    BEFORE UPDATE OF publisher ON books_publishers_link
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from publishers WHERE id=NEW.publisher) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: publisher not in publishers')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_update_books_ratings_link_a
                    BEFORE UPDATE OF book ON books_ratings_link
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_update_books_ratings_link_b
                    BEFORE UPDATE OF rating ON books_ratings_link
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from ratings WHERE id=NEW.rating) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: rating not in ratings')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_update_books_series_link_a
                    BEFORE UPDATE OF book ON books_series_link
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_update_books_series_link_b
                    BEFORE UPDATE OF series ON books_series_link
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from series WHERE id=NEW.series) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: series not in series')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_update_books_tags_link_a
                    BEFORE UPDATE OF book ON books_tags_link
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from books WHERE id=NEW.book) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: book not in books')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS fkc_update_books_tags_link_b
                    BEFORE UPDATE OF tag ON books_tags_link
                    BEGIN
                        SELECT CASE
                            WHEN (SELECT id from tags WHERE id=NEW.tag) IS NULL
                            THEN RAISE(ABORT, 'Foreign key violation: tag not in tags')
                        END;
                    END;
                CREATE TRIGGER IF NOT EXISTS series_insert_trg
                    AFTER INSERT ON series
                    BEGIN
                        UPDATE series SET sort=title_sort(NEW.name) WHERE id=NEW.id;
                    END;
                CREATE TRIGGER IF NOT EXISTS series_update_trg
                    AFTER UPDATE ON series
                    BEGIN
                        UPDATE series SET sort=title_sort(NEW.name) WHERE id=NEW.id;
                    END;",
            )
            .map_err(|err| {
                CoreError::Io(
                    "create calibre triggers".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    fn ensure_calibre_pragmas(&self) -> CoreResult<()> {
        self.conn
            .execute_batch("PRAGMA application_id = 0x63616c69; PRAGMA user_version = 27;")
            .map_err(|err| {
                CoreError::Io(
                    "set calibre pragmas".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
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

    pub fn update_book_path(&self, id: i64, path: &str) -> CoreResult<()> {
        self.conn
            .execute(
                "UPDATE books SET path = ?1 WHERE id = ?2",
                params![path, id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "update book path".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    pub fn search_books(&self, query: &str) -> CoreResult<Vec<BookRecord>> {
        if self.fts.enabled && query.chars().count() >= self.fts.min_query_len {
            if let Ok(results) = self.search_books_fts(query) {
                return Ok(results);
            }
        }
        self.search_books_like(query)
    }

    pub fn search_books_query(&self, query: &BookQuery) -> CoreResult<Vec<BookRecord>> {
        let mut sql = String::from("SELECT DISTINCT b.id, b.title, b.format, b.path FROM books b");
        let mut joins: Vec<&str> = Vec::new();
        let mut conditions: Vec<String> = Vec::new();
        let mut params: Vec<Value> = Vec::new();

        if query.author.is_some() {
            joins.push(
                "LEFT JOIN books_authors_link bal ON bal.book = b.id \
                 LEFT JOIN authors a ON a.id = bal.author",
            );
        }
        if query.tag.is_some() {
            joins.push(
                "LEFT JOIN books_tags_link btl ON btl.book = b.id \
                 LEFT JOIN tags t ON t.id = btl.tag",
            );
        }
        if query.series.is_some() {
            joins.push(
                "LEFT JOIN books_series_link bsl ON bsl.book = b.id \
                 LEFT JOIN series s ON s.id = bsl.series",
            );
        }
        if query.publisher.is_some() {
            joins.push(
                "LEFT JOIN books_publishers_link bpl ON bpl.book = b.id \
                 LEFT JOIN publishers p ON p.id = bpl.publisher",
            );
        }
        if query.language.is_some() {
            joins.push(
                "LEFT JOIN books_languages_link bll ON bll.book = b.id \
                 LEFT JOIN languages l ON l.id = bll.lang_code",
            );
        }
        if query.identifier.is_some() {
            joins.push("LEFT JOIN identifiers i ON i.book = b.id");
        }

        if let Some(title) = &query.title {
            conditions.push("b.title LIKE ?".to_string());
            params.push(Value::from(format!("%{title}%")));
        }
        if let Some(author) = &query.author {
            conditions.push("a.name LIKE ?".to_string());
            params.push(Value::from(format!("%{author}%")));
        }
        if let Some(tag) = &query.tag {
            conditions.push("t.name LIKE ?".to_string());
            params.push(Value::from(format!("%{tag}%")));
        }
        if let Some(series) = &query.series {
            conditions.push("s.name LIKE ?".to_string());
            params.push(Value::from(format!("%{series}%")));
        }
        if let Some(publisher) = &query.publisher {
            conditions.push("p.name LIKE ?".to_string());
            params.push(Value::from(format!("%{publisher}%")));
        }
        if let Some(language) = &query.language {
            conditions.push("l.lang_code LIKE ?".to_string());
            params.push(Value::from(format!("%{language}%")));
        }
        if let Some(identifier) = &query.identifier {
            conditions.push("(i.val LIKE ? OR i.type LIKE ?)".to_string());
            let pattern = Value::from(format!("%{identifier}%"));
            params.push(pattern.clone());
            params.push(pattern);
        }
        if let Some(format) = &query.format {
            conditions.push("b.format LIKE ?".to_string());
            params.push(Value::from(format!("%{format}%")));
        }

        if !joins.is_empty() {
            sql.push(' ');
            sql.push_str(&joins.join(" "));
        }
        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(" ORDER BY b.id");
        if let Some(limit) = query.limit {
            sql.push_str(" LIMIT ?");
            params.push(Value::from(limit as i64));
        }

        let mut stmt = self.conn.prepare(&sql).map_err(|err| {
            CoreError::Io(
                "prepare search query".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        let rows = stmt
            .query_map(params_from_iter(params), |row| {
                Ok(BookRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    format: row.get(2)?,
                    path: row.get(3)?,
                })
            })
            .map_err(|err| {
                CoreError::Io(
                    "query search query".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|err| {
                CoreError::Io(
                    "read search query".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?);
        }
        Ok(results)
    }

    pub fn search_books_like(&self, query: &str) -> CoreResult<Vec<BookRecord>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT DISTINCT b.id, b.title, b.format, b.path
                 FROM books b
                 LEFT JOIN books_authors_link bal ON bal.book = b.id
                 LEFT JOIN authors a ON a.id = bal.author
                 LEFT JOIN books_tags_link btl ON btl.book = b.id
                 LEFT JOIN tags t ON t.id = btl.tag
                 LEFT JOIN books_series_link bsl ON bsl.book = b.id
                 LEFT JOIN series s ON s.id = bsl.series
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

    pub fn update_asset_paths(
        &self,
        id: i64,
        stored_path: &str,
        storage_mode: &str,
        source_path: Option<&str>,
    ) -> CoreResult<()> {
        self.conn
            .execute(
                "UPDATE assets SET stored_path = ?1, storage_mode = ?2, source_path = ?3 WHERE id = ?4",
                params![stored_path, storage_mode, source_path, id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "update asset path".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
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

    pub fn list_author_categories(&self) -> CoreResult<Vec<CategoryCount>> {
        self.list_category_counts(
            "authors",
            "name",
            "books_authors_link",
            "author",
            "author categories",
        )
    }

    pub fn list_tag_categories(&self) -> CoreResult<Vec<CategoryCount>> {
        self.list_category_counts("tags", "name", "books_tags_link", "tag", "tag categories")
    }

    pub fn list_series_categories(&self) -> CoreResult<Vec<CategoryCount>> {
        self.list_category_counts(
            "series",
            "name",
            "books_series_link",
            "series",
            "series categories",
        )
    }

    pub fn list_publisher_categories(&self) -> CoreResult<Vec<CategoryCount>> {
        self.list_category_counts(
            "publishers",
            "name",
            "books_publishers_link",
            "publisher",
            "publisher categories",
        )
    }

    pub fn list_rating_categories(&self) -> CoreResult<Vec<CategoryCount>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT r.id, r.rating, COUNT(brl.id)
                 FROM ratings r
                 LEFT JOIN books_ratings_link brl ON brl.rating = r.id
                 GROUP BY r.id
                 ORDER BY r.rating",
            )
            .map_err(|err| {
                CoreError::Io(
                    "prepare rating categories".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let rows = stmt
            .query_map([], |row| {
                let rating: i64 = row.get(1)?;
                Ok(CategoryCount {
                    id: row.get(0)?,
                    name: rating.to_string(),
                    count: row.get(2)?,
                })
            })
            .map_err(|err| {
                CoreError::Io(
                    "query rating categories".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|err| {
                CoreError::Io(
                    "read rating categories".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?);
        }
        Ok(results)
    }

    pub fn list_language_categories(&self) -> CoreResult<Vec<CategoryCount>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT l.id, l.lang_code, COUNT(bll.id)
                 FROM languages l
                 LEFT JOIN books_languages_link bll ON bll.lang_code = l.id
                 GROUP BY l.id
                 ORDER BY l.lang_code",
            )
            .map_err(|err| {
                CoreError::Io(
                    "prepare language categories".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let rows = stmt
            .query_map([], |row| {
                let lang_code: String = row.get(1)?;
                Ok(CategoryCount {
                    id: row.get(0)?,
                    name: lang_code,
                    count: row.get(2)?,
                })
            })
            .map_err(|err| {
                CoreError::Io(
                    "query language categories".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|err| {
                CoreError::Io(
                    "read language categories".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?);
        }
        Ok(results)
    }

    pub fn list_custom_columns(&self) -> CoreResult<Vec<CustomColumn>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, label, name, datatype, mark_for_delete, editable, display, is_multiple, normalized
                 FROM custom_columns ORDER BY id",
            )
            .map_err(|err| {
                CoreError::Io(
                    "prepare list custom columns".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let rows = stmt
            .query_map([], |row| {
                let mark_for_delete: i64 = row.get(4)?;
                let editable: i64 = row.get(5)?;
                let is_multiple: i64 = row.get(7)?;
                let normalized: i64 = row.get(8)?;
                Ok(CustomColumn {
                    id: row.get(0)?,
                    label: row.get(1)?,
                    name: row.get(2)?,
                    datatype: row.get(3)?,
                    mark_for_delete: mark_for_delete != 0,
                    editable: editable != 0,
                    display: row.get(6)?,
                    is_multiple: is_multiple != 0,
                    normalized: normalized != 0,
                })
            })
            .map_err(|err| {
                CoreError::Io(
                    "query list custom columns".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|err| {
                CoreError::Io(
                    "read list custom columns".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?);
        }
        Ok(results)
    }

    pub fn create_custom_column(
        &self,
        label: &str,
        name: &str,
        datatype: &str,
        display: &str,
    ) -> CoreResult<i64> {
        let normalized = if matches!(datatype, "int" | "float") {
            1
        } else {
            0
        };
        self.conn
            .execute(
                "INSERT INTO custom_columns (label, name, datatype, display, normalized)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![label, name, datatype, display, normalized],
            )
            .map_err(|err| {
                CoreError::Io(
                    "insert custom column".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let id = self.conn.last_insert_rowid();
        self.create_custom_column_table(label, datatype)?;
        Ok(id)
    }

    pub fn delete_custom_column(&self, label: &str) -> CoreResult<bool> {
        let changed = self
            .conn
            .execute(
                "UPDATE custom_columns SET mark_for_delete = 1 WHERE label = ?1",
                params![label],
            )
            .map_err(|err| {
                CoreError::Io(
                    "mark custom column delete".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        if changed == 0 {
            return Ok(false);
        }
        let table_name = format!("custom_column_{label}");
        self.conn
            .execute(&format!("DROP TABLE IF EXISTS {table_name}"), [])
            .map_err(|err| {
                CoreError::Io(
                    "drop custom column table".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        self.conn
            .execute(
                "DELETE FROM custom_columns WHERE label = ?1",
                params![label],
            )
            .map_err(|err| {
                CoreError::Io(
                    "delete custom column row".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(true)
    }

    pub fn set_custom_value(&self, book_id: i64, label: &str, value: &str) -> CoreResult<()> {
        let table_name = format!("custom_column_{label}");
        if !self.schema_object_exists("table", &table_name)? {
            let datatype = self.lookup_custom_column_datatype(label)?;
            self.create_custom_column_table(label, &datatype)?;
        }
        self.conn
            .execute(
                &format!(
                    "INSERT INTO {table_name} (book, value) VALUES (?1, ?2)
                     ON CONFLICT(book) DO UPDATE SET value = excluded.value"
                ),
                params![book_id, value],
            )
            .map_err(|err| {
                CoreError::Io(
                    "set custom column value".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    pub fn get_custom_value(&self, book_id: i64, label: &str) -> CoreResult<Option<String>> {
        let table_name = format!("custom_column_{label}");
        if !self.schema_object_exists("table", &table_name)? {
            return Ok(None);
        }
        self.conn
            .query_row(
                &format!("SELECT value FROM {table_name} WHERE book = ?1"),
                params![book_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| {
                CoreError::Io(
                    "get custom column value".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })
    }

    pub fn schema_object_exists(&self, object_type: &str, name: &str) -> CoreResult<bool> {
        let exists: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = ?1 AND name = ?2",
                params![object_type, name],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| {
                CoreError::Io(
                    "query schema object".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(exists.is_some())
    }

    pub fn table_columns(&self, table: &str) -> CoreResult<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(|err| {
                CoreError::Io(
                    "query table columns".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|err| {
                CoreError::Io(
                    "query table columns".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let mut columns = Vec::new();
        for row in rows {
            columns.push(row.map_err(|err| {
                CoreError::Io(
                    "read table columns".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?);
        }
        Ok(columns)
    }

    pub fn get_preference_json(&self, key: &str) -> CoreResult<Option<JsonValue>> {
        let value: Option<String> = self
            .conn
            .query_row(
                "SELECT val FROM preferences WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| {
                CoreError::Io(
                    "query preference".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        let Some(raw) = value else {
            return Ok(None);
        };
        let parsed = serde_json::from_str(&raw).map_err(|err| {
            CoreError::Io(
                "parse preference".to_string(),
                std::io::Error::new(std::io::ErrorKind::InvalidData, err),
            )
        })?;
        Ok(Some(parsed))
    }

    pub fn set_preference_json(&self, key: &str, value: &JsonValue) -> CoreResult<()> {
        let raw = serde_json::to_string_pretty(value).map_err(|err| {
            CoreError::Io(
                "encode preference".to_string(),
                std::io::Error::new(std::io::ErrorKind::InvalidData, err),
            )
        })?;
        self.conn
            .execute(
                "INSERT INTO preferences (key, val) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET val = excluded.val",
                params![key, raw],
            )
            .map_err(|err| {
                CoreError::Io(
                    "write preference".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    pub fn delete_preference(&self, key: &str) -> CoreResult<()> {
        self.conn
            .execute("DELETE FROM preferences WHERE key = ?1", params![key])
            .map_err(|err| {
                CoreError::Io(
                    "delete preference".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    pub fn list_saved_searches(&self) -> CoreResult<std::collections::BTreeMap<String, String>> {
        let mut searches = std::collections::BTreeMap::new();
        let value = self.get_preference_json("saved_searches")?;
        let Some(value) = value else {
            return Ok(searches);
        };
        let Some(map) = value.as_object() else {
            return Err(CoreError::Io(
                "saved_searches is not a map".to_string(),
                std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid saved_searches"),
            ));
        };
        for (key, value) in map {
            if let Some(query) = value.as_str() {
                searches.insert(key.clone(), query.to_string());
            }
        }
        Ok(searches)
    }

    pub fn add_saved_search(&self, name: &str, query: &str) -> CoreResult<()> {
        let mut searches = self.list_saved_searches()?;
        searches.insert(name.to_string(), query.to_string());
        let mut map = serde_json::Map::new();
        for (key, value) in searches {
            map.insert(key, JsonValue::String(value));
        }
        self.set_preference_json("saved_searches", &JsonValue::Object(map))
    }

    pub fn remove_saved_search(&self, name: &str) -> CoreResult<bool> {
        let mut searches = self.list_saved_searches()?;
        let removed = searches.remove(name).is_some();
        let mut map = serde_json::Map::new();
        for (key, value) in searches {
            map.insert(key, JsonValue::String(value));
        }
        self.set_preference_json("saved_searches", &JsonValue::Object(map))?;
        Ok(removed)
    }

    fn list_category_counts(
        &self,
        table: &str,
        name_column: &str,
        link_table: &str,
        link_column: &str,
        context: &str,
    ) -> CoreResult<Vec<CategoryCount>> {
        let sql = format!(
            "SELECT t.id, t.{name_column}, COUNT(l.id)
             FROM {table} t
             LEFT JOIN {link_table} l ON l.{link_column} = t.id
             GROUP BY t.id
             ORDER BY t.{name_column}"
        );
        let mut stmt = self.conn.prepare(&sql).map_err(|err| {
            CoreError::Io(
                format!("prepare {context}"),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        let rows = stmt.query_map([], |row| {
            Ok(CategoryCount {
                id: row.get(0)?,
                name: row.get(1)?,
                count: row.get(2)?,
            })
        });
        let rows = rows.map_err(|err| {
            CoreError::Io(
                format!("query {context}"),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|err| {
                CoreError::Io(
                    format!("read {context}"),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?);
        }
        Ok(results)
    }

    fn lookup_custom_column_datatype(&self, label: &str) -> CoreResult<String> {
        self.conn
            .query_row(
                "SELECT datatype FROM custom_columns WHERE label = ?1",
                params![label],
                |row| row.get(0),
            )
            .map_err(|err| {
                CoreError::Io(
                    "lookup custom column datatype".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })
    }

    fn create_custom_column_table(&self, label: &str, datatype: &str) -> CoreResult<()> {
        let table_name = format!("custom_column_{label}");
        let column_type = match datatype {
            "int" => "INTEGER",
            "float" => "REAL",
            "bool" => "INTEGER",
            "datetime" => "TEXT",
            _ => "TEXT",
        };
        self.conn
            .execute(
                &format!(
                    "CREATE TABLE IF NOT EXISTS {table_name} (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        book INTEGER NOT NULL UNIQUE,
                        value {column_type} NOT NULL DEFAULT '',
                        FOREIGN KEY(book) REFERENCES books(id)
                    )"
                ),
                [],
            )
            .map_err(|err| {
                CoreError::Io(
                    "create custom column table".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    pub fn query_scalar_string(&self, sql: &str) -> CoreResult<Option<String>> {
        self.conn
            .query_row(sql, [], |row| row.get(0))
            .optional()
            .map_err(|err| {
                CoreError::Io(
                    "query scalar string".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })
    }

    pub fn query_scalar_i64(&self, sql: &str) -> CoreResult<Option<i64>> {
        self.conn
            .query_row(sql, [], |row| row.get(0))
            .optional()
            .map_err(|err| {
                CoreError::Io(
                    "query scalar i64".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })
    }

    pub fn get_books_pages_entry(&self, book_id: i64) -> CoreResult<Option<BooksPagesEntry>> {
        self.conn
            .query_row(
                "SELECT book, pages, algorithm, format, format_size, timestamp, needs_scan
                 FROM books_pages_link WHERE book = ?1",
                params![book_id],
                |row| {
                    let needs_scan: i64 = row.get(6)?;
                    Ok(BooksPagesEntry {
                        book_id: row.get(0)?,
                        pages: row.get(1)?,
                        algorithm: row.get(2)?,
                        format: row.get(3)?,
                        format_size: row.get(4)?,
                        timestamp: row.get(5)?,
                        needs_scan: needs_scan != 0,
                    })
                },
            )
            .optional()
            .map_err(|err| {
                CoreError::Io(
                    "query books pages link".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })
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
                "INSERT OR IGNORE INTO books_authors_link (book, author) VALUES (?1, ?2)",
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
            "DELETE FROM books_authors_link WHERE book = ?1",
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
                "INSERT OR IGNORE INTO books_authors_link (book, author) VALUES (?1, ?2)",
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
                 JOIN books_authors_link bal ON bal.author = a.id
                 WHERE bal.book = ?1
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
                "INSERT OR IGNORE INTO books_tags_link (book, tag) VALUES (?1, ?2)",
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
            "DELETE FROM books_tags_link WHERE book = ?1",
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
                "INSERT OR IGNORE INTO books_tags_link (book, tag) VALUES (?1, ?2)",
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
                 JOIN books_tags_link btl ON btl.tag = t.id
                 WHERE btl.book = ?1
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
            "DELETE FROM books_series_link WHERE book = ?1",
            params![book_id],
        )
        .map_err(|err| {
            CoreError::Io(
                "clear book series link".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        tx.execute(
            "INSERT INTO books_series_link (book, series) VALUES (?1, ?2)",
            params![book_id, series_id],
        )
        .map_err(|err| {
            CoreError::Io(
                "insert book series link".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        tx.execute(
            "UPDATE books SET series_index = ?1 WHERE id = ?2",
            params![index, book_id],
        )
        .map_err(|err| {
            CoreError::Io(
                "update book series index".to_string(),
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
                "DELETE FROM books_series_link WHERE book = ?1",
                params![book_id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "clear book series".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        self.conn
            .execute(
                "UPDATE books SET series_index = 1.0 WHERE id = ?1",
                params![book_id],
            )
            .map_err(|err| {
                CoreError::Io(
                    "reset book series index".to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, err),
                )
            })?;
        Ok(())
    }

    pub fn get_book_series(&self, book_id: i64) -> CoreResult<Option<SeriesEntry>> {
        self.conn
            .query_row(
                "SELECT s.name, b.series_index
                 FROM books_series_link bsl
                 JOIN series s ON s.id = bsl.series
                 JOIN books b ON b.id = bsl.book
                 WHERE bsl.book = ?1",
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
                "INSERT OR REPLACE INTO identifiers (book, type, val)
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
        tx.execute("DELETE FROM identifiers WHERE book = ?1", params![book_id])
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
                "INSERT OR REPLACE INTO identifiers (book, type, val)\n                 VALUES (?1, ?2, ?3)",
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
                "SELECT type, val
                 FROM identifiers
                 WHERE book = ?1
                 ORDER BY type",
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
                "INSERT OR REPLACE INTO comments (book, text) VALUES (?1, ?2)",
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
                "SELECT text FROM comments WHERE book = ?1",
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
            .execute("DELETE FROM comments WHERE book = ?1", params![book_id])
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
            "DELETE FROM books_publishers_link WHERE book = ?1",
            params![book_id],
        )
        .map_err(|err| {
            CoreError::Io(
                "clear book publisher".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        tx.execute(
            "INSERT OR IGNORE INTO books_publishers_link (book, publisher) VALUES (?1, ?2)",
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
                "DELETE FROM books_publishers_link WHERE book = ?1",
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
                "DELETE FROM books_ratings_link WHERE book = ?1",
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
            "DELETE FROM books_ratings_link WHERE book = ?1",
            params![book_id],
        )
        .map_err(|err| {
            CoreError::Io(
                "clear book rating".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;
        tx.execute(
            "INSERT OR IGNORE INTO books_ratings_link (book, rating) VALUES (?1, ?2)",
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
            "DELETE FROM books_languages_link WHERE book = ?1",
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
                "INSERT OR IGNORE INTO books_languages_link (book, lang_code, item_order) VALUES (?1, ?2, ?3)",
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
                 JOIN publishers p ON p.id = bpl.publisher
                 WHERE bpl.book = ?1",
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
                 JOIN ratings r ON r.id = brl.rating
                 WHERE brl.book = ?1",
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
                 JOIN languages l ON l.id = bll.lang_code
                 WHERE bll.book = ?1
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
