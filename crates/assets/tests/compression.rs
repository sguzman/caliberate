use caliberate_assets::compression::{compress_file, decompress_file};
use std::fs;
use tempfile::tempdir;

#[test]
fn compress_and_decompress_roundtrip() {
    let dir = tempdir().expect("temp dir");
    let source = dir.path().join("sample.txt");
    fs::write(&source, b"caliberate compression").expect("write source");

    let compressed = dir.path().join("sample.txt.zst");
    let written = compress_file(&source, &compressed, 3).expect("compress");
    assert!(written > 0);

    let decompressed = dir.path().join("sample.out");
    let decoded = decompress_file(&compressed, &decompressed).expect("decompress");
    assert!(decoded > 0);

    let data = fs::read(&decompressed).expect("read decompressed");
    assert_eq!(data, b"caliberate compression");
}
