use caliberate_core::config::ControlPlane;
use caliberate_db::database::Database;
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

#[test]
fn calibredb_notes_commands() {
    let (temp_dir, config_path) = setup_library_config();
    let exe = env!("CARGO_BIN_EXE_calibredb");

    let book_path = temp_dir.path().join("notes.epub");
    std::fs::write(&book_path, b"notes").expect("write notes book");
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
    let id = stdout
        .split_whitespace()
        .last()
        .expect("book id")
        .to_string();

    let note_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "notes",
            "add",
            "--book-id",
            &id,
            "--text",
            "Remember this",
        ])
        .output()
        .expect("run calibredb notes add");
    assert!(note_output.status.success());
    let note_stdout = String::from_utf8_lossy(&note_output.stdout);
    let note_id = note_stdout
        .split_whitespace()
        .nth(2)
        .expect("note id")
        .to_string();

    let list_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "notes",
            "list",
            "--book-id",
            &id,
        ])
        .output()
        .expect("run calibredb notes list");
    assert!(list_output.status.success());
    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(list_stdout.contains(&note_id));
    assert!(list_stdout.contains("Remember this"));

    let delete_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "notes",
            "delete",
            "--note-id",
            &note_id,
        ])
        .output()
        .expect("run calibredb notes delete");
    assert!(delete_output.status.success());

    let list_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "notes",
            "list",
            "--book-id",
            &id,
        ])
        .output()
        .expect("run calibredb notes list after delete");
    assert!(list_output.status.success());
    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(list_stdout.contains("No notes for book"));
}

#[test]
fn calibredb_set_title() {
    let (temp_dir, config_path) = setup_library_config();
    let exe = env!("CARGO_BIN_EXE_calibredb");

    let book_path = temp_dir.path().join("set-title.epub");
    std::fs::write(&book_path, b"title").expect("write title book");
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
    let id = stdout
        .split_whitespace()
        .last()
        .expect("book id")
        .parse::<i64>()
        .expect("book id int");

    let update_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "set",
            "title",
            "--id",
            &id.to_string(),
            "--title",
            "Updated Title",
        ])
        .output()
        .expect("run calibredb set title");
    assert!(update_output.status.success());

    let config = ControlPlane::load_from_path(&config_path).expect("load config");
    let db = Database::open_with_fts(&config.db, &config.fts).expect("open db");
    let book = db.get_book(id).expect("get book").expect("book");
    assert_eq!(book.title, "Updated Title");
}

#[test]
fn calibredb_set_identifiers() {
    let (temp_dir, config_path) = setup_library_config();
    let exe = env!("CARGO_BIN_EXE_calibredb");

    let book_path = temp_dir.path().join("set-ident.epub");
    std::fs::write(&book_path, b"ident").expect("write ident book");
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
    let id = stdout
        .split_whitespace()
        .last()
        .expect("book id")
        .parse::<i64>()
        .expect("book id int");

    let update_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "set",
            "identifiers",
            "--id",
            &id.to_string(),
            "--identifier",
            "isbn=1234567890",
            "--identifier",
            "asin=B00TEST",
        ])
        .output()
        .expect("run calibredb set identifiers");
    assert!(update_output.status.success());

    let config = ControlPlane::load_from_path(&config_path).expect("load config");
    let db = Database::open_with_fts(&config.db, &config.fts).expect("open db");
    let identifiers = db.list_book_identifiers(id).expect("list identifiers");
    assert!(identifiers.iter().any(|i| i.id_type == "isbn"));
    assert!(identifiers.iter().any(|i| i.id_type == "asin"));
}

#[test]
fn calibredb_set_dates() {
    let (temp_dir, config_path) = setup_library_config();
    let exe = env!("CARGO_BIN_EXE_calibredb");

    let book_path = temp_dir.path().join("set-dates.epub");
    std::fs::write(&book_path, b"dates").expect("write dates book");
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
    let id = stdout
        .split_whitespace()
        .last()
        .expect("book id")
        .parse::<i64>()
        .expect("book id int");

    let update_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "set",
            "dates",
            "--id",
            &id.to_string(),
            "--timestamp",
            "2026-04-01T00:00:00Z",
            "--pubdate",
            "2026-03-01T00:00:00Z",
            "--last-modified",
            "2026-04-02T00:00:00Z",
        ])
        .output()
        .expect("run calibredb set dates");
    assert!(update_output.status.success());

    let config = ControlPlane::load_from_path(&config_path).expect("load config");
    let db = Database::open_with_fts(&config.db, &config.fts).expect("open db");
    let extras = db.get_book_extras(id).expect("extras");
    assert_eq!(extras.timestamp.as_deref(), Some("2026-04-01T00:00:00Z"));
    assert_eq!(extras.pubdate.as_deref(), Some("2026-03-01T00:00:00Z"));
    assert_eq!(
        extras.last_modified.as_deref(),
        Some("2026-04-02T00:00:00Z")
    );
}

#[test]
fn ebook_convert_list_and_info() {
    let (_temp_dir, config_path) = setup_library_config();
    let exe = env!("CARGO_BIN_EXE_ebook-convert");

    let list_output = Command::new(exe)
        .args(["--config", config_path.to_str().unwrap(), "--list-formats"])
        .output()
        .expect("run ebook-convert list-formats");
    assert!(list_output.status.success());
    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(list_stdout.contains("epub"));

    let archive_output = Command::new(exe)
        .args(["--config", config_path.to_str().unwrap(), "--list-archives"])
        .output()
        .expect("run ebook-convert list-archives");
    assert!(archive_output.status.success());
    let archive_stdout = String::from_utf8_lossy(&archive_output.stdout);
    assert!(archive_stdout.contains("zip"));

    let info_output = Command::new(exe)
        .args(["--config", config_path.to_str().unwrap(), "--info"])
        .output()
        .expect("run ebook-convert info");
    assert!(info_output.status.success());
    let info_stdout = String::from_utf8_lossy(&info_output.stdout);
    assert!(info_stdout.contains("Conversion enabled"));
}

#[test]
fn calibre_server_user_commands() {
    let (_temp_dir, config_path) = setup_library_config();
    let exe = env!("CARGO_BIN_EXE_calibre-server");

    let list_output = Command::new(exe)
        .args(["--config", config_path.to_str().unwrap(), "users", "list"])
        .output()
        .expect("run calibre-server users list");
    assert!(list_output.status.success());
    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(list_stdout.contains("No API keys configured"));

    let add_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "users",
            "add",
            "--key",
            "abc123",
        ])
        .output()
        .expect("run calibre-server users add");
    assert!(add_output.status.success());

    let list_output = Command::new(exe)
        .args(["--config", config_path.to_str().unwrap(), "users", "list"])
        .output()
        .expect("run calibre-server users list after add");
    assert!(list_output.status.success());
    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(list_stdout.contains("abc123"));

    let remove_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "users",
            "remove",
            "--key",
            "abc123",
        ])
        .output()
        .expect("run calibre-server users remove");
    assert!(remove_output.status.success());

    let list_output = Command::new(exe)
        .args(["--config", config_path.to_str().unwrap(), "users", "list"])
        .output()
        .expect("run calibre-server users list after remove");
    assert!(list_output.status.success());
    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(list_stdout.contains("No API keys configured"));
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
