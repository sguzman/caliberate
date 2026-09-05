//! Read-only adapter for an attached modern Calibre library.
use crate::catalog::{LibraryBackend, LibraryBook, LibraryContent};
use crate::query::{
    LibraryFacetKind, LibraryFacetValue, LibraryQuery, LibraryQueryPage, LibrarySortField,
};
use crate::summary::{LibraryBookSummary, LibrarySummaryPage};
mod metadata;
mod path;
mod query;
#[cfg(test)]
mod tests;
use caliberate_core::error::{CoreError, CoreResult};
use metadata::load as load_metadata;
use path::safe_path;
use query::{filters, like_escape, paging, sort_expr};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params_from_iter, types::Value};
use std::path::{Path, PathBuf};
use std::time::Instant;

const ESC: char = '\\';

#[derive(Debug, Clone)]
pub struct CalibreLibraryBackend {
    root: PathBuf,
    metadata: PathBuf,
}
impl CalibreLibraryBackend {
    pub fn open(root: impl AsRef<Path>) -> CoreResult<Self> {
        let root = std::fs::canonicalize(root.as_ref())
            .map_err(|e| CoreError::Io("normalize Calibre library root".into(), e))?;
        if !root.is_dir() {
            return Err(ioerr("open Calibre library", std::io::ErrorKind::NotFound));
        }
        let metadata = root.join("metadata.db");
        if !metadata.is_file() {
            return Err(incompatible("missing metadata.db"));
        }
        let b = Self { root, metadata };
        let c = b.connection()?;
        validate_schema(&c)?;
        tracing::debug!(library_root=%b.root.display(),metadata=%b.metadata.display(),"opened attached Calibre library");
        Ok(b)
    }
    pub fn library_root(&self) -> &Path {
        &self.root
    }
    fn connection(&self) -> CoreResult<Connection> {
        let c = Connection::open_with_flags(&self.metadata, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| sqlerr("open Calibre metadata read-only", e))?;
        c.execute_batch("PRAGMA query_only = ON")
            .map_err(|e| sqlerr("enable Calibre query-only protection", e))?;
        Ok(c)
    }
    fn rows(&self, q: &LibraryQuery, page: bool) -> CoreResult<Vec<LibraryBook>> {
        let (w, mut p) = filters(q)?;
        let dir = if q.descending { "DESC" } else { "ASC" };
        let mut s = format!(
            "SELECT b.id,b.title,COALESCE((SELECT LOWER(d.format) FROM data d WHERE d.book=b.id ORDER BY d.id LIMIT 1),''),COALESCE((SELECT d.name FROM data d WHERE d.book=b.id ORDER BY d.id LIMIT 1),''),b.path FROM books b WHERE {w} ORDER BY {}",
            sort_expr(q.sort)
        );
        s.push_str(&format!(" {dir}"));
        if q.sort == LibrarySortField::Series {
            s.push_str(&format!(",b.series_index {dir}"));
        }
        if q.sort != LibrarySortField::Id {
            s.push_str(",b.id ASC");
        }
        if page {
            paging(&mut s, q, &mut p)
        }
        let c = self.connection()?;
        let mut st = c
            .prepare(&s)
            .map_err(|e| sqlerr("prepare Calibre book query", e))?;
        let it = st
            .query_map(params_from_iter(p.iter()), |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| sqlerr("query Calibre books", e))?;
        let mut out = Vec::new();
        for r in it {
            let (id, title, format, name, bp) = r.map_err(|e| sqlerr("read Calibre book", e))?;
            let path = if name.is_empty() {
                String::new()
            } else {
                safe_path(&self.root, &bp, &name, &format)?
                    .to_string_lossy()
                    .into_owned()
            };
            out.push(LibraryBook {
                id,
                title,
                format,
                path,
            });
        }
        Ok(out)
    }
    fn total(&self, q: &LibraryQuery) -> CoreResult<usize> {
        let (w, p) = filters(q)?;
        let c = self.connection()?;
        c.query_row(
            &format!("SELECT COUNT(*) FROM books b WHERE {w}"),
            params_from_iter(p.iter()),
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n as usize)
        .map_err(|e| sqlerr("count Calibre books", e))
    }
    fn primary(&self, id: i64) -> CoreResult<Option<(String, String, String)>> {
        let c = self.connection()?;
        c.query_row("SELECT b.path,COALESCE(d.name,''),LOWER(COALESCE(d.format,'')) FROM books b LEFT JOIN data d ON d.id=(SELECT MIN(x.id) FROM data x WHERE x.book=b.id) WHERE b.id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(|e|sqlerr("read Calibre primary format",e))
    }
}
impl LibraryBackend for CalibreLibraryBackend {
    fn list_books(&self) -> CoreResult<Vec<LibraryBook>> {
        self.rows(&LibraryQuery::default(), false)
    }
    fn get_book(&self, id: i64) -> CoreResult<Option<LibraryBook>> {
        let Some((book_path, name, format)) = self.primary(id)? else {
            return Ok(None);
        };
        let c = self.connection()?;
        let title = c
            .query_row("SELECT title FROM books WHERE id=?1", [id], |r| {
                r.get::<_, String>(0)
            })
            .map_err(|e| sqlerr("read Calibre book title", e))?;
        let path = if name.is_empty() {
            String::new()
        } else {
            safe_path(&self.root, &book_path, &name, &format)?
                .to_string_lossy()
                .into_owned()
        };
        Ok(Some(LibraryBook {
            id,
            title,
            format,
            path,
        }))
    }
    fn search_books(&self, q: &str) -> CoreResult<Vec<LibraryBook>> {
        let p = Value::Text(format!("%{}%", like_escape(q)));
        let c = self.connection()?;
        let sql = format!(
            "SELECT b.id,b.title,COALESCE((SELECT LOWER(d.format) FROM data d WHERE d.book=b.id ORDER BY d.id LIMIT 1),''),COALESCE((SELECT d.name FROM data d WHERE d.book=b.id ORDER BY d.id LIMIT 1),''),b.path FROM books b WHERE b.title LIKE ? ESCAPE '{ESC}' OR EXISTS(SELECT 1 FROM books_authors_link x JOIN authors a ON a.id=x.author WHERE x.book=b.id AND a.name LIKE ? ESCAPE '{ESC}') OR EXISTS(SELECT 1 FROM books_tags_link x JOIN tags t ON t.id=x.tag WHERE x.book=b.id AND t.name LIKE ? ESCAPE '{ESC}') OR EXISTS(SELECT 1 FROM books_series_link x JOIN series s ON s.id=x.series WHERE x.book=b.id AND s.name LIKE ? ESCAPE '{ESC}') ORDER BY b.id"
        );
        let ps = [p.clone(), p.clone(), p.clone(), p];
        let mut st = c
            .prepare(&sql)
            .map_err(|e| sqlerr("prepare Calibre search", e))?;
        let it = st
            .query_map(params_from_iter(ps.iter()), |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| sqlerr("query Calibre search", e))?;
        let mut o = Vec::new();
        for r in it {
            let (id, title, format, name, bp) = r.map_err(|e| sqlerr("read Calibre search", e))?;
            o.push(LibraryBook {
                id,
                title,
                format: format.clone(),
                path: if name.is_empty() {
                    String::new()
                } else {
                    safe_path(&self.root, &bp, &name, &format)?
                        .to_string_lossy()
                        .into_owned()
                },
            })
        }
        Ok(o)
    }
    fn query_books(&self, q: &LibraryQuery) -> CoreResult<Vec<LibraryBook>> {
        self.rows(q, true)
    }
    fn query_page(&self, q: &LibraryQuery) -> CoreResult<LibraryQueryPage> {
        let t = Instant::now();
        let books = self.rows(q, true)?;
        let total = self.total(q)?;
        tracing::debug!(
            total,
            offset = q.offset.unwrap_or(0),
            elapsed_ms = t.elapsed().as_millis(),
            "queried attached Calibre library"
        );
        Ok(LibraryQueryPage {
            books,
            total,
            offset: q.offset.unwrap_or(0),
            limit: q.limit,
        })
    }
    fn query_summary_page(&self, q: &LibraryQuery) -> CoreResult<LibrarySummaryPage> {
        let books = self.rows(q, true)?;
        let total = self.total(q)?;
        let ids: Vec<i64> = books.iter().map(|b| b.id).collect();
        let m = load_metadata(self, &ids)?;
        let out = books
            .into_iter()
            .map(|b| {
                let x = m.get(&b.id).cloned().unwrap_or_default();
                LibraryBookSummary {
                    id: b.id,
                    title: b.title,
                    format: b.format,
                    path: b.path,
                    authors: x.authors,
                    tags: x.tags,
                    series: x.series,
                    rating: x.rating,
                    publisher: x.publisher,
                    languages: x.languages,
                    has_cover: x.cover,
                    date_added: x.added,
                    date_modified: x.modified,
                    pubdate: x.pubdate,
                }
            })
            .collect();
        Ok(LibrarySummaryPage {
            books: out,
            total,
            offset: q.offset.unwrap_or(0),
            limit: q.limit,
        })
    }
    fn list_facets(&self, k: LibraryFacetKind) -> CoreResult<Vec<LibraryFacetValue>> {
        let (c, sql) = self.connection().map(|c| (c, facet(k)))?;
        let mut s = c
            .prepare(sql)
            .map_err(|e| sqlerr("prepare Calibre facets", e))?;
        let it = s
            .query_map([], |r| {
                Ok(LibraryFacetValue {
                    id: r.get(0)?,
                    name: r.get::<_, String>(1)?,
                    count: r.get(2)?,
                })
            })
            .map_err(|e| sqlerr("query Calibre facets", e))?;
        it.map(|r| r.map_err(|e| sqlerr("read Calibre facet", e)))
            .collect()
    }
    fn resolve_content(&self, id: i64) -> CoreResult<Option<LibraryContent>> {
        let Some((bp, name, format)) = self.primary(id)? else {
            return Ok(None);
        };
        if name.is_empty() || format.is_empty() {
            return Ok(None);
        }
        let p = safe_path(&self.root, &bp, &name, &format)?;
        Ok(Some(LibraryContent {
            book_id: id,
            format: format.to_ascii_lowercase(),
            path: p.to_string_lossy().into_owned(),
            storage_mode: Some("reference".into()),
        }))
    }
}

fn validate_schema(c: &Connection) -> CoreResult<()> {
    let req = [
        (
            "books",
            &[
                "id",
                "title",
                "timestamp",
                "pubdate",
                "series_index",
                "author_sort",
                "path",
                "uuid",
                "has_cover",
                "last_modified",
            ][..],
        ),
        (
            "data",
            &["id", "book", "format", "uncompressed_size", "name"][..],
        ),
        ("authors", &["id", "name"][..]),
        ("books_authors_link", &["id", "book", "author"][..]),
        ("tags", &["id", "name"][..]),
        ("books_tags_link", &["id", "book", "tag"][..]),
        ("series", &["id", "name"][..]),
        ("books_series_link", &["id", "book", "series"][..]),
        ("publishers", &["id", "name"][..]),
        ("books_publishers_link", &["id", "book", "publisher"][..]),
        ("ratings", &["id", "rating"][..]),
        ("books_ratings_link", &["id", "book", "rating"][..]),
        ("languages", &["id", "lang_code"][..]),
        (
            "books_languages_link",
            &["id", "book", "lang_code", "item_order"][..],
        ),
        ("identifiers", &["id", "book", "type", "val"][..]),
    ];
    for (t, cols) in req {
        let mut s = c
            .prepare(&format!("PRAGMA table_info([{t}])"))
            .map_err(|e| sqlerr("validate Calibre schema", e))?;
        let ns: Vec<String> = s
            .query_map([], |r| r.get(1))
            .map_err(|e| sqlerr("validate Calibre schema", e))?
            .collect::<Result<_, _>>()
            .map_err(|e| sqlerr("read Calibre schema", e))?;
        if ns.is_empty() {
            return Err(incompatible(&format!("missing required table {t}")));
        }
        for col in cols {
            if !ns.iter().any(|x| x == col) {
                return Err(incompatible(&format!("missing required column {t}.{col}")));
            }
        }
    }
    Ok(())
}
fn facet(k: LibraryFacetKind) -> &'static str {
    match k {
        LibraryFacetKind::Authors => {
            "SELECT a.id,a.name,COUNT(DISTINCT x.book) FROM authors a LEFT JOIN books_authors_link x ON x.author=a.id GROUP BY a.id ORDER BY a.name COLLATE NOCASE"
        }
        LibraryFacetKind::Tags => {
            "SELECT a.id,a.name,COUNT(DISTINCT x.book) FROM tags a LEFT JOIN books_tags_link x ON x.tag=a.id GROUP BY a.id ORDER BY a.name COLLATE NOCASE"
        }
        LibraryFacetKind::Series => {
            "SELECT a.id,a.name,COUNT(DISTINCT x.book) FROM series a LEFT JOIN books_series_link x ON x.series=a.id GROUP BY a.id ORDER BY a.name COLLATE NOCASE"
        }
        LibraryFacetKind::Publishers => {
            "SELECT a.id,a.name,COUNT(DISTINCT x.book) FROM publishers a LEFT JOIN books_publishers_link x ON x.publisher=a.id GROUP BY a.id ORDER BY a.name COLLATE NOCASE"
        }
        LibraryFacetKind::Ratings => {
            "SELECT a.id,CAST(a.rating AS TEXT),COUNT(DISTINCT x.book) FROM ratings a LEFT JOIN books_ratings_link x ON x.rating=a.id GROUP BY a.id ORDER BY a.rating"
        }
        LibraryFacetKind::Languages => {
            "SELECT a.id,a.lang_code,COUNT(DISTINCT x.book) FROM languages a LEFT JOIN books_languages_link x ON x.lang_code=a.id GROUP BY a.id ORDER BY a.lang_code COLLATE NOCASE"
        }
    }
}
fn incompatible(s: &str) -> CoreError {
    CoreError::ConfigValidate(format!("incompatible Calibre metadata schema: {s}"))
}
fn ioerr(s: &str, k: std::io::ErrorKind) -> CoreError {
    CoreError::Io(s.into(), std::io::Error::new(k, s))
}
fn sqlerr(s: &str, e: rusqlite::Error) -> CoreError {
    CoreError::Io(s.into(), std::io::Error::other(e))
}
