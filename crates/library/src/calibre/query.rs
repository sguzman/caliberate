use crate::query::{
    LibraryMetadataFilterField, LibraryMetadataFilterMode, LibraryQuery, LibrarySortField,
};
use caliberate_core::error::CoreResult;
use rusqlite::types::Value;
const ESC: char = '\\';
pub(super) fn filters(q: &LibraryQuery) -> CoreResult<(String, Vec<Value>)> {
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
            "EXISTS(SELECT 1 FROM identifiers z WHERE z.book=b.id AND (z.val LIKE ? ESCAPE '\\' OR z.type LIKE ? ESCAPE '\\'))",
        ),
    ] {
        if let Some(x) = v {
            c.push(sql.into());
            p.push(Value::Text(format!("%{}%", like_escape(x))));
            if sql.contains("identifiers") {
                p.push(Value::Text(format!("%{}%", like_escape(x))));
            }
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
                super::incompatible("invalid rating filter; expected an integer from 0 through 10")
            })?;
            if !(0..=10).contains(&n) {
                return Err(super::incompatible(
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
pub(super) fn sort_expr(s: LibrarySortField) -> &'static str {
    match s {
        LibrarySortField::Id => "b.id",
        LibrarySortField::Title => "b.title COLLATE NOCASE",
        LibrarySortField::Authors => {
            "COALESCE((SELECT a.name FROM books_authors_link x JOIN authors a ON a.id=x.author WHERE x.book=b.id ORDER BY a.name COLLATE NOCASE,a.id LIMIT 1),'') COLLATE NOCASE"
        }
        LibrarySortField::Tags => {
            "COALESCE((SELECT z.name FROM books_tags_link x JOIN tags z ON z.id=x.tag WHERE x.book=b.id ORDER BY z.name COLLATE NOCASE,z.id LIMIT 1),'') COLLATE NOCASE"
        }
        LibrarySortField::Series => {
            "COALESCE((SELECT z.name FROM books_series_link x JOIN series z ON z.id=x.series WHERE x.book=b.id ORDER BY z.name COLLATE NOCASE,z.id LIMIT 1),'') COLLATE NOCASE"
        }
        LibrarySortField::Format => {
            "COALESCE((SELECT LOWER(d.format) FROM data d WHERE d.book=b.id ORDER BY d.id LIMIT 1),'')"
        }
        LibrarySortField::Rating => {
            "COALESCE((SELECT z.rating FROM books_ratings_link x JOIN ratings z ON z.id=x.rating WHERE x.book=b.id ORDER BY x.id LIMIT 1),0)"
        }
        LibrarySortField::Publisher => {
            "COALESCE((SELECT z.name FROM books_publishers_link x JOIN publishers z ON z.id=x.publisher WHERE x.book=b.id ORDER BY x.id LIMIT 1),'') COLLATE NOCASE"
        }
        LibrarySortField::Languages => {
            "COALESCE((SELECT z.lang_code FROM books_languages_link x JOIN languages z ON z.id=x.lang_code WHERE x.book=b.id ORDER BY x.item_order,x.id LIMIT 1),'') COLLATE NOCASE"
        }
        LibrarySortField::DateAdded => "COALESCE(b.timestamp,'')",
        LibrarySortField::DateModified => "COALESCE(b.last_modified,'')",
        LibrarySortField::PubDate => "COALESCE(b.pubdate,'')",
    }
}
pub(super) fn paging(s: &mut String, q: &LibraryQuery, p: &mut Vec<Value>) {
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
pub(super) fn like_escape(s: &str) -> String {
    s.replace(ESC, "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
