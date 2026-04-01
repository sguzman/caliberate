use caliberate_core::config::FormatsConfig;
use caliberate_metadata::extract::extract_archive_entry_to_temp;
use std::fs;
use std::io::Write;
use tempfile::tempdir;
use zip::write::FileOptions;

#[test]
fn extracts_archive_entry_to_temp() {
    let dir = tempdir().expect("temp dir");
    let archive_path = dir.path().join("books.zip");
    let file = fs::File::create(&archive_path).expect("create zip");
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file("book.epub", FileOptions::<()>::default())
        .expect("start file");
    zip.write_all(b"epub contents").expect("write zip");
    zip.finish().expect("finish zip");

    let formats = FormatsConfig {
        supported: vec!["epub".to_string()],
        archive_formats: vec!["zip".to_string()],
    };

    let extracted = extract_archive_entry_to_temp(&archive_path, "book.epub", dir.path(), &formats)
        .expect("extract");

    let contents = fs::read(&extracted).expect("read extracted");
    assert_eq!(contents, b"epub contents");
}
