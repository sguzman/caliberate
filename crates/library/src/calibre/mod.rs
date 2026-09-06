//! Read-only adapter for an attached modern Calibre library.
use crate::catalog::{
    LibraryBackend, LibraryBook, LibraryContent, LibraryContentEncoding, LibraryFormat,
};
use crate::query::{
    LibraryFacetKind, LibraryFacetValue, LibraryQuery, LibraryQueryPage, LibrarySortField,
};
use crate::summary::{LibraryBookSummary, LibrarySummaryPage};
pub mod materialize;
mod metadata;
mod path;
mod query;
#[cfg(test)]
mod tests;
use caliberate_core::error::{CoreError, CoreResult};
use metadata::{load as load_metadata, load_formats};
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
    mode: CalibreOpenMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibreOpenMode {
    LockingReadOnly,
    ImmutableReadOnly,
}

impl CalibreLibraryBackend {
    pub fn open(root: impl AsRef<Path>) -> CoreResult<Self> {
        Self::open_with_mode(root, CalibreOpenMode::LockingReadOnly)
    }

    pub fn open_with_mode(root: impl AsRef<Path>, mode: CalibreOpenMode) -> CoreResult<Self> {
        let root = std::fs::canonicalize(root.as_ref())
            .map_err(|e| CoreError::Io("normalize Calibre library root".into(), e))?;
        if !root.is_dir() {
            return Err(ioerr("open Calibre library", std::io::ErrorKind::NotFound));
        }
        let metadata = root.join("metadata.db");
        if !metadata.is_file() {
            return Err(incompatible("missing metadata.db"));
        }
        let b = Self {
            root,
            metadata,
            mode,
        };
        for suffix in ["-wal", "-shm", "-journal"] {
            let sidecar = b.metadata.with_file_name(format!("metadata.db{suffix}"));
            if sidecar.is_file() {
                tracing::debug!(path=%sidecar.display(), mode=?mode, "Calibre SQLite sidecar present");
            }
        }
        let c = b.connection()?;
        validate_schema(&c, mode)?;
        tracing::debug!(library_root=%b.root.display(),metadata=%b.metadata.display(),mode=?mode,"opened attached Calibre library");
        Ok(b)
    }
    pub fn library_root(&self) -> &Path {
        &self.root
    }

    pub fn open_mode(&self) -> CalibreOpenMode {
        self.mode
    }

