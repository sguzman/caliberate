use caliberate_device::detection::DeviceInfo;
use caliberate_device::sync::{cleanup_device_orphans, list_device_entries, send_to_device};
use tempfile::TempDir;

#[test]
fn sends_and_lists_device_files() {
    let temp = TempDir::new().expect("tempdir");
    let device_root = temp.path().join("device");
    let library_path = device_root.join("Caliberate Library");
    std::fs::create_dir_all(&library_path).expect("create library");
    let source_path = temp.path().join("book.epub");
    std::fs::write(&source_path, b"content").expect("write source");

    let device = DeviceInfo {
        name: "device".to_string(),
        mount_path: device_root.clone(),
        library_path: library_path.clone(),
    };
    let result = send_to_device(&source_path, &device, None).expect("send");
    assert!(result.destination.exists());

    let entries = list_device_entries(&device).expect("list");
    assert_eq!(entries.len(), 1);
}

#[test]
fn cleans_device_orphans() {
    let temp = TempDir::new().expect("tempdir");
    let device_root = temp.path().join("device");
    let library_path = device_root.join("Caliberate Library");
    std::fs::create_dir_all(&library_path).expect("create library");
    std::fs::write(library_path.join("keep.epub"), b"keep").expect("write keep");
    std::fs::write(library_path.join("remove.epub"), b"remove").expect("write remove");

    let device = DeviceInfo {
        name: "device".to_string(),
        mount_path: device_root.clone(),
        library_path: library_path.clone(),
    };
    let removed = cleanup_device_orphans(&device, &vec!["keep.epub".to_string()]).expect("cleanup");
    assert_eq!(removed, 1);
    assert!(library_path.join("keep.epub").exists());
    assert!(!library_path.join("remove.epub").exists());
}
