//! Canonical catalog provenance, logical formats, and format-aware assets.

use super::{Database, sqlite_error};
use caliberate_core::error::{CoreError, CoreResult};
use rusqlite::types::Value;
use rusqlite::{OptionalExtension, params, params_from_iter};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibrarySourceRow {
    pub id: i64,
    pub kind: String,
    pub locator: String,
    pub label: Option<String>,
    pub read_only: bool,
    pub created_at: String,
    pub last_sync_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBookRow {
    pub id: i64,
    pub source_id: i64,
    pub book_id: i64,
    pub external_id: String,
    pub external_uuid: Option<String>,
    pub external_modified: Option<String>,
    pub imported_at: String,
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookFormatRow {
    pub id: i64,
    pub book_id: i64,
    pub format: String,
    pub size_bytes: Option<u64>,
    pub created_at: String,
}

impl Database {
    pub(super) fn backfill_canonical_formats(&self) -> CoreResult<()> {
        self.conn
            .execute(
                "INSERT INTO book_formats (book_id, format, size_bytes)
                 SELECT id, LOWER(format), NULL FROM books WHERE format <> ''
                 ON CONFLICT(book_id, format) DO NOTHING",
                [],
            )
            .map_err(|err| sqlite_error("backfill book formats", err))?;
        self.conn
            .execute(
                "UPDATE assets
                 SET book_format_id = (
                     SELECT bf.id FROM book_formats bf
                     JOIN books b ON b.id = assets.book_id
                     WHERE bf.book_id = assets.book_id
                       AND bf.format = LOWER(b.format)
                 )
                 WHERE book_format_id IS NULL
                   AND EXISTS (
                     SELECT 1 FROM books b
                     WHERE b.id = assets.book_id AND b.format <> ''
                   )",
                [],
            )
            .map_err(|err| sqlite_error("link existing assets to book formats", err))?;
        self.conn
            .execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_assets_book_format_id ON assets(book_format_id);
                 CREATE INDEX IF NOT EXISTS idx_assets_source_id ON assets(source_id);",
            )
            .map_err(|err| sqlite_error("index canonical asset links", err))?;
        Ok(())
    }
    pub fn upsert_library_source(
        &self,
        kind: &str,
        locator: &str,
        label: Option<&str>,
        read_only: bool,
    ) -> CoreResult<i64> {
        self.conn
            .execute(
                "INSERT INTO library_sources (kind, locator, label, read_only)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(kind, locator) DO UPDATE SET label=excluded.label, read_only=excluded.read_only",
                params![kind, locator, label, if read_only { 1 } else { 0 }],
            )
            .map_err(|err| sqlite_error("upsert library source", err))?;
        self.conn
            .query_row(
                "SELECT id FROM library_sources WHERE kind=?1 AND locator=?2",
                params![kind, locator],
                |row| row.get(0),
            )
            .map_err(|err| sqlite_error("read library source id", err))
    }

    pub fn get_library_source(&self, id: i64) -> CoreResult<Option<LibrarySourceRow>> {
        self.conn
            .query_row(
                "SELECT id,kind,locator,label,read_only,created_at,last_sync_at FROM library_sources WHERE id=?1",
                params![id],
                |row| {
                    Ok(LibrarySourceRow {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        locator: row.get(2)?,
                        label: row.get(3)?,
                        read_only: row.get::<_, i64>(4)? != 0,
                        created_at: row.get(5)?,
                        last_sync_at: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(|err| sqlite_error("get library source", err))
    }

    pub fn find_library_source(
        &self,
        kind: &str,
        locator: &str,
    ) -> CoreResult<Option<LibrarySourceRow>> {
        self.conn
            .query_row(
                "SELECT id,kind,locator,label,read_only,created_at,last_sync_at FROM library_sources WHERE kind=?1 AND locator=?2",
                params![kind, locator],
                |row| {
                    Ok(LibrarySourceRow {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        locator: row.get(2)?,
                        label: row.get(3)?,
                        read_only: row.get::<_, i64>(4)? != 0,
                        created_at: row.get(5)?,
                        last_sync_at: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(|err| sqlite_error("find library source", err))
    }

    pub fn list_library_sources(&self) -> CoreResult<Vec<LibrarySourceRow>> {
        let mut statement = self.conn.prepare(
            "SELECT id,kind,locator,label,read_only,created_at,last_sync_at FROM library_sources ORDER BY id",
        ).map_err(|err| sqlite_error("prepare library sources", err))?;
        let rows = statement
            .query_map([], |row| {
                Ok(LibrarySourceRow {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    locator: row.get(2)?,
                    label: row.get(3)?,
                    read_only: row.get::<_, i64>(4)? != 0,
                    created_at: row.get(5)?,
                    last_sync_at: row.get(6)?,
                })
            })
            .map_err(|err| sqlite_error("query library sources", err))?;
        rows.map(|row| row.map_err(|err| sqlite_error("read library source", err)))
            .collect()
    }

    pub fn update_library_source_last_sync(
        &self,
        id: i64,
        timestamp: Option<&str>,
    ) -> CoreResult<()> {
        self.conn
            .execute(
                "UPDATE library_sources SET last_sync_at=?1 WHERE id=?2",
                params![timestamp, id],
            )
            .map_err(|err| sqlite_error("update library source sync time", err))?;
        Ok(())
    }

    pub fn upsert_source_book(
        &self,
        source_id: i64,
        book_id: i64,
        external_id: &str,
        external_uuid: Option<&str>,
        external_modified: Option<&str>,
        last_seen_at: Option<&str>,
    ) -> CoreResult<i64> {
        self.conn.execute(
            "INSERT INTO source_books (source_id,book_id,external_id,external_uuid,external_modified,last_seen_at)
             VALUES (?1,?2,?3,?4,?5,?6)
             ON CONFLICT(source_id,external_id) DO UPDATE SET book_id=excluded.book_id,external_uuid=excluded.external_uuid,external_modified=excluded.external_modified,last_seen_at=excluded.last_seen_at",
            params![source_id, book_id, external_id, external_uuid, external_modified, last_seen_at],
        ).map_err(|err| sqlite_error("upsert source book", err))?;
        self.conn
            .query_row(
                "SELECT id FROM source_books WHERE source_id=?1 AND external_id=?2",
                params![source_id, external_id],
                |row| row.get(0),
            )
            .map_err(|err| sqlite_error("read source book id", err))
    }

    pub fn get_source_book(
        &self,
        source_id: i64,
        external_id: &str,
    ) -> CoreResult<Option<SourceBookRow>> {
        self.conn.query_row(
            "SELECT id,source_id,book_id,external_id,external_uuid,external_modified,imported_at,last_seen_at FROM source_books WHERE source_id=?1 AND external_id=?2",
            params![source_id, external_id], |row| self.source_book_from_row(row)).optional()
            .map_err(|err| sqlite_error("get source book", err))
    }

    pub fn list_source_books(&self, source_id: i64) -> CoreResult<Vec<SourceBookRow>> {
        let mut statement = self.conn.prepare("SELECT id,source_id,book_id,external_id,external_uuid,external_modified,imported_at,last_seen_at FROM source_books WHERE source_id=?1 ORDER BY id")
            .map_err(|err| sqlite_error("prepare source books", err))?;
        let rows = statement
            .query_map(params![source_id], |row| self.source_book_from_row(row))
            .map_err(|err| sqlite_error("query source books", err))?;
        rows.map(|row| row.map_err(|err| sqlite_error("read source book", err)))
            .collect()
    }

    fn source_book_from_row(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<SourceBookRow> {
        Ok(SourceBookRow {
            id: row.get(0)?,
            source_id: row.get(1)?,
            book_id: row.get(2)?,
            external_id: row.get(3)?,
            external_uuid: row.get(4)?,
            external_modified: row.get(5)?,
            imported_at: row.get(6)?,
            last_seen_at: row.get(7)?,
        })
    }

    pub fn upsert_book_format(
        &self,
        book_id: i64,
        format: &str,
        size_bytes: Option<u64>,
    ) -> CoreResult<i64> {
        let format = format.to_ascii_lowercase();
        if format.is_empty() {
            return Err(CoreError::ConfigValidate(
                "book format cannot be empty".into(),
            ));
        }
        let exists = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM books WHERE id=?1)",
                params![book_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|err| sqlite_error("check book for format", err))?;
        if exists == 0 {
            return Err(CoreError::ConfigValidate(format!(
                "book {book_id} does not exist"
            )));
        }
        self.conn.execute(
            "INSERT INTO book_formats (book_id,format,size_bytes) VALUES (?1,?2,?3)
             ON CONFLICT(book_id,format) DO UPDATE SET size_bytes=COALESCE(excluded.size_bytes,book_formats.size_bytes)",
            params![book_id, format, size_bytes.map(|size| size as i64)],
        ).map_err(|err| sqlite_error("upsert book format", err))?;
        self.conn
            .query_row(
                "SELECT id FROM book_formats WHERE book_id=?1 AND format=?2",
                params![book_id, format],
                |row| row.get(0),
            )
            .map_err(|err| sqlite_error("read book format id", err))
    }

    pub fn get_book_format(&self, book_id: i64, format: &str) -> CoreResult<Option<BookFormatRow>> {
        self.conn.query_row("SELECT id,book_id,format,size_bytes,created_at FROM book_formats WHERE book_id=?1 AND format=?2 COLLATE NOCASE", params![book_id, format.to_ascii_lowercase()], |row| self.book_format_from_row(row)).optional()
            .map_err(|err| sqlite_error("get book format", err))
    }

    pub fn list_book_formats(&self, book_id: i64) -> CoreResult<Vec<BookFormatRow>> {
        self.list_book_formats_for_books(&[book_id])
            .map(|mut rows| rows.remove(&book_id).unwrap_or_default())
    }

    pub fn list_book_formats_for_books(
        &self,
        book_ids: &[i64],
    ) -> CoreResult<HashMap<i64, Vec<BookFormatRow>>> {
        let mut result = book_ids
            .iter()
            .map(|id| (*id, Vec::new()))
            .collect::<HashMap<_, _>>();
        const CHUNK: usize = 400;
        for chunk in book_ids.chunks(CHUNK) {
            let placeholders = std::iter::repeat("?")
                .take(chunk.len())
                .collect::<Vec<_>>()
                .join(",");
            let values = chunk.iter().copied().map(Value::from).collect::<Vec<_>>();
            let mut statement = self.conn.prepare(&format!("SELECT id,book_id,format,size_bytes,created_at FROM book_formats WHERE book_id IN ({placeholders}) ORDER BY book_id,id"))
                .map_err(|err| sqlite_error("prepare book format batch", err))?;
            let rows = statement
                .query_map(params_from_iter(values.into_iter()), |row| {
                    self.book_format_from_row(row)
                })
                .map_err(|err| sqlite_error("query book format batch", err))?;
            for row in rows {
                let format = row.map_err(|err| sqlite_error("read book format batch", err))?;
                result.entry(format.book_id).or_default().push(format);
            }
        }

        Ok(result)
    }

    fn book_format_from_row(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<BookFormatRow> {
        Ok(BookFormatRow {
            id: row.get(0)?,
            book_id: row.get(1)?,
            format: row.get(2)?,
            size_bytes: row
                .get::<_, Option<i64>>(3)?
                .and_then(|size| u64::try_from(size).ok()),
            created_at: row.get(4)?,
        })
    }

    pub fn remove_book_format(&self, book_id: i64, format: &str) -> CoreResult<()> {
        let Some(row) = self.get_book_format(book_id, format)? else {
            return Ok(());
        };
        let linked = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM assets WHERE book_format_id=?1)",
                params![row.id],
                |value| value.get::<_, i64>(0),
            )
            .map_err(|err| sqlite_error("check linked assets for format", err))?;
        if linked != 0 {
            return Err(CoreError::ConfigValidate(format!(
                "book format {format} has linked assets"
            )));
        }
        self.conn
            .execute("DELETE FROM book_formats WHERE id=?1", params![row.id])
            .map_err(|err| sqlite_error("remove book format", err))?;
        Ok(())
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
        let format: Option<String> = self
            .conn
            .query_row(
                "SELECT NULLIF(format,'') FROM books WHERE id=?1",
                params![book_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(|err| sqlite_error("read asset book format", err))?
            .flatten();
        let book_format_id = format
            .as_deref()
            .map(|format| self.upsert_book_format(book_id, format, None))
            .transpose()?;
        self.insert_asset(
            book_id,
            book_format_id,
            None,
            storage_mode,
            stored_path,
            source_path,
            size_bytes,
            stored_size_bytes,
            checksum,
            is_compressed,
            created_at,
        )
    }

    pub fn add_asset_for_format(
        &self,
        book_id: i64,
        book_format_id: i64,
        source_id: Option<i64>,
        storage_mode: &str,
        stored_path: &str,
        source_path: Option<&str>,
        size_bytes: u64,
        stored_size_bytes: u64,
        checksum: Option<&str>,
        is_compressed: bool,
        created_at: &str,
    ) -> CoreResult<i64> {
        let valid = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM book_formats WHERE id=?1 AND book_id=?2)",
                params![book_format_id, book_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|err| sqlite_error("validate asset book format", err))?;
        if valid == 0 {
            return Err(CoreError::ConfigValidate(
                "asset format does not belong to book".into(),
            ));
        }
        self.insert_asset(
            book_id,
            Some(book_format_id),
            source_id,
            storage_mode,
            stored_path,
            source_path,
            size_bytes,
            stored_size_bytes,
            checksum,
            is_compressed,
            created_at,
        )
    }

    fn insert_asset(
        &self,
        book_id: i64,
        book_format_id: Option<i64>,
        source_id: Option<i64>,
        storage_mode: &str,
        stored_path: &str,
        source_path: Option<&str>,
        size_bytes: u64,
        stored_size_bytes: u64,
        checksum: Option<&str>,
        is_compressed: bool,
        created_at: &str,
    ) -> CoreResult<i64> {
        if let Some(source_id) = source_id {
            let valid = self
                .conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM library_sources WHERE id=?1)",
                    params![source_id],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(|err| sqlite_error("validate asset source", err))?;
            if valid == 0 {
                return Err(CoreError::ConfigValidate(
                    "asset source does not exist".into(),
                ));
            }
        }
        self.conn.execute(
            "INSERT INTO assets (book_id,book_format_id,source_id,storage_mode,stored_path,source_path,size_bytes,stored_size_bytes,checksum,is_compressed,created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![book_id, book_format_id, source_id, storage_mode, stored_path, source_path, size_bytes as i64, stored_size_bytes as i64, checksum, if is_compressed { 1 } else { 0 }, created_at],
        ).map_err(|err| sqlite_error("insert asset", err))?;
        Ok(self.conn.last_insert_rowid())
    }
}