    fn connection(&self) -> CoreResult<Connection> {
        let c = match self.mode {
            CalibreOpenMode::LockingReadOnly => {
                Connection::open_with_flags(&self.metadata, OpenFlags::SQLITE_OPEN_READ_ONLY)
                    .map_err(|e| {
                        sqlerr_with_mode(self.mode, "open Calibre metadata read-only", e)
                    })?
            }
            CalibreOpenMode::ImmutableReadOnly => immutable_connection(&self.metadata)?,
        };
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
    fn primary(&self, id: i64) -> CoreResult<Option<(String, String, String, Option<u64>)>> {
        let c = self.connection()?;
        c.query_row("SELECT b.path,COALESCE(d.name,''),LOWER(COALESCE(d.format,'')),d.uncompressed_size FROM books b LEFT JOIN data d ON d.id=(SELECT MIN(x.id) FROM data x WHERE x.book=b.id) WHERE b.id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get::<_,Option<i64>>(3)?.and_then(|size| u64::try_from(size).ok())))).optional().map_err(|e|sqlerr("read Calibre primary format",e))
    }
}

fn sqlite_file_uri(path: &Path) -> CoreResult<String> {
    let raw = path.to_str().ok_or_else(|| {
        CoreError::ConfigValidate("Calibre metadata path is not valid Unicode".into())
    })?;
    let normalized = raw.replace('\\', "/");
    let normalized = if let Some(rest) = normalized.strip_prefix("//?/UNC/") {
        format!("//{rest}")
    } else if let Some(rest) = normalized.strip_prefix("//?/") {
        rest.to_string()
    } else {
        normalized
    };
    if normalized.is_empty() || normalized.contains('\0') {
        return Err(CoreError::ConfigValidate(
            "Calibre metadata path cannot form a SQLite URI".into(),
        ));
    }
    let mut uri = String::from("file:");
    for byte in normalized.as_bytes() {
        if matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':')
        {
            uri.push(*byte as char);
        } else {
            uri.push('%');
            uri.push_str(&format!("{byte:02X}"));
        }
    }
    Ok(uri)
}

fn immutable_sqlite_uri(path: &Path) -> CoreResult<String> {
    Ok(format!("{}?mode=ro&immutable=1", sqlite_file_uri(path)?))
}

fn is_windows_unc_path(path: &Path) -> bool {
    let path = path.to_string_lossy().to_ascii_lowercase();
    path.starts_with(r"\\") && (!path.starts_with(r"\\?\") || path.starts_with(r"\\?\unc\"))
}

#[cfg(windows)]
fn ordinary_windows_unc_path(path: &Path) -> PathBuf {
    let path = path.to_string_lossy();
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path.to_string().into()
    }
}

fn immutable_connection(path: &Path) -> CoreResult<Connection> {
    #[cfg(windows)]
    if is_windows_unc_path(path) {
        let ordinary_path = ordinary_windows_unc_path(path);
        tracing::debug!(path=%ordinary_path.display(), "attached Calibre static source using win32-none VFS");
        return Connection::open_with_flags_and_vfs(
            &ordinary_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY,
            "win32-none",
        )
        .map_err(|e| {
            CoreError::Io(
                format!(
                    "open Calibre UNC metadata with win32-none VFS at {} in static mode; keep the source unchanged while it is in use",
                    ordinary_path.display()
                ),
                std::io::Error::other(e),
            )
        });
    }

    let uri = immutable_sqlite_uri(path)?;
    Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| {
        sqlerr_with_mode(
            CalibreOpenMode::ImmutableReadOnly,
            "open Calibre metadata read-only",
            e,
        )
    })
}

fn sqlerr_with_mode(mode: CalibreOpenMode, context: &str, error: rusqlite::Error) -> CoreError {
    let message = error.to_string();
    if mode == CalibreOpenMode::LockingReadOnly
        && (message.contains("database is locked") || message.contains("database is busy"))
    {
        return CoreError::Io(
            format!(
                "{context} with normal SQLite locking; the source could not be read with normal locking. For a static WSL/network source, explicitly use --calibre-library-immutable; do not modify the library while it is in use"
            ),
            std::io::Error::other(error),
        );
    }
    sqlerr("open Calibre metadata read-only", error)
}
impl LibraryBackend for CalibreLibraryBackend {
    fn list_books(&self) -> CoreResult<Vec<LibraryBook>> {
        self.rows(&LibraryQuery::default(), false)
    }
    fn get_book(&self, id: i64) -> CoreResult<Option<LibraryBook>> {
        let Some((book_path, name, format, _size_bytes)) = self.primary(id)? else {
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
        let formats = load_formats(self, &ids)?;
        let out = books
            .into_iter()
            .map(|b| {
                let x = m.get(&b.id).cloned().unwrap_or_default();
                LibraryBookSummary {
                    id: b.id,
                    title: b.title,
                    format: b.format,
                    path: b.path,
                    formats: formats.get(&b.id).cloned().unwrap_or_default(),
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
        let Some((bp, name, format, size_bytes)) = self.primary(id)? else {
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
            encoding: LibraryContentEncoding::Identity,
            size_bytes,
            stored_size_bytes: size_bytes,
        }))
    }

    fn list_formats(&self, book_id: i64) -> CoreResult<Vec<LibraryFormat>> {
        let c = self.connection()?;
        let mut statement = c
            .prepare("SELECT format, uncompressed_size FROM data WHERE book=?1 ORDER BY id ASC")
            .map_err(|e| sqlerr("prepare Calibre format list", e))?;
        let rows = statement
            .query_map([book_id], |row| {
                let format: String = row.get(0)?;
                let size = row
                    .get::<_, Option<i64>>(1)?
                    .and_then(|size| u64::try_from(size).ok());
                Ok((format.to_ascii_lowercase(), size))
            })
            .map_err(|e| sqlerr("query Calibre formats", e))?;
        let mut formats = Vec::new();
        for row in rows {
            let (format, size_bytes) = row.map_err(|e| sqlerr("read Calibre format", e))?;
            if formats
                .iter()
                .any(|entry: &LibraryFormat| entry.format == format)
            {
                continue;
            }
            formats.push(LibraryFormat { format, size_bytes });
        }
        Ok(formats)
    }

    fn resolve_content_format(
        &self,
        book_id: i64,
        requested_format: &str,
    ) -> CoreResult<Option<LibraryContent>> {
        let c = self.connection()?;
        let row = c
            .query_row(
                "SELECT b.path, d.name, d.format, d.uncompressed_size FROM books b JOIN data d ON d.book=b.id WHERE b.id=?1 AND LOWER(d.format)=LOWER(?2) ORDER BY d.id ASC LIMIT 1",
                rusqlite::params![book_id, requested_format],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, Option<i64>>(3)?.and_then(|size| u64::try_from(size).ok()))),
            )
            .optional()
            .map_err(|e| sqlerr("read Calibre format content", e))?;
        let Some((book_path, name, format, size_bytes)) = row else {
            return Ok(None);
        };
        if name.is_empty() || format.is_empty() {
            return Ok(None);
        }
        let normalized_format = format.to_ascii_lowercase();
        let path = safe_path(&self.root, &book_path, &name, &normalized_format)?;
        Ok(Some(LibraryContent {
            book_id,
            format: normalized_format,
            path: path.to_string_lossy().into_owned(),
            storage_mode: Some("reference".into()),
            encoding: LibraryContentEncoding::Identity,
            size_bytes,
            stored_size_bytes: size_bytes,
        }))
    }
}

fn validate_schema(c: &Connection, mode: CalibreOpenMode) -> CoreResult<()> {
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
            .map_err(|e| sqlerr_with_mode(mode, "validate Calibre schema", e))?;
        let ns: Vec<String> = s
            .query_map([], |r| r.get(1))
            .map_err(|e| sqlerr_with_mode(mode, "validate Calibre schema", e))?
            .collect::<Result<_, _>>()
            .map_err(|e| sqlerr_with_mode(mode, "read Calibre schema", e))?;
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
