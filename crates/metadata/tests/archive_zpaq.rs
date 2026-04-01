use caliberate_core::config::FormatsConfig;
use caliberate_metadata::extract::{extract_archive_entry, extract_archive_preview};
use std::fs;
use tempfile::tempdir;

const MAGIC_16: [u8; 16] = [
    0x37, 0x6b, 0x53, 0x74, 0xa0, 0x31, 0x83, 0xd3, 0x8c, 0xb2, 0x28, 0xb0, 0xd3, b'z', b'P', b'Q',
];

fn build_unmodeled_archive(filename: &str, data: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&MAGIC_16);
    buf.push(2);
    buf.push(1);
    buf.extend_from_slice(&7u16.to_le_bytes());
    buf.extend_from_slice(&[0, 0, 0, 0, 0]);
    buf.push(0);
    buf.push(0);

    buf.push(1);
    buf.extend_from_slice(filename.as_bytes());
    buf.push(0);
    buf.push(0);
    buf.push(0);

    let payload_len = (data.len() + 1) as u32;
    buf.extend_from_slice(&payload_len.to_be_bytes());
    buf.push(0);
    buf.extend_from_slice(data);
    buf.extend_from_slice(&0u32.to_be_bytes());

    buf.push(254);
    buf.push(255);

    buf
}

#[test]
fn zpaq_preview_and_extract_unmodeled() {
    let data = build_unmodeled_archive("book.txt", b"hello");
    let dir = tempdir().expect("tempdir");
    let archive_path = dir.path().join("book.zpaq");
    fs::write(&archive_path, data).expect("write zpaq");

    let formats = FormatsConfig {
        supported: Vec::new(),
        archive_formats: vec!["zpaq".to_string()],
    };

    let preview = extract_archive_preview(&archive_path, &formats).expect("preview");
    assert_eq!(preview.entries, vec!["book.txt".to_string()]);

    let output_dir = dir.path().join("out");
    let extracted =
        extract_archive_entry(&archive_path, "book.txt", &output_dir, &formats).expect("extract");
    let contents = fs::read(extracted).expect("read extracted");
    assert_eq!(contents, b"hello");
}
