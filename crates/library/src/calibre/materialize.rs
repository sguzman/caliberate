//! Calibre-specific extraction for materialization into the canonical catalog.

use super::{CalibreLibraryBackend, path::safe_path, sqlerr};
use caliberate_core::error::{CoreError, CoreResult};
use caliberate_db::database::{
    CanonicalAssetImport, CanonicalBookImport, CanonicalFormatImport,
    CanonicalMaterializeBatchResult, Database,
};
use rusqlite::{Connection, params, params_from_iter, types::Value};
use std::collections::HashMap;
use std::path::Path;

const DEFAULT_PAGE_SIZE: usize = 500;
const ID_CHUNK: usize = 400;

#[derive(Debug, Clone)]
pub struct CalibreMaterializeOptions {
    pub label: Option<String>,
    pub page_size: usize,
    pub stop_after_pages: Option<usize>,
}

impl Default for CalibreMaterializeOptions {
    fn default() -> Self {
        Self {
            label: None,
            page_size: DEFAULT_PAGE_SIZE,
            stop_after_pages: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CalibreMaterializeReport {
    pub source_id: i64,
    pub source_books_seen: usize,
    pub imported_books: usize,
    pub skipped_existing: usize,
    pub metadata_only_books: usize,
    pub logical_formats: usize,
    pub reference_assets: usize,
    pub last_external_id: Option<String>,
    pub completed: bool,
}

pub fn materialize_calibre_source(
    source: &CalibreLibraryBackend,
    target: &mut Database,
    options: CalibreMaterializeOptions,
) -> CoreResult<CalibreMaterializeReport> {
    let page_size = options.page_size.max(1);
    let locator = source.library_root().to_string_lossy().into_owned();
    let source_id =
        target.upsert_library_source("calibre", &locator, options.label.as_deref(), true)?;
    tracing::info!(source_id, library_root=%locator, page_size, "starting Calibre materialization");
    let mut report = CalibreMaterializeReport {
        source_id,
        ..Default::default()
    };
    let mut last_id = 0_i64;
    let mut pages = 0;
    loop {
        let page = read_page(source, last_id, page_size)?;
        if page.is_empty() {
            break;
        }
        last_id = page
            .last()
            .map(|record| record.external_id.parse::<i64>().unwrap_or(last_id))
            .unwrap_or(last_id);
        report.source_books_seen += page.len();
        let seen_at = materialization_timestamp();
        let result = target.materialize_source_books(source_id, &page, &seen_at)?;
        add_batch(&mut report, result);
        pages += 1;
        tracing::info!(source_id, last_external_id=?report.last_external_id, imported=report.imported_books, skipped=report.skipped_existing, "committed Calibre materialization page");
        if options.stop_after_pages.is_some_and(|limit| pages >= limit) {
            return Ok(report);
        }
        if page.len() < page_size {
            break;
        }
    }
    let completed_at = materialization_timestamp();
    target.update_library_source_last_sync(source_id, Some(&completed_at))?;
    report.completed = true;
    tracing::info!(
        source_id,
        imported = report.imported_books,
        skipped = report.skipped_existing,
        "completed Calibre materialization"
    );
    Ok(report)
}

fn add_batch(report: &mut CalibreMaterializeReport, batch: CanonicalMaterializeBatchResult) {
    report.imported_books += batch.imported_books;
    report.skipped_existing += batch.skipped_existing;
    report.metadata_only_books += batch.metadata_only_books;
    report.logical_formats += batch.logical_formats;
    report.reference_assets += batch.reference_assets;
    if batch.last_external_id.is_some() {
        report.last_external_id = batch.last_external_id;
    }
}

fn materialization_timestamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn read_page(
    source: &CalibreLibraryBackend,
    last_id: i64,
    page_size: usize,
) -> CoreResult<Vec<CanonicalBookImport>> {
    let connection = source.connection()?;
    let mut statement = connection
        .prepare(
            "SELECT id,title,sort,timestamp,pubdate,series_index,author_sort,uuid,has_cover,last_modified,path
             FROM books WHERE id > ?1 ORDER BY id ASC LIMIT ?2",
        )
        .map_err(|error| sqlerr("prepare Calibre materialization page", error))?;
    let rows = statement
        .query_map(params![last_id, page_size as i64], |row| {
            Ok(SourceBook {
                id: row.get(0)?,
                title: row.get(1)?,
                sort: row.get(2)?,
                timestamp: row.get(3)?,
                pubdate: row.get(4)?,
                series_index: row.get(5)?,
                author_sort: row.get(6)?,
                uuid: row.get(7)?,
                has_cover: row.get::<_, i64>(8)? != 0,
                last_modified: row.get(9)?,
                path: row.get(10)?,
            })
        })
        .map_err(|error| sqlerr("query Calibre materialization page", error))?;
    let mut books = Vec::new();
    for row in rows {
        books.push(row.map_err(|error| sqlerr("read Calibre materialization book", error))?);
    }
    let ids = books.iter().map(|book| book.id).collect::<Vec<_>>();
    let relations = load_relations(&connection, &ids)?;
    let book_paths = books
        .iter()
        .map(|book| (book.id, book.path.as_str()))
        .collect::<HashMap<_, _>>();
    let formats = load_formats(&connection, &ids, source.library_root(), &book_paths)?;
    books
        .into_iter()
        .map(|book| {
            let id = book.id;
            build_import(book, relations.get(&id), formats.get(&id))
        })
        .collect()
}

#[derive(Debug, Clone)]
struct SourceBook {
    id: i64,
    title: String,
    sort: Option<String>,
    timestamp: Option<String>,
    pubdate: Option<String>,
    series_index: f64,
    author_sort: Option<String>,
    uuid: Option<String>,
    has_cover: bool,
    last_modified: Option<String>,
    path: String,
}

#[derive(Debug, Clone, Default)]
struct Relations {
    authors: Vec<String>,
    tags: Vec<String>,
    series: Option<(String, f64)>,
    publisher: Option<String>,
    rating: Option<i64>,
    languages: Vec<String>,
    identifiers: Vec<(String, String)>,
    comment: Option<String>,
}

fn build_import(
    book: SourceBook,
    relation: Option<&Relations>,
    formats: Option<&Vec<CanonicalFormatImport>>,
) -> CoreResult<CanonicalBookImport> {
    let formats = formats.cloned().unwrap_or_default();
    let primary_format = formats
        .first()
        .map(|format| format.format.clone())
        .unwrap_or_default();
    let primary_path = formats
        .first()
        .and_then(|format| format.representations.first())
        .map(|asset| asset.stored_path.clone())
        .unwrap_or_default();
    let relation = relation.cloned().unwrap_or_default();
    Ok(CanonicalBookImport {
        external_id: book.id.to_string(),
        external_uuid: book.uuid.clone(),
        external_modified: book.last_modified.clone(),
        title: book.title,
        sort: book.sort,
        timestamp: book.timestamp,
        pubdate: book.pubdate,
        series_index: book.series_index,
        author_sort: book.author_sort,
        uuid: book.uuid,
        has_cover: book.has_cover,
        last_modified: book.last_modified,
        authors: relation.authors,
        tags: relation.tags,
        series: relation.series,
        publisher: relation.publisher,
        rating: relation.rating,
        languages: relation.languages,
        identifiers: relation.identifiers,
        comment: relation.comment,
        primary_format,
        primary_path,
        formats,
    })
}

fn id_params(ids: &[i64]) -> (String, Vec<Value>) {
    (
        std::iter::repeat("?")
            .take(ids.len())
            .collect::<Vec<_>>()
            .join(","),
        ids.iter().copied().map(Value::from).collect(),
    )
}

fn batch_rows<T, F>(
    connection: &Connection,
    sql: &str,
    values: Vec<Value>,
    mapper: F,
    label: &str,
) -> CoreResult<Vec<T>>
where
    F: for<'row> FnMut(&rusqlite::Row<'row>) -> rusqlite::Result<T>,
{
    let mut statement = connection
        .prepare(sql)
        .map_err(|e| sqlerr(&format!("prepare Calibre materialization {label}"), e))?;
    statement
        .query_map(params_from_iter(values.into_iter()), mapper)
        .map_err(|e| sqlerr(&format!("query Calibre materialization {label}"), e))?
        .map(|row| row.map_err(|e| sqlerr(&format!("read Calibre materialization {label}"), e)))
        .collect()
}

fn load_relations(connection: &Connection, ids: &[i64]) -> CoreResult<HashMap<i64, Relations>> {
    let mut result = ids
        .iter()
        .map(|id| (*id, Relations::default()))
        .collect::<HashMap<_, _>>();
    for chunk in ids.chunks(ID_CHUNK) {
        let (placeholders, values) = id_params(chunk);
        for (id, name) in batch_rows(
            connection,
            &format!(
                "SELECT x.book,z.name FROM books_authors_link x JOIN authors z ON z.id=x.author WHERE x.book IN ({placeholders}) ORDER BY x.book,x.id"
            ),
            values.clone(),
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
            "authors",
        )? {
            result.get_mut(&id).unwrap().authors.push(name);
        }
        for (id, name) in batch_rows(
            connection,
            &format!(
                "SELECT x.book,z.name FROM books_tags_link x JOIN tags z ON z.id=x.tag WHERE x.book IN ({placeholders}) ORDER BY x.book,x.id"
            ),
            values.clone(),
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
            "tags",
        )? {
            result.get_mut(&id).unwrap().tags.push(name);
        }
        for (id, name, index) in batch_rows(
            connection,
            &format!(
                "SELECT x.book,z.name,b.series_index FROM books_series_link x JOIN series z ON z.id=x.series JOIN books b ON b.id=x.book WHERE x.book IN ({placeholders}) ORDER BY x.book,x.id"
            ),
            values.clone(),
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, f64>(2)?,
                ))
            },
            "series",
        )? {
            result.get_mut(&id).unwrap().series = Some((name, index));
        }
        for (id, name) in batch_rows(
            connection,
            &format!(
                "SELECT x.book,z.name FROM books_publishers_link x JOIN publishers z ON z.id=x.publisher WHERE x.book IN ({placeholders}) ORDER BY x.book,x.id"
            ),
            values.clone(),
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
            "publishers",
        )? {
            result.get_mut(&id).unwrap().publisher = Some(name);
        }
        for (id, rating) in batch_rows(
            connection,
            &format!(
                "SELECT x.book,z.rating FROM books_ratings_link x JOIN ratings z ON z.id=x.rating WHERE x.book IN ({placeholders}) ORDER BY x.book,x.id"
            ),
            values.clone(),
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
            "ratings",
        )? {
            result.get_mut(&id).unwrap().rating = Some(rating);
        }
        for (id, language) in batch_rows(
            connection,
            &format!(
                "SELECT x.book,z.lang_code FROM books_languages_link x JOIN languages z ON z.id=x.lang_code WHERE x.book IN ({placeholders}) ORDER BY x.book,x.item_order,x.id"
            ),
            values.clone(),
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
            "languages",
        )? {
            result.get_mut(&id).unwrap().languages.push(language);
        }
        for (id, kind, value) in batch_rows(
            connection,
            &format!(
                "SELECT book,type,val FROM identifiers WHERE book IN ({placeholders}) ORDER BY book,id"
            ),
            values.clone(),
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
            "identifiers",
        )? {
            result.get_mut(&id).unwrap().identifiers.push((kind, value));
        }
        for (id, comment) in batch_rows(
            connection,
            &format!(
                "SELECT book,text FROM comments WHERE book IN ({placeholders}) ORDER BY book,id"
            ),
            values,
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
            "comments",
        )? {
            result.get_mut(&id).unwrap().comment = Some(comment);
        }
    }
    Ok(result)
}

