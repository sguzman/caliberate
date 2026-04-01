use caliberate_core::config::FormatsConfig;
use caliberate_metadata::extract::{extract_archive_entry, extract_archive_preview};
use std::fs;
use tempfile::tempdir;

#[test]
fn extracts_7z_archive_entry() {
    let dir = tempdir().expect("temp dir");
    let source_path = dir.path().join("book.epub");
    fs::write(&source_path, b"seven z contents").expect("write source");

    let archive_path = dir.path().join("books.7z");
    sevenz_rust2::compress_to_path(&source_path, &archive_path).expect("compress 7z");

    let formats = FormatsConfig {
        supported: vec!["epub".to_string()],
        archive_formats: vec!["7z".to_string()],
    };

    let preview = extract_archive_preview(&archive_path, &formats).expect("preview");
    assert!(preview.entries.iter().any(|entry| entry == "book.epub"));

    let output_dir = dir.path().join("out");
    let extracted =
        extract_archive_entry(&archive_path, "book.epub", &output_dir, &formats).expect("extract");
    let contents = fs::read(&extracted).expect("read extracted");
    assert_eq!(contents, b"seven z contents");
}
