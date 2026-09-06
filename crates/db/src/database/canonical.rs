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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CanonicalAssetImport {
    pub storage_mode: String,
    pub stored_path: String,
    pub source_path: Option<String>,
    pub size_bytes: u64,
    pub stored_size_bytes: u64,
    pub checksum: Option<String>,
    pub is_compressed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CanonicalFormatImport {
    pub format: String,
    pub size_bytes: Option<u64>,
    pub representations: Vec<CanonicalAssetImport>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CanonicalBookImport {
    pub external_id: String,
    pub external_uuid: Option<String>,
    pub external_modified: Option<String>,
    pub title: String,
    pub sort: Option<String>,
    pub timestamp: Option<String>,
    pub pubdate: Option<String>,
    pub series_index: f64,
    pub author_sort: Option<String>,
    pub uuid: Option<String>,
    pub has_cover: bool,
    pub last_modified: Option<String>,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub series: Option<(String, f64)>,
    pub publisher: Option<String>,
    pub rating: Option<i64>,
    pub languages: Vec<String>,
    pub identifiers: Vec<(String, String)>,
    pub comment: Option<String>,
    pub primary_format: String,
    pub primary_path: String,
    pub formats: Vec<CanonicalFormatImport>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CanonicalMaterializeBatchResult {
    pub imported_books: usize,
    pub skipped_existing: usize,
    pub metadata_only_books: usize,
    pub logical_formats: usize,
    pub reference_assets: usize,
    pub last_external_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAuditCounts {
    pub source_id: i64,
    pub mapped_books: u64,
    pub source_reference_assets: u64,
    pub source_backed_formats: u64,
    pub managed_backed_formats: u64,
    pub source_dependent_formats: u64,
    pub metadata_only_source_books: u64,
    pub fully_managed_source_books: u64,
    pub source_books_with_dependencies: u64,
    pub unlinked_source_assets: u64,
    pub orphan_source_assets: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceManagedCandidate {
    pub book_id: i64,
    pub book_format_id: i64,
    pub format: String,
    pub asset_id: i64,
    pub stored_path: String,
    pub size_bytes: u64,
    pub stored_size_bytes: u64,
    pub checksum: Option<String>,
    pub is_compressed: bool,
}

impl Database {
    pub fn audit_source_counts(&self, source_id: i64) -> CoreResult<SourceAuditCounts> {
        self.conn.query_row(
            "WITH
             mapped AS (SELECT book_id FROM source_books WHERE source_id=?1),
             refs AS (SELECT a.* FROM assets a WHERE a.source_id=?1 AND a.storage_mode='reference'),
             backed AS (SELECT DISTINCT book_format_id FROM refs WHERE book_format_id IS NOT NULL),
             managed AS (SELECT DISTINCT a.book_format_id FROM assets a JOIN backed b ON b.book_format_id=a.book_format_id WHERE a.storage_mode='copy' AND a.source_id IS NULL),
             dependent AS (SELECT book_format_id FROM backed EXCEPT SELECT book_format_id FROM managed),
             mapped_formats AS (SELECT m.book_id, COUNT(DISTINCT r.book_format_id) AS formats, COUNT(DISTINCT d.book_format_id) AS dependent FROM mapped m LEFT JOIN refs r ON r.book_id=m.book_id AND r.book_format_id IS NOT NULL LEFT JOIN dependent d ON d.book_format_id=r.book_format_id GROUP BY m.book_id),
             unlinked AS (SELECT COUNT(*) AS n FROM refs WHERE book_format_id IS NULL),
             orphaned AS (SELECT COUNT(*) AS n FROM assets a WHERE a.source_id=?1 AND NOT EXISTS (SELECT 1 FROM source_books sb WHERE sb.source_id=?1 AND sb.book_id=a.book_id))
             SELECT
               (SELECT COUNT(*) FROM mapped), (SELECT COUNT(*) FROM refs),
               (SELECT COUNT(*) FROM backed), (SELECT COUNT(*) FROM managed),
               (SELECT COUNT(*) FROM dependent),
               (SELECT COUNT(*) FROM mapped_formats WHERE formats=0),
               (SELECT COUNT(*) FROM mapped_formats WHERE formats>0 AND dependent=0),
               (SELECT COUNT(*) FROM mapped_formats WHERE dependent>0),
               (SELECT n FROM unlinked), (SELECT n FROM orphaned)",
            params![source_id],
            |row| {
                Ok(SourceAuditCounts {
                    source_id,
                    mapped_books: count_u64(row, 0)?,
                    source_reference_assets: count_u64(row, 1)?,
                    source_backed_formats: count_u64(row, 2)?,
                    managed_backed_formats: count_u64(row, 3)?,
                    source_dependent_formats: count_u64(row, 4)?,
                    metadata_only_source_books: count_u64(row, 5)?,
                    fully_managed_source_books: count_u64(row, 6)?,
                    source_books_with_dependencies: count_u64(row, 7)?,
                    unlinked_source_assets: count_u64(row, 8)?,
                    orphan_source_assets: count_u64(row, 9)?,
                })
            },
        ).map_err(|err| sqlite_error("audit source counts", err))
    }

    pub fn list_source_managed_candidates(
        &self,
        source_id: i64,
        after: Option<(i64, i64)>,
        page_size: usize,
    ) -> CoreResult<Vec<SourceManagedCandidate>> {
        let limit = page_size.clamp(1, 500) as i64;
        let (cursor_clause, params): (String, Vec<Value>) =
            if let Some((format_id, asset_id)) = after {
                (
                    "AND (a.book_format_id > ?2 OR (a.book_format_id = ?2 AND a.id > ?3))".into(),
                    vec![
                        Value::from(source_id),
                        Value::from(format_id),
                        Value::from(asset_id),
                    ],
                )
            } else {
                (String::new(), vec![Value::from(source_id)])
            };
        let sql = format!(
            "WITH source_formats AS (
                 SELECT DISTINCT book_format_id FROM assets
                 WHERE source_id=?1 AND storage_mode='reference' AND book_format_id IS NOT NULL
             )
             SELECT a.book_id,a.book_format_id,bf.format,a.id,a.stored_path,a.size_bytes,
                    a.stored_size_bytes,a.checksum,a.is_compressed
             FROM assets a JOIN source_formats sf ON sf.book_format_id=a.book_format_id
             JOIN book_formats bf ON bf.id=a.book_format_id
             WHERE a.storage_mode='copy' AND a.source_id IS NULL
               AND NOT EXISTS (
                 SELECT 1 FROM assets earlier
                 WHERE earlier.book_format_id=a.book_format_id
                   AND earlier.storage_mode='copy' AND earlier.source_id IS NULL
                   AND earlier.id < a.id
               ) {cursor_clause}
             ORDER BY a.book_format_id,a.id LIMIT ?{}",
            params.len() + 1
        );
        let mut values = params;
        values.push(Value::from(limit));
        let mut statement = self
            .conn
            .prepare(&sql)
            .map_err(|err| sqlite_error("prepare source managed candidates", err))?;
        let rows = statement
            .query_map(params_from_iter(values), |row| {
                Ok(SourceManagedCandidate {
                    book_id: row.get(0)?,
                    book_format_id: row.get(1)?,
                    format: row.get(2)?,
                    asset_id: row.get(3)?,
                    stored_path: row.get(4)?,
                    size_bytes: u64::try_from(row.get::<_, i64>(5)?).unwrap_or_default(),
                    stored_size_bytes: u64::try_from(row.get::<_, i64>(6)?).unwrap_or_default(),
                    checksum: row.get(7)?,
                    is_compressed: row.get::<_, i64>(8)? != 0,
                })
            })
            .map_err(|err| sqlite_error("query source managed candidates", err))?;
        rows.map(|row| row.map_err(|err| sqlite_error("read source managed candidate", err)))
            .collect()
    }

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

    pub fn materialize_source_books(
        &mut self,
        source_id: i64,
        records: &[CanonicalBookImport],
        seen_at: &str,
    ) -> CoreResult<CanonicalMaterializeBatchResult> {
        let tx = self
            .conn
            .transaction()
            .map_err(|err| sqlite_error("begin canonical materialization chunk", err))?;
        let mut result = CanonicalMaterializeBatchResult::default();
        for record in records {
            result.last_external_id = Some(record.external_id.clone());
            let existing: Option<i64> = tx
                .query_row(
                    "SELECT id FROM source_books WHERE source_id=?1 AND external_id=?2",
                    params![source_id, record.external_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|err| sqlite_error("check materialized source book", err))?;
            if existing.is_some() {
                result.skipped_existing += 1;
                continue;
            }

            let primary_format = record.primary_format.to_ascii_lowercase();
            tx.execute(
                "INSERT INTO books (title,sort,timestamp,pubdate,series_index,author_sort,uuid,has_cover,last_modified,format,path,created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                params![
                    record.title,
                    record.sort,
                    record.timestamp,
                    record.pubdate,
                    record.series_index,
                    record.author_sort,
                    record.uuid,
                    if record.has_cover { 1 } else { 0 },
                    record.last_modified.as_deref().unwrap_or(""),
                    primary_format,
                    record.primary_path,
                    seen_at,
                ],
            )
            .map_err(|err| sqlite_error("insert materialized book", err))?;
            let book_id = tx.last_insert_rowid();
            if let Some(sort) = &record.sort {
                tx.execute(
                    "UPDATE books SET sort=?1 WHERE id=?2",
                    params![sort, book_id],
                )
                .map_err(|err| sqlite_error("set materialized book sort", err))?;
            }
            if let Some(uuid) = &record.uuid {
                tx.execute(
                    "UPDATE books SET uuid=?1 WHERE id=?2",
                    params![uuid, book_id],
                )
                .map_err(|err| sqlite_error("set materialized book uuid", err))?;
            }
            materialize_relations(&tx, book_id, record)?;
            tx.execute(
                "INSERT INTO source_books (source_id,book_id,external_id,external_uuid,external_modified,last_seen_at)
                 VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    source_id,
                    book_id,
                    record.external_id,
                    record.external_uuid,
                    record.external_modified,
                    seen_at,
                ],
            )
            .map_err(|err| sqlite_error("insert materialized source mapping", err))?;

            if record.formats.is_empty() {
                result.metadata_only_books += 1;
            }
            for format in &record.formats {
                let normalized = format.format.to_ascii_lowercase();
                if normalized.is_empty() {
                    continue;
                }
                tx.execute(
                    "INSERT INTO book_formats (book_id,format,size_bytes) VALUES (?1,?2,?3)",
                    params![
                        book_id,
                        normalized,
                        format.size_bytes.map(|size| size as i64)
                    ],
                )
                .map_err(|err| sqlite_error("insert materialized book format", err))?;
                let format_id = tx.last_insert_rowid();
                result.logical_formats += 1;
                for asset in &format.representations {
                    tx.execute(
                        "INSERT INTO assets (book_id,book_format_id,source_id,storage_mode,stored_path,source_path,size_bytes,stored_size_bytes,checksum,is_compressed,created_at)
                         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                        params![
                            book_id,
                            format_id,
                            source_id,
                            "reference",
                            asset.stored_path,
                            Option::<&str>::None,
                            asset.size_bytes as i64,
                            asset.stored_size_bytes as i64,
                            Option::<&str>::None,
                            0,
                            seen_at,
                        ],
                    )
                    .map_err(|err| sqlite_error("insert materialized reference asset", err))?;
                    result.reference_assets += 1;
                }
            }
            result.imported_books += 1;
        }
        tx.commit()
            .map_err(|err| sqlite_error("commit canonical materialization chunk", err))?;
        Ok(result)
    }
}

fn count_u64(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    u64::try_from(row.get::<_, i64>(index)?).map_err(|_| rusqlite::Error::InvalidQuery)
}

#[cfg(test)]
mod audit_tests {
    use super::super::Database;
    use caliberate_core::config::ControlPlane;
    use rusqlite::params;

    #[test]
    fn orphan_count_includes_non_reference_source_assets() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../config/control-plane.toml");
        let mut config = ControlPlane::load_from_path(config_path).unwrap();
        config.db.sqlite_path = dir.path().join("library.db");
        let db = Database::open_with_fts(&config.db, &config.fts).unwrap();
        let source_id = db
            .upsert_library_source("fixture", "never-open", None, true)
            .unwrap();
        db.add_book("Orphan", "", "", "2026").unwrap();
        db.conn
            .execute(
                "INSERT INTO assets (book_id,book_format_id,source_id,storage_mode,stored_path,source_path,size_bytes,stored_size_bytes,checksum,is_compressed,created_at)
                 VALUES (1,NULL,?1,'copy','never-open',NULL,0,0,NULL,0,'2026')",
                params![source_id],
            )
            .unwrap();
        let counts = db.audit_source_counts(source_id).unwrap();
        assert_eq!(counts.source_reference_assets, 0);
        assert_eq!(counts.source_backed_formats, 0);
        assert_eq!(counts.orphan_source_assets, 1);
    }
}

fn materialize_relations(
    tx: &rusqlite::Transaction<'_>,
    book_id: i64,
    record: &CanonicalBookImport,
) -> CoreResult<()> {
    for author in &record.authors {
        tx.execute(
            "INSERT OR IGNORE INTO authors (name) VALUES (?1)",
            params![author],
        )
        .map_err(|err| sqlite_error("insert materialized author", err))?;
        let id: i64 = tx
            .query_row(
                "SELECT id FROM authors WHERE name=?1",
                params![author],
                |row| row.get(0),
            )
            .map_err(|err| sqlite_error("read materialized author", err))?;
        tx.execute(
            "INSERT OR IGNORE INTO books_authors_link (book,author) VALUES (?1,?2)",
            params![book_id, id],
        )
        .map_err(|err| sqlite_error("link materialized author", err))?;
    }
    for tag in &record.tags {
        tx.execute(
            "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
            params![tag],
        )
        .map_err(|err| sqlite_error("insert materialized tag", err))?;
        let id: i64 = tx
            .query_row("SELECT id FROM tags WHERE name=?1", params![tag], |row| {
                row.get(0)
            })
            .map_err(|err| sqlite_error("read materialized tag", err))?;
        tx.execute(
            "INSERT OR IGNORE INTO books_tags_link (book,tag) VALUES (?1,?2)",
            params![book_id, id],
        )
        .map_err(|err| sqlite_error("link materialized tag", err))?;
    }
    if let Some((name, index)) = &record.series {
        tx.execute(
            "INSERT OR IGNORE INTO series (name) VALUES (?1)",
            params![name],
        )
        .map_err(|err| sqlite_error("insert materialized series", err))?;
        let id: i64 = tx
            .query_row(
                "SELECT id FROM series WHERE name=?1",
                params![name],
                |row| row.get(0),
            )
            .map_err(|err| sqlite_error("read materialized series", err))?;
        tx.execute(
            "INSERT INTO books_series_link (book,series) VALUES (?1,?2)",
            params![book_id, id],
        )
        .map_err(|err| sqlite_error("link materialized series", err))?;
        tx.execute(
            "UPDATE books SET series_index=?1 WHERE id=?2",
            params![index, book_id],
        )
        .map_err(|err| sqlite_error("set materialized series index", err))?;
    }
    if let Some(publisher) = &record.publisher {
        tx.execute(
            "INSERT OR IGNORE INTO publishers (name) VALUES (?1)",
            params![publisher],
        )
        .map_err(|err| sqlite_error("insert materialized publisher", err))?;
        let id: i64 = tx
            .query_row(
                "SELECT id FROM publishers WHERE name=?1",
                params![publisher],
                |row| row.get(0),
            )
            .map_err(|err| sqlite_error("read materialized publisher", err))?;
        tx.execute(
            "INSERT INTO books_publishers_link (book,publisher) VALUES (?1,?2)",
            params![book_id, id],
        )
        .map_err(|err| sqlite_error("link materialized publisher", err))?;
    }
    if let Some(rating) = record.rating {
        tx.execute(
            "INSERT OR IGNORE INTO ratings (rating) VALUES (?1)",
            params![rating],
        )
        .map_err(|err| sqlite_error("insert materialized rating", err))?;
        let id: i64 = tx
            .query_row(
                "SELECT id FROM ratings WHERE rating=?1",
                params![rating],
                |row| row.get(0),
            )
            .map_err(|err| sqlite_error("read materialized rating", err))?;
        tx.execute(
            "INSERT INTO books_ratings_link (book,rating) VALUES (?1,?2)",
            params![book_id, id],
        )
        .map_err(|err| sqlite_error("link materialized rating", err))?;
    }
    for (order, language) in record.languages.iter().enumerate() {
        tx.execute(
            "INSERT OR IGNORE INTO languages (lang_code) VALUES (?1)",
            params![language],
        )
        .map_err(|err| sqlite_error("insert materialized language", err))?;
        let id: i64 = tx
            .query_row(
                "SELECT id FROM languages WHERE lang_code=?1",
                params![language],
                |row| row.get(0),
            )
            .map_err(|err| sqlite_error("read materialized language", err))?;
        tx.execute(
            "INSERT INTO books_languages_link (book,lang_code,item_order) VALUES (?1,?2,?3)",
            params![book_id, id, order as i64],
        )
        .map_err(|err| sqlite_error("link materialized language", err))?;
    }
    for (kind, value) in &record.identifiers {
        tx.execute(
            "INSERT OR REPLACE INTO identifiers (book,type,val) VALUES (?1,?2,?3)",
            params![book_id, kind, value],
        )
        .map_err(|err| sqlite_error("insert materialized identifier", err))?;
    }
    if let Some(comment) = &record.comment {
        tx.execute(
            "INSERT OR REPLACE INTO comments (book,text) VALUES (?1,?2)",
            params![book_id, comment],
        )
        .map_err(|err| sqlite_error("insert materialized comment", err))?;
    }
    Ok(())
}