fn load_formats(
    connection: &Connection,
    ids: &[i64],
    root: &Path,
    book_paths: &HashMap<i64, &str>,
) -> CoreResult<HashMap<i64, Vec<CanonicalFormatImport>>> {
    let mut result = ids
        .iter()
        .map(|id| (*id, Vec::<CanonicalFormatImport>::new()))
        .collect::<HashMap<_, _>>();
    for chunk in ids.chunks(ID_CHUNK) {
        let (placeholders, values) = id_params(chunk);
        let mut statement = connection.prepare(&format!("SELECT id,book,format,uncompressed_size,name FROM data WHERE book IN ({placeholders}) ORDER BY book,id")).map_err(|e|sqlerr("prepare Calibre materialization formats",e))?;
        let rows = statement
            .query_map(params_from_iter(values.into_iter()), |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| sqlerr("query Calibre materialization formats", e))?;
        for row in rows {
            let (_data_id, book_id, raw_format, raw_size, name) =
                row.map_err(|e| sqlerr("read Calibre materialization format", e))?;
            let format = raw_format.to_ascii_lowercase();
            if format.is_empty() {
                return Err(CoreError::ConfigValidate(
                    "Calibre data.format cannot be empty".into(),
                ));
            }
            let book_path = book_paths.get(&book_id).copied().unwrap_or("");
            let path = safe_path(root, book_path, &name, &format)?
                .to_string_lossy()
                .into_owned();
            let entry = result.get_mut(&book_id).unwrap();
            let asset = CanonicalAssetImport {
                storage_mode: "reference".into(),
                stored_path: path,
                source_path: None,
                size_bytes: u64::try_from(raw_size).unwrap_or(0),
                stored_size_bytes: u64::try_from(raw_size).unwrap_or(0),
                checksum: None,
                is_compressed: false,
            };
            if let Some(existing) = entry.iter_mut().find(|item| item.format == format) {
                existing.representations.push(asset);
            } else {
                entry.push(CanonicalFormatImport {
                    format,
                    size_bytes: u64::try_from(raw_size).ok(),
                    representations: vec![asset],
                });
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use caliberate_db::database::Database;
    use rusqlite::Connection;
    use std::fs;
    use tempfile::TempDir;

    fn fixture() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let db = Connection::open(dir.path().join("metadata.db")).unwrap();
        db.execute_batch(
            "CREATE TABLE books(id INTEGER PRIMARY KEY,title TEXT,sort TEXT,timestamp TEXT,pubdate TEXT,series_index REAL,author_sort TEXT,path TEXT,uuid TEXT,has_cover INTEGER,last_modified TEXT);
             CREATE TABLE data(id INTEGER PRIMARY KEY,book INTEGER,format TEXT,uncompressed_size INTEGER,name TEXT);
             CREATE TABLE authors(id INTEGER PRIMARY KEY,name TEXT);
             CREATE TABLE books_authors_link(id INTEGER PRIMARY KEY,book INTEGER,author INTEGER);
             CREATE TABLE tags(id INTEGER PRIMARY KEY,name TEXT);
             CREATE TABLE books_tags_link(id INTEGER PRIMARY KEY,book INTEGER,tag INTEGER);
             CREATE TABLE series(id INTEGER PRIMARY KEY,name TEXT);
             CREATE TABLE books_series_link(id INTEGER PRIMARY KEY,book INTEGER,series INTEGER);
             CREATE TABLE publishers(id INTEGER PRIMARY KEY,name TEXT);
             CREATE TABLE books_publishers_link(id INTEGER PRIMARY KEY,book INTEGER,publisher INTEGER);
             CREATE TABLE ratings(id INTEGER PRIMARY KEY,rating INTEGER);
             CREATE TABLE books_ratings_link(id INTEGER PRIMARY KEY,book INTEGER,rating INTEGER);
             CREATE TABLE languages(id INTEGER PRIMARY KEY,lang_code TEXT);
             CREATE TABLE books_languages_link(id INTEGER PRIMARY KEY,book INTEGER,lang_code INTEGER,item_order INTEGER);
             CREATE TABLE identifiers(id INTEGER PRIMARY KEY,book INTEGER,type TEXT,val TEXT);
             CREATE TABLE comments(id INTEGER PRIMARY KEY,book INTEGER,text TEXT);",
        )
        .unwrap();
        for id in 1..=105_i64 {
            let (title, path) = match id {
                1 => ("Book One", "Author A/Book One (1)"),
                2 => ("Book Two", "Author B/Book Two (2)"),
                _ => ("Synthetic", "Synthetic"),
            };
            db.execute(
                "INSERT INTO books VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![
                    id,
                    title,
                    format!("{title} sort"),
                    "2026-01-01",
                    "2025-01-01",
                    2.5,
                    "sort",
                    path,
                    format!("uuid-{id}"),
                    if id == 1 { 1 } else { 0 },
                    "2026-01-02"
                ],
            )
            .unwrap();
        }
        db.execute(
            "INSERT INTO data VALUES(10,1,'PDF',20,'Book One - Author A')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO data VALUES(11,1,'EPUB',10,'Book One - Author A')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO data VALUES(20,2,'MOBI',30,'Book Two - Author B')",
            [],
        )
        .unwrap();
        db.execute_batch(
            "INSERT INTO authors VALUES(1,'Author A'),(2,'Author B');
             INSERT INTO books_authors_link VALUES(1,1,1),(2,1,2);
             INSERT INTO tags VALUES(1,'fiction'); INSERT INTO books_tags_link VALUES(1,1,1);
             INSERT INTO series VALUES(1,'Series A'); INSERT INTO books_series_link VALUES(1,1,1);
             INSERT INTO publishers VALUES(1,'Publisher A'); INSERT INTO books_publishers_link VALUES(1,1,1);
             INSERT INTO ratings VALUES(1,8); INSERT INTO books_ratings_link VALUES(1,1,1);
             INSERT INTO languages VALUES(1,'en'),(2,'es'); INSERT INTO books_languages_link VALUES(1,1,1,1),(2,1,2,0);
             INSERT INTO identifiers VALUES(1,1,'isbn','abc-1');
             INSERT INTO comments VALUES(1,1,'description');",
        )
        .unwrap();
        dir
    }

    #[test]
    fn materializes_batched_source_without_reading_content_files() {
        let source_dir = fixture();
        let source_bytes = fs::read(source_dir.path().join("metadata.db")).unwrap();
        let source = CalibreLibraryBackend::open(source_dir.path()).unwrap();
        let target_dir = tempfile::tempdir().unwrap();
        let target_path = target_dir.path().join("canonical.db");
        let mut target = Database::open_path(&target_path, 1000).unwrap();
        let report = materialize_calibre_source(
            &source,
            &mut target,
            CalibreMaterializeOptions {
                page_size: 7,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(report.source_books_seen, 105);
        assert_eq!(report.imported_books, 105);
        assert_eq!(report.metadata_only_books, 103);
        assert_eq!(report.logical_formats, 3);
        assert_eq!(report.reference_assets, 3);
        assert!(report.completed);
        assert_eq!(target.list_books().unwrap().len(), 105);
        assert_eq!(target.get_book(1).unwrap().unwrap().format, "pdf");
        assert_eq!(
            target.list_book_authors(1).unwrap(),
            ["Author A", "Author B"]
        );
        assert_eq!(target.list_book_tags(1).unwrap(), ["fiction"]);
        assert_eq!(target.get_book_series(1).unwrap().unwrap().name, "Series A");
        assert_eq!(
            target.get_book_comment(1).unwrap().as_deref(),
            Some("description")
        );
        assert_eq!(
            target
                .list_book_formats(1)
                .unwrap()
                .iter()
                .map(|f| f.format.as_str())
                .collect::<Vec<_>>(),
            ["pdf", "epub"]
        );
        let assets = target.list_assets_for_book(1).unwrap();
        assert_eq!(assets.len(), 2);
        assert!(
            assets.iter().all(|asset| asset.storage_mode == "reference"
                && asset.source_id == Some(report.source_id))
        );
        assert!(assets.iter().all(|asset| !asset.stored_path.is_empty()
            && !std::path::Path::new(&asset.stored_path).exists()));
        let original_title = target.get_book(1).unwrap().unwrap().title;
        target.update_book_title(1, "Local edit").unwrap();
        let second = materialize_calibre_source(
            &source,
            &mut target,
            CalibreMaterializeOptions {
                page_size: 11,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(second.imported_books, 0);
        assert_eq!(second.skipped_existing, 105);
        assert_eq!(target.get_book(1).unwrap().unwrap().title, "Local edit");
        assert_ne!(original_title, "Local edit");
        drop(source);
        assert_eq!(target.list_books().unwrap().len(), 105);
        assert!(
            target
                .get_library_source(report.source_id)
                .unwrap()
                .unwrap()
                .last_sync_at
                .is_some()
        );
        assert_eq!(
            fs::read(source_dir.path().join("metadata.db")).unwrap(),
            source_bytes
        );
    }

    #[test]
    fn materialization_rejects_unsafe_metadata_path_without_file_access() {
        let source_dir = fixture();
        let db = Connection::open(source_dir.path().join("metadata.db")).unwrap();
        db.execute("UPDATE books SET path='../escape' WHERE id=1", [])
            .unwrap();
        drop(db);
        let source = CalibreLibraryBackend::open(source_dir.path()).unwrap();
        let target_dir = tempfile::tempdir().unwrap();
        let mut target = Database::open_path(target_dir.path().join("canonical.db"), 1000).unwrap();
        assert!(materialize_calibre_source(&source, &mut target, Default::default()).is_err());
        assert!(!target_dir.path().join("escape").exists());
    }

    #[test]
    fn materialization_resumes_after_a_committed_page() {
        let source_dir = fixture();
        let source = CalibreLibraryBackend::open(source_dir.path()).unwrap();
        let target_dir = tempfile::tempdir().unwrap();
        let mut target = Database::open_path(target_dir.path().join("canonical.db"), 1000).unwrap();
        let partial = materialize_calibre_source(
            &source,
            &mut target,
            CalibreMaterializeOptions {
                page_size: 10,
                stop_after_pages: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!partial.completed);
        assert_eq!(partial.imported_books, 10);
        let complete = materialize_calibre_source(
            &source,
            &mut target,
            CalibreMaterializeOptions {
                page_size: 10,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(complete.completed);
        assert_eq!(complete.imported_books, 95);
        assert_eq!(complete.skipped_existing, 10);
        assert_eq!(target.list_books().unwrap().len(), 105);
    }
}
