use caliberate_db::database::Database;
use tempfile::tempdir;

fn database() -> Database {
    let dir = tempdir().expect("temporary database directory");
    let path = dir.keep().join("library.db");
    Database::open_path(path, 1000).expect("open database")
}

#[test]
fn sources_formats_and_assets_are_canonical_and_batched() {
    let db = database();
    let book = db
        .add_book("Canonical", "EPUB", "/books/canonical.epub", "2026-01-01")
        .expect("add book");
    let other = db
        .add_book("Other", "pdf", "/books/other.pdf", "2026-01-02")
        .expect("add other book");
    let empty = db
        .add_book("Empty", "", "", "2026-01-03")
        .expect("add empty book");

    let source = db
        .upsert_library_source("calibre", r"C:\synthetic\calibre", Some("Synthetic"), true)
        .expect("register source");
    assert_eq!(
        db.upsert_library_source("calibre", r"C:\synthetic\calibre", Some("Updated"), false)
            .unwrap(),
        source
    );
    assert_eq!(
        db.get_library_source(source)
            .unwrap()
            .unwrap()
            .label
            .as_deref(),
        Some("Updated")
    );
    assert!(!db.get_library_source(source).unwrap().unwrap().read_only);
    let second_source = db
        .upsert_library_source("directory", r"C:\synthetic\directory", None, true)
        .expect("register second source");
    db.upsert_source_book(second_source, book, "directory-book", None, None, None)
        .expect("map second source to canonical book");

    let first_mapping = db
        .upsert_source_book(
            source,
            book,
            "external-1",
            Some("uuid-1"),
            Some("modified"),
            Some("seen"),
        )
        .expect("map source book");
    let imported_at = db
        .get_source_book(source, "external-1")
        .unwrap()
        .unwrap()
        .imported_at;
    assert_eq!(
        db.upsert_source_book(source, other, "external-1", Some("uuid-2"), None, None)
            .unwrap(),
        first_mapping
    );
    let mapping = db.get_source_book(source, "external-1").unwrap().unwrap();
    assert_eq!(mapping.book_id, other);
    assert_eq!(mapping.external_uuid.as_deref(), Some("uuid-2"));
    assert_eq!(mapping.imported_at, imported_at);
    assert_eq!(db.list_source_books(source).unwrap().len(), 1);
    db.upsert_source_book(source, book, "external-2", None, None, None)
        .expect("map second source book");
    db.update_library_source_last_sync(source, Some("now"))
        .unwrap();
    assert_eq!(
        db.find_library_source("calibre", r"C:\synthetic\calibre")
            .unwrap()
            .unwrap()
            .last_sync_at
            .as_deref(),
        Some("now")
    );

    let epub = db.upsert_book_format(book, "EPUB", Some(100)).unwrap();
    assert_eq!(db.upsert_book_format(book, "epub", None).unwrap(), epub);
    assert_eq!(
        db.get_book_format(book, "ePuB")
            .unwrap()
            .unwrap()
            .size_bytes,
        Some(100)
    );
    let pdf = db.upsert_book_format(book, "PDF", Some(200)).unwrap();
    let mobi = db.upsert_book_format(book, "MOBI", None).unwrap();
    let formats = db
        .list_book_formats_for_books(&[book, other, empty, 999])
        .unwrap();
    assert_eq!(
        formats[&book]
            .iter()
            .map(|row| row.format.as_str())
            .collect::<Vec<_>>(),
        ["epub", "pdf", "mobi"]
    );
    assert_eq!(
        formats[&other]
            .iter()
            .map(|row| row.format.as_str())
            .collect::<Vec<_>>(),
        ["pdf"]
    );
    assert!(formats[&empty].is_empty());
    assert!(formats[&999].is_empty());

    let old_asset = db
        .add_asset(
            book,
            "copy",
            "/managed/epub",
            None,
            100,
            100,
            None,
            false,
            "now",
        )
        .unwrap();
    assert_eq!(
        db.list_assets_for_book(book).unwrap()[0].book_format_id,
        Some(epub)
    );
    let _pdf_asset = db
        .add_asset_for_format(
            book,
            pdf,
            Some(source),
            "reference",
            "/source/pdf",
            Some("/source/pdf"),
            200,
            200,
            None,
            false,
            "now",
        )
        .unwrap();
    let _mobi_asset = db
        .add_asset_for_format(
            book,
            mobi,
            None,
            "reference",
            "/source/mobi",
            None,
            300,
            300,
            None,
            false,
            "now",
        )
        .unwrap();
    assert!(db.remove_book_format(book, "pdf").is_err());
    assert!(
        db.list_assets()
            .unwrap()
            .iter()
            .any(|asset| asset.id == old_asset && asset.source_id.is_none())
    );

    let metadata_only_asset = db
        .add_asset(
            empty,
            "reference",
            "/synthetic/metadata-only.pdf",
            Some("/synthetic/metadata-only.epub"),
            12,
            12,
            None,
            false,
            "now",
        )
        .expect("metadata-only asset should be accepted");
    let metadata_only = db
        .list_assets_for_book(empty)
        .unwrap()
        .into_iter()
        .find(|asset| asset.id == metadata_only_asset)
        .expect("metadata-only asset row");
    assert_eq!(metadata_only.book_format_id, None);
    assert_eq!(metadata_only.source_id, None);
    assert!(db.list_book_formats(empty).unwrap().is_empty());
    assert_eq!(metadata_only.stored_path, "/synthetic/metadata-only.pdf");
    assert_eq!(
        metadata_only.source_path.as_deref(),
        Some("/synthetic/metadata-only.epub")
    );

    let mut db = db;
    db.delete_book_with_assets(book)
        .expect("delete canonical book");
    assert!(db.list_book_formats(book).unwrap().is_empty());
    assert_eq!(db.list_source_books(source).unwrap().len(), 1);
    assert!(db.get_source_book(source, "external-2").unwrap().is_none());
    assert!(db.list_source_books(second_source).unwrap().is_empty());
    assert!(db.get_library_source(source).unwrap().is_some());
}
