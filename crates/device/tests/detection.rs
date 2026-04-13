use caliberate_core::config::DeviceConfig;
use caliberate_device::detection::detect_devices;
use tempfile::TempDir;

#[test]
fn detects_devices_with_library_dir() {
    let temp = TempDir::new().expect("tempdir");
    let root = temp.path().join("mounts");
    std::fs::create_dir_all(&root).expect("create root");
    let device_path = root.join("device-one");
    let library_dir = device_path.join("Caliberate Library");
    std::fs::create_dir_all(&library_dir).expect("create library");

    let config = DeviceConfig {
        mount_roots: vec![root.clone()],
        library_subdir: "Caliberate Library".to_string(),
        send_auto_convert: false,
        send_overwrite: false,
        sync_metadata: true,
        sync_cover: true,
        scan_recursive: true,
        driver_backend: "auto".to_string(),
        connection_timeout_ms: 5_000,
    };
    let devices = detect_devices(&config).expect("detect");
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].mount_path, device_path);
    assert_eq!(devices[0].library_path, library_dir);
}
