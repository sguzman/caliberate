use caliberate_core::config::ControlPlane;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn calibredb_check_config() {
    let exe = env!("CARGO_BIN_EXE_calibredb");
    let config_path = workspace_config();
    let status = Command::new(exe)
        .args(["--config", config_path.to_str().unwrap(), "check-config"])
        .status()
        .expect("run calibredb");
    assert!(status.success());
}

#[test]
fn calibre_server_check_config() {
    let exe = env!("CARGO_BIN_EXE_calibre-server");
    let config_path = workspace_config();
    let status = Command::new(exe)
        .args(["--config", config_path.to_str().unwrap(), "check-config"])
        .status()
        .expect("run calibre-server");
    assert!(status.success());
}

#[test]
fn calibredb_device_commands() {
    let (temp_dir, config_path, device_name, library_dir) = setup_device_config();
    let exe = env!("CARGO_BIN_EXE_calibredb");

    let output = Command::new(exe)
        .args(["--config", config_path.to_str().unwrap(), "device", "list"])
        .output()
        .expect("run calibredb device list");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(device_name));

    let list_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "device",
            "list-files",
            "--device",
            device_name,
        ])
        .output()
        .expect("run calibredb device list-files");
    assert!(list_output.status.success());
    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(list_stdout.contains("keep.epub"));

    let send_path = temp_dir.path().join("send.epub");
    std::fs::write(&send_path, b"send").expect("write send file");
    let send_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "device",
            "send",
            "--device",
            device_name,
            "--path",
            send_path.to_str().unwrap(),
            "--dest-name",
            "sent.epub",
        ])
        .output()
        .expect("run calibredb device send");
    assert!(send_output.status.success());
    assert!(library_dir.join("sent.epub").exists());

    let orphan_path = library_dir.join("orphan.epub");
    std::fs::write(&orphan_path, b"orphan").expect("write orphan file");
    let cleanup_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "device",
            "cleanup",
            "--device",
            device_name,
            "--keep",
            "keep.epub",
        ])
        .output()
        .expect("run calibredb device cleanup");
    assert!(cleanup_output.status.success());
    assert!(library_dir.join("keep.epub").exists());
    assert!(!library_dir.join("orphan.epub").exists());
}

#[test]
fn calibredb_format_commands() {
    let (temp_dir, config_path) = setup_library_config();
    let exe = env!("CARGO_BIN_EXE_calibredb");

    let book_path = temp_dir.path().join("book.epub");
    std::fs::write(&book_path, b"book").expect("write book");
    let output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "add",
            "--path",
            book_path.to_str().unwrap(),
        ])
        .output()
        .expect("run calibredb add");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout.split_whitespace().last().expect("book id");

    let list_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "formats",
            "list",
            "--id",
            id,
        ])
        .output()
        .expect("run calibredb formats list");
    assert!(list_output.status.success());
    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(list_stdout.contains("epub"));

    let add_path = temp_dir.path().join("book.pdf");
    std::fs::write(&add_path, b"pdf").expect("write pdf");
    let add_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "formats",
            "add",
            "--id",
            id,
            "--path",
            add_path.to_str().unwrap(),
        ])
        .output()
        .expect("run calibredb formats add");
    assert!(add_output.status.success());

    let list_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "formats",
            "list",
            "--id",
            id,
        ])
        .output()
        .expect("run calibredb formats list");
    assert!(list_output.status.success());
    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(list_stdout.contains("pdf"));

    let remove_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "formats",
            "remove",
            "--id",
            id,
            "--format",
            "pdf",
        ])
        .output()
        .expect("run calibredb formats remove");
    assert!(remove_output.status.success());

    let list_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "formats",
            "list",
            "--id",
            id,
        ])
        .output()
        .expect("run calibredb formats list");
    assert!(list_output.status.success());
    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(!list_stdout.contains("pdf"));
}

fn workspace_config() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("config")
        .join("control-plane.toml")
}

fn setup_device_config() -> (
    TempDir,
    std::path::PathBuf,
    &'static str,
    std::path::PathBuf,
) {
    let temp_dir = tempfile::Builder::new()
        .prefix("caliberate-test-device-")
        .tempdir()
        .expect("tempdir");
    let device_root = temp_dir.path().join("devices");
    let device_name = "Kobo";
    let device_mount = device_root.join(device_name);
    let library_dir = device_mount.join("Caliberate Library");
    std::fs::create_dir_all(&library_dir).expect("create device library");
    std::fs::write(library_dir.join("keep.epub"), b"keep").expect("write keep file");

    let fixture = workspace_root().join("crates/core/tests/fixtures/control-plane.toml");
    let mut config = ControlPlane::load_from_path(&fixture).expect("load fixture");
    let data_dir = temp_dir.path().join("data");
    let cache_dir = temp_dir.path().join("cache");
    let log_dir = temp_dir.path().join("logs");
    let tmp_dir = temp_dir.path().join("tmp");
    let library_dir_root = temp_dir.path().join("library");
    std::fs::create_dir_all(&data_dir).expect("create data dir");
    std::fs::create_dir_all(&cache_dir).expect("create cache dir");
    std::fs::create_dir_all(&log_dir).expect("create log dir");
    std::fs::create_dir_all(&tmp_dir).expect("create tmp dir");
    std::fs::create_dir_all(&library_dir_root).expect("create library dir");

    config.paths.data_dir = data_dir.clone();
    config.paths.cache_dir = cache_dir;
    config.paths.log_dir = log_dir;
    config.paths.tmp_dir = tmp_dir;
    config.paths.library_dir = library_dir_root;
    config.db.sqlite_path = data_dir.join("caliberate.db");
    config.device.mount_roots = vec![device_root];

    let config_path = temp_dir.path().join("control-plane.toml");
    config.save_to_path(&config_path).expect("write config");

    (temp_dir, config_path, device_name, library_dir)
}

fn setup_library_config() -> (TempDir, std::path::PathBuf) {
    let temp_dir = tempfile::Builder::new()
        .prefix("caliberate-test-library-")
        .tempdir()
        .expect("tempdir");
    let fixture = workspace_root().join("crates/core/tests/fixtures/control-plane.toml");
    let mut config = ControlPlane::load_from_path(&fixture).expect("load fixture");
    let data_dir = temp_dir.path().join("data");
    let cache_dir = temp_dir.path().join("cache");
    let log_dir = temp_dir.path().join("logs");
    let tmp_dir = temp_dir.path().join("tmp");
    let library_dir = temp_dir.path().join("library");
    std::fs::create_dir_all(&data_dir).expect("create data dir");
    std::fs::create_dir_all(&cache_dir).expect("create cache dir");
    std::fs::create_dir_all(&log_dir).expect("create log dir");
    std::fs::create_dir_all(&tmp_dir).expect("create tmp dir");
    std::fs::create_dir_all(&library_dir).expect("create library dir");

    config.paths.data_dir = data_dir.clone();
    config.paths.cache_dir = cache_dir;
    config.paths.log_dir = log_dir;
    config.paths.tmp_dir = tmp_dir;
    config.paths.library_dir = library_dir;
    config.db.sqlite_path = data_dir.join("caliberate.db");
    config.formats.supported = vec!["epub".to_string(), "pdf".to_string()];

    let config_path = temp_dir.path().join("control-plane.toml");
    config.save_to_path(&config_path).expect("write config");

    (temp_dir, config_path)
}

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}
