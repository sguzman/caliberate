use super::CalibreLibraryBackend;
use super::sqlerr;
use crate::catalog::LibraryFormat;
use crate::summary::LibrarySeriesSummary;
use caliberate_core::error::CoreResult;
use rusqlite::{Connection, params_from_iter, types::Value};
use std::collections::HashMap;

const FORMAT_ID_CHUNK: usize = 400;
#[derive(Default, Clone)]
pub(super) struct Meta {
    pub(super) authors: Vec<String>,
    pub(super) tags: Vec<String>,
    pub(super) series: Option<LibrarySeriesSummary>,
    pub(super) rating: Option<i64>,
    pub(super) publisher: Option<String>,
    pub(super) languages: Vec<String>,
    pub(super) cover: bool,
    pub(super) added: Option<String>,
    pub(super) modified: Option<String>,
    pub(super) pubdate: Option<String>,
}
pub(super) fn load(b: &CalibreLibraryBackend, ids: &[i64]) -> CoreResult<HashMap<i64, Meta>> {
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

pub(super) fn load_formats(
    b: &CalibreLibraryBackend,
    ids: &[i64],
) -> CoreResult<HashMap<i64, Vec<LibraryFormat>>> {
    let mut formats = ids
        .iter()
        .map(|id| (*id, Vec::new()))
        .collect::<HashMap<_, _>>();
    if ids.is_empty() {
        return Ok(formats);
    }
    let c = b.connection()?;
    for chunk in ids.chunks(FORMAT_ID_CHUNK) {
        let placeholders = std::iter::repeat("?")
            .take(chunk.len())
            .collect::<Vec<_>>()
            .join(",");
        let values: Vec<Value> = chunk.iter().copied().map(Value::from).collect();
        let mut statement = c
            .prepare(&format!(
                "SELECT book,format,uncompressed_size FROM data WHERE book IN ({placeholders}) ORDER BY book,id"
            ))
            .map_err(|e| sqlerr("prepare Calibre summary formats", e))?;
        let rows = statement
            .query_map(params_from_iter(values.into_iter()), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            })
            .map_err(|e| sqlerr("query Calibre summary formats", e))?;
        for row in rows {
            let (book_id, raw_format, raw_size) =
                row.map_err(|e| sqlerr("read Calibre summary format", e))?;
            let format = raw_format.to_ascii_lowercase();
            let Some(book_formats) = formats.get_mut(&book_id) else {
                continue;
            };
            if book_formats.iter().any(|item| item.format == format) {
                continue;
            }
            book_formats.push(LibraryFormat {
                format,
                size_bytes: raw_size.and_then(|size| u64::try_from(size).ok()),
            });
        }
    }
    Ok(formats)
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
