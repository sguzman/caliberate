//! Read-only adapter for an attached modern Calibre library.
use crate::catalog::{LibraryBackend, LibraryBook, LibraryContent};
use crate::query::{
    LibraryFacetKind, LibraryFacetValue, LibraryMetadataFilterField, LibraryMetadataFilterMode,
    LibraryQuery, LibraryQueryPage, LibrarySortField,
};
use crate::summary::{LibraryBookSummary, LibrarySeriesSummary, LibrarySummaryPage};
use caliberate_core::error::{CoreError, CoreResult};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params_from_iter, types::Value};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

const ESC: char = '\\';

#[derive(Debug, Clone)]
pub struct CalibreLibraryBackend {
    root: PathBuf,
    metadata: PathBuf,
}
impl CalibreLibraryBackend {
    pub fn open(root: impl AsRef<Path>) -> CoreResult<Self> {
        let root = root.as_ref().to_path_buf();
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
        c.query_row("SELECT b.path,COALESCE(d.name,''),COALESCE(d.format,'') FROM books b LEFT JOIN data d ON d.id=(SELECT MIN(x.id) FROM data x WHERE x.book=b.id) WHERE b.id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(|e|sqlerr("read Calibre primary format",e))
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
        let m = metadata(self, &ids)?;
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
fn filters(q: &LibraryQuery) -> CoreResult<(String, Vec<Value>)> {
    let mut c = Vec::new();
    let mut p = Vec::new();
    if let Some(v) = &q.title {
        c.push(format!("b.title LIKE ? ESCAPE '{ESC}'"));
        p.push(Value::Text(format!("%{}%", like_escape(v))))
    }
    for (v, sql) in [
        (
            &q.author,
            "EXISTS(SELECT 1 FROM books_authors_link x JOIN authors z ON z.id=x.author WHERE x.book=b.id AND z.name LIKE ? ESCAPE '\\')",
        ),
        (
            &q.tag,
            "EXISTS(SELECT 1 FROM books_tags_link x JOIN tags z ON z.id=x.tag WHERE x.book=b.id AND z.name LIKE ? ESCAPE '\\')",
        ),
        (
            &q.series,
            "EXISTS(SELECT 1 FROM books_series_link x JOIN series z ON z.id=x.series WHERE x.book=b.id AND z.name LIKE ? ESCAPE '\\')",
        ),
        (
            &q.publisher,
            "EXISTS(SELECT 1 FROM books_publishers_link x JOIN publishers z ON z.id=x.publisher WHERE x.book=b.id AND z.name LIKE ? ESCAPE '\\')",
        ),
        (
            &q.language,
            "EXISTS(SELECT 1 FROM books_languages_link x JOIN languages z ON z.id=x.lang_code WHERE x.book=b.id AND z.lang_code LIKE ? ESCAPE '\\')",
        ),
        (
            &q.identifier,
            "EXISTS(SELECT 1 FROM identifiers z WHERE z.book=b.id AND z.val LIKE ? ESCAPE '\\')",
        ),
    ] {
        if let Some(x) = v {
            c.push(sql.into());
            p.push(Value::Text(format!("%{}%", like_escape(x))))
        }
    }
    if let Some(v) = &q.format {
        c.push(format!("LOWER(COALESCE((SELECT d.format FROM data d WHERE d.book=b.id ORDER BY d.id LIMIT 1),'')) LIKE LOWER(?) ESCAPE '{ESC}'"));
        p.push(Value::Text(format!("%{}%", like_escape(v))))
    }
    for f in &q.metadata_filters {
        let (is_rating, table, link, col, name) = match f.field {
            LibraryMetadataFilterField::Authors => {
                (false, "authors", "books_authors_link", "author", "name")
            }
            LibraryMetadataFilterField::Tags => (false, "tags", "books_tags_link", "tag", "name"),
            LibraryMetadataFilterField::Series => {
                (false, "series", "books_series_link", "series", "name")
            }
            LibraryMetadataFilterField::Publishers => (
                false,
                "publishers",
                "books_publishers_link",
                "publisher",
                "name",
            ),
            LibraryMetadataFilterField::Languages => (
                false,
                "languages",
                "books_languages_link",
                "lang_code",
                "lang_code",
            ),
            LibraryMetadataFilterField::Ratings => {
                (true, "ratings", "books_ratings_link", "rating", "rating")
            }
        };
        if is_rating {
            let n = f.value.parse::<i64>().map_err(|_| {
                incompatible("invalid rating filter; expected an integer from 0 through 10")
            })?;
            if !(0..=10).contains(&n) {
                return Err(incompatible(
                    "invalid rating filter; expected an integer from 0 through 10",
                ));
            }
            c.push(format!(
                "{}EXISTS(SELECT 1 FROM {link} x JOIN {table} z ON z.id=x.{col} WHERE x.book=b.id AND z.rating=?)",
                if f.mode == LibraryMetadataFilterMode::Exclude {
                    "NOT "
                } else {
                    ""
                }
            ));
            p.push(Value::Integer(n))
        } else {
            c.push(format!("{}EXISTS(SELECT 1 FROM {link} x JOIN {table} z ON z.id=x.{col} WHERE x.book=b.id AND z.{name} LIKE ? ESCAPE '{ESC}')",if f.mode==LibraryMetadataFilterMode::Exclude{"NOT "}else{""}));
            p.push(Value::Text(format!("%{}%", like_escape(&f.value))))
        }
    }
    Ok((
        if c.is_empty() {
            "1=1".into()
        } else {
            c.join(" AND ")
        },
        p,
    ))
}
fn sort_expr(s: LibrarySortField) -> &'static str {
    match s {
        LibrarySortField::Id => "b.id",
        LibrarySortField::Title => "b.title COLLATE NOCASE",
        LibrarySortField::Authors => {
            "COALESCE((SELECT a.name FROM books_authors_link x JOIN authors a ON a.id=x.author WHERE x.book=b.id ORDER BY a.name COLLATE NOCASE,a.id LIMIT 1),'')"
        }
        LibrarySortField::Tags => {
            "COALESCE((SELECT z.name FROM books_tags_link x JOIN tags z ON z.id=x.tag WHERE x.book=b.id ORDER BY z.name COLLATE NOCASE,z.id LIMIT 1),'')"
        }
        LibrarySortField::Series => {
            "COALESCE((SELECT z.name FROM books_series_link x JOIN series z ON z.id=x.series WHERE x.book=b.id ORDER BY z.name COLLATE NOCASE,z.id LIMIT 1),'')"
        }
        LibrarySortField::Format => {
            "COALESCE((SELECT LOWER(d.format) FROM data d WHERE d.book=b.id ORDER BY d.id LIMIT 1),'')"
        }
        LibrarySortField::Rating => {
            "COALESCE((SELECT z.rating FROM books_ratings_link x JOIN ratings z ON z.id=x.rating WHERE x.book=b.id ORDER BY x.id LIMIT 1),0)"
        }
        LibrarySortField::Publisher => {
            "COALESCE((SELECT z.name FROM books_publishers_link x JOIN publishers z ON z.id=x.publisher WHERE x.book=b.id ORDER BY x.id LIMIT 1),'')"
        }
        LibrarySortField::Languages => {
            "COALESCE((SELECT z.lang_code FROM books_languages_link x JOIN languages z ON z.id=x.lang_code WHERE x.book=b.id ORDER BY x.item_order,x.id LIMIT 1),'')"
        }
        LibrarySortField::DateAdded => "COALESCE(b.timestamp,'')",
        LibrarySortField::DateModified => "COALESCE(b.last_modified,'')",
        LibrarySortField::PubDate => "COALESCE(b.pubdate,'')",
    }
}
fn paging(s: &mut String, q: &LibraryQuery, p: &mut Vec<Value>) {
    if let Some(n) = q.limit {
        s.push_str(" LIMIT ?");
        p.push(Value::Integer(n as i64));
        if let Some(n) = q.offset {
            s.push_str(" OFFSET ?");
            p.push(Value::Integer(n as i64));
        }
    } else if let Some(n) = q.offset {
        s.push_str(" LIMIT -1 OFFSET ?");
        p.push(Value::Integer(n as i64));
    }
}
fn like_escape(s: &str) -> String {
    s.replace(ESC, "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
fn safe_path(root: &Path, book: &str, name: &str, format: &str) -> CoreResult<PathBuf> {
    let p = Path::new(book);
    if p.is_absolute()
        || p.components().any(|x| {
            matches!(
                x,
                Component::Prefix(_) | Component::RootDir | Component::ParentDir
            )
        })
    {
        return Err(incompatible("unsafe Calibre books.path"));
    }
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || Path::new(name).components().count() != 1
    {
        return Err(incompatible("unsafe Calibre data.name"));
    }
    let out = root
        .join(p)
        .join(format!("{name}.{}", format.to_ascii_lowercase()));
    if !out.starts_with(root) {
        return Err(incompatible("Calibre content path escapes library root"));
    }
    Ok(out)
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
#[derive(Default, Clone)]
struct Meta {
    authors: Vec<String>,
    tags: Vec<String>,
    series: Option<LibrarySeriesSummary>,
    rating: Option<i64>,
    publisher: Option<String>,
    languages: Vec<String>,
    cover: bool,
    added: Option<String>,
    modified: Option<String>,
    pubdate: Option<String>,
}
fn metadata(b: &CalibreLibraryBackend, ids: &[i64]) -> CoreResult<HashMap<i64, Meta>> {
    let mut o = ids
        .iter()
        .map(|i| (*i, Meta::default()))
        .collect::<HashMap<_, _>>();
    if ids.is_empty() {
        return Ok(o);
    }
    let c = b.connection()?;
    let p = std::iter::repeat("?")
        .take(ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let values: Vec<Value> = ids.iter().copied().map(Value::from).collect();
    let vals = || params_from_iter(values.clone().into_iter());
    {
        let mut s = c
            .prepare(&format!(
                "SELECT id,timestamp,last_modified,pubdate,has_cover FROM books WHERE id IN ({p})"
            ))
            .map_err(|e| sqlerr("prepare Calibre summary", e))?;
        for r in s
            .query_map(vals(), |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get::<_, i64>(4)? != 0,
                ))
            })
            .map_err(|e| sqlerr("query Calibre summary", e))?
        {
            let (id, a, m, pv, cover) = r.map_err(|e| sqlerr("read Calibre summary", e))?;
            let x = o.get_mut(&id).unwrap();
            x.added = a;
            x.modified = m;
            x.pubdate = pv;
            x.cover = cover;
        }
    }
    bulk_text(
        &c,
        &p,
        &vals,
        "SELECT x.book,z.name FROM books_authors_link x JOIN authors z ON z.id=x.author WHERE x.book IN ({p}) ORDER BY x.book,z.name COLLATE NOCASE,z.id",
        &mut o,
        |x, v| x.authors.push(v),
    )?;
    bulk_text(
        &c,
        &p,
        &vals,
        "SELECT x.book,z.name FROM books_tags_link x JOIN tags z ON z.id=x.tag WHERE x.book IN ({p}) ORDER BY x.book,z.name COLLATE NOCASE,z.id",
        &mut o,
        |x, v| x.tags.push(v),
    )?;
    {
        let mut s=c.prepare(&format!("SELECT x.book,z.name,b.series_index FROM books_series_link x JOIN series z ON z.id=x.series JOIN books b ON b.id=x.book WHERE x.book IN ({p}) ORDER BY x.book,x.id")).map_err(|e|sqlerr("prepare Calibre series",e))?;
        for r in s
            .query_map(vals(), |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get(2)?))
            })
            .map_err(|e| sqlerr("query Calibre series", e))?
        {
            let (id, n, i) = r.map_err(|e| sqlerr("read Calibre series", e))?;
            o.get_mut(&id)
                .unwrap()
                .series
                .get_or_insert(LibrarySeriesSummary { name: n, index: i });
        }
    }
    bulk_scalar(
        &c,
        &p,
        &vals,
        "SELECT x.book,z.rating FROM books_ratings_link x JOIN ratings z ON z.id=x.rating WHERE x.book IN ({p}) ORDER BY x.book,x.id",
        &mut o,
        true,
    )?;
    bulk_publisher(
        &c,
        &p,
        &vals,
        "SELECT x.book,z.name FROM books_publishers_link x JOIN publishers z ON z.id=x.publisher WHERE x.book IN ({p}) ORDER BY x.book,x.id",
        &mut o,
    )?;
    bulk_text(
        &c,
        &p,
        &vals,
        "SELECT x.book,z.lang_code FROM books_languages_link x JOIN languages z ON z.id=x.lang_code WHERE x.book IN ({p}) ORDER BY x.book,x.item_order,x.id",
        &mut o,
        |x, v| x.languages.push(v),
    )?;
    Ok(o)
}
fn bulk_text<F: FnMut(&mut Meta, String)>(
    c: &Connection,
    p: &str,
    vals: &impl Fn() -> rusqlite::ParamsFromIter<std::vec::IntoIter<Value>>,
    sql: &str,
    o: &mut HashMap<i64, Meta>,
    mut f: F,
) -> CoreResult<()> {
    let mut s = c
        .prepare(&sql.replace("{p}", p))
        .map_err(|e| sqlerr("prepare Calibre metadata", e))?;
    for r in s
        .query_map(vals(), |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(|e| sqlerr("query Calibre metadata", e))?
    {
        let (i, v) = r.map_err(|e| sqlerr("read Calibre metadata", e))?;
        if let Some(x) = o.get_mut(&i) {
            f(x, v)
        }
    }
    Ok(())
}
fn bulk_scalar(
    c: &Connection,
    p: &str,
    vals: &impl Fn() -> rusqlite::ParamsFromIter<std::vec::IntoIter<Value>>,
    sql: &str,
    o: &mut HashMap<i64, Meta>,
    rating: bool,
) -> CoreResult<()> {
    let mut s = c
        .prepare(&sql.replace("{p}", p))
        .map_err(|e| sqlerr("prepare Calibre scalar", e))?;
    for r in s
        .query_map(vals(), |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
        .map_err(|e| sqlerr("query Calibre scalar", e))?
    {
        let (i, v) = r.map_err(|e| sqlerr("read Calibre scalar", e))?;
        let x = o.get_mut(&i).unwrap();
        if rating {
            x.rating.get_or_insert(v);
        }
    }
    Ok(())
}
fn bulk_publisher(
    c: &Connection,
    p: &str,
    vals: &impl Fn() -> rusqlite::ParamsFromIter<std::vec::IntoIter<Value>>,
    sql: &str,
    o: &mut HashMap<i64, Meta>,
) -> CoreResult<()> {
    let mut s = c
        .prepare(&sql.replace("{p}", p))
        .map_err(|e| sqlerr("prepare Calibre publisher", e))?;
    for r in s
        .query_map(vals(), |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(|e| sqlerr("query Calibre publisher", e))?
    {
        let (i, v) = r.map_err(|e| sqlerr("read Calibre publisher", e))?;
        if let Some(x) = o.get_mut(&i) {
            x.publisher.get_or_insert(v);
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::LibraryBackend;
    use crate::query::{LibraryMetadataFilterField, LibraryMetadataFilterMode};
    use rusqlite::Connection;
    use std::fs;
    use tempfile::TempDir;

    fn fixture() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let db = Connection::open(dir.path().join("metadata.db")).unwrap();
        db.execute_batch(
            "CREATE TABLE books(id INTEGER PRIMARY KEY,title TEXT,timestamp TEXT,pubdate TEXT,series_index REAL,author_sort TEXT,path TEXT,uuid TEXT,has_cover INTEGER,last_modified TEXT);
             CREATE TABLE data(id INTEGER PRIMARY KEY,book INTEGER,format TEXT,uncompressed_size INTEGER,name TEXT,UNIQUE(book,format));
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
             INSERT INTO books VALUES(1,'Book One','2026-01-01','2025-01-01',1.0,'Author A','Author A/Book One (1)','u1',1,'2026-01-02');
             INSERT INTO books VALUES(2,'Book Two','2026-02-01',NULL,1.0,'Author B','Author B/Book Two (2)','u2',0,NULL);
             INSERT INTO data VALUES(10,1,'PDF',20,'Book One - Author A');
             INSERT INTO data VALUES(11,1,'EPUB',10,'Book One - Author A');
             INSERT INTO data VALUES(20,2,'AZW3',30,'Book Two - Author B');
             INSERT INTO authors VALUES(1,'Author A'),(2,'Author B');
             INSERT INTO books_authors_link VALUES(1,1,1),(2,2,2);
             INSERT INTO tags VALUES(1,'fiction'); INSERT INTO books_tags_link VALUES(1,1,1);
             INSERT INTO series VALUES(1,'Series A'); INSERT INTO books_series_link VALUES(1,1,1);
             INSERT INTO publishers VALUES(1,'Publisher A'); INSERT INTO books_publishers_link VALUES(1,1,1);
             INSERT INTO ratings VALUES(1,8); INSERT INTO books_ratings_link VALUES(1,1,1);
             INSERT INTO languages VALUES(1,'en'); INSERT INTO books_languages_link VALUES(1,1,1,0);
             INSERT INTO identifiers VALUES(1,1,'isbn','abc-1');",
        ).unwrap();
        fs::create_dir_all(dir.path().join("Author A/Book One (1)")).unwrap();
        fs::write(
            dir.path()
                .join("Author A/Book One (1)/Book One - Author A.pdf"),
            b"pdf",
        )
        .unwrap();
        dir
    }

    #[test]
    fn reads_modern_fixture_without_writing_source() {
        let dir = fixture();
        let db_path = dir.path().join("metadata.db");
        let before = fs::read(&db_path).unwrap();
        let backend = CalibreLibraryBackend::open(dir.path()).unwrap();
        assert_eq!(backend.list_books().unwrap()[0].format, "pdf");
        assert_eq!(backend.get_book(1).unwrap().unwrap().title, "Book One");
        assert!(backend.search_books("fiction").unwrap().len() == 1);
        assert_eq!(backend.resolve_content(1).unwrap().unwrap().format, "pdf");
        let summary = backend
            .query_summary_page(&LibraryQuery::default())
            .unwrap();
        assert_eq!(summary.books[0].authors, ["Author A"]);
        assert_eq!(summary.books[0].publisher.as_deref(), Some("Publisher A"));
        assert_eq!(
            backend.list_facets(LibraryFacetKind::Ratings).unwrap()[0].name,
            "8"
        );
        let filtered = LibraryQuery::default().with_metadata_filter(
            LibraryMetadataFilterField::Tags,
            LibraryMetadataFilterMode::Include,
            "FICT",
        );
        assert_eq!(backend.query_page(&filtered).unwrap().total, 1);
        assert_eq!(before, fs::read(&db_path).unwrap());
    }

    #[test]
    fn rejects_missing_metadata_and_unsafe_source_paths() {
        let empty = tempfile::tempdir().unwrap();
        assert!(CalibreLibraryBackend::open(empty.path()).is_err());
        let dir = fixture();
        let db = Connection::open(dir.path().join("metadata.db")).unwrap();
        db.execute("UPDATE books SET path='../outside' WHERE id=1", [])
            .unwrap();
        let backend = CalibreLibraryBackend::open(dir.path()).unwrap();
        assert!(backend.resolve_content(1).is_err());
    }
}
