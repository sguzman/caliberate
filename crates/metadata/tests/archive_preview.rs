use caliberate_core::config::FormatsConfig;
use caliberate_metadata::extract::extract_archive_preview;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use zip::write::FileOptions;

#[test]
fn zip_archive_preview_lists_entries() {
    let path = temp_zip_path();
    create_zip(&path);

    let formats = FormatsConfig {
        supported: vec!["epub".to_string()],
        archive_formats: vec!["zip".to_string()],
    };

    let preview = extract_archive_preview(&path, &formats).expect("preview");
    assert_eq!(preview.format, "zip");
    assert_eq!(preview.entries, vec!["hello.txt".to_string()]);

    let _ = std::fs::remove_file(path);
}

fn create_zip(path: &PathBuf) {
    let file = File::create(path).expect("create zip");
    let mut zip = zip::ZipWriter::new(file);
    let options: FileOptions<'_, ()> = FileOptions::default();
    zip.start_file("hello.txt", options).expect("start");
    zip.write_all(b"hello").expect("write");
    zip.finish().expect("finish");
}

fn temp_zip_path() -> PathBuf {
    let mut path = std::env::temp_dir();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_millis();
    path.push(format!("caliberate-test-{timestamp}.zip"));
    path
}
