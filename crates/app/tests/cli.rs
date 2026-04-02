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

#[test]
fn calibredb_list_categories() {
    let (temp_dir, config_path) = setup_library_config();
    let exe = env!("CARGO_BIN_EXE_calibredb");

    let book_path = temp_dir.path().join("categories.epub");
    std::fs::write(&book_path, b"categories").expect("write categories book");
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

    let config = ControlPlane::load_from_path(&config_path).expect("load config");
    let mut db = Database::open_with_fts(&config.db, &config.fts).expect("open db");
    db.add_book_authors(id, &["Ada Lovelace".to_string()])
        .expect("add author");
    db.add_book_tags(id, &["math".to_string()])
        .expect("add tag");
    db.set_book_series(id, "Analytical Engine", 1.0)
        .expect("set series");
    db.set_book_publisher(id, "History Press")
        .expect("set publisher");
    db.set_book_rating(id, 5).expect("set rating");
    db.set_book_languages(id, &["en".to_string()])
        .expect("set languages");

    let output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "list-categories",
            "--category",
            "authors",
        ])
        .output()
        .expect("run calibredb list-categories");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Ada Lovelace"));
}

#[test]
fn calibredb_check_library() {
    let (temp_dir, config_path) = setup_library_config();
    let exe = env!("CARGO_BIN_EXE_calibredb");

    let book_path = temp_dir.path().join("check.epub");
    std::fs::write(&book_path, b"check").expect("write check book");
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

    let output = Command::new(exe)
        .args(["--config", config_path.to_str().unwrap(), "check-library"])
        .output()
        .expect("run calibredb check-library");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Library check OK"));
}

#[test]
fn calibredb_export_command() {
    let (temp_dir, config_path) = setup_library_config();
    let exe = env!("CARGO_BIN_EXE_calibredb");

    let book_path = temp_dir.path().join("export.epub");
    std::fs::write(&book_path, b"export").expect("write export book");
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

    let export_dir = temp_dir.path().join("exported");
    let output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "export",
            "--id",
            &id.to_string(),
            "--output-dir",
            export_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run calibredb export");
    assert!(output.status.success());
    assert!(export_dir.join(format!("book-{id}")).exists());
}

#[test]
fn calibredb_backup_metadata_command() {
    let (temp_dir, config_path) = setup_library_config();
    let exe = env!("CARGO_BIN_EXE_calibredb");

    let book_path = temp_dir.path().join("backup.epub");
    std::fs::write(&book_path, b"backup").expect("write backup book");
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

    let backup_dir = temp_dir.path().join("metadata");
    let output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "backup-metadata",
            "--id",
            &id.to_string(),
            "--output-dir",
            backup_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run calibredb backup-metadata");
    assert!(output.status.success());
    let metadata_path = backup_dir.join(format!("metadata-{id}.json"));
    assert!(metadata_path.exists());
    let contents = std::fs::read_to_string(metadata_path).expect("read metadata");
    assert!(contents.contains("backup.epub"));
}

#[test]
fn calibredb_catalog_command() {
    let (temp_dir, config_path) = setup_library_config();
    let exe = env!("CARGO_BIN_EXE_calibredb");

    let book_path = temp_dir.path().join("catalog.epub");
    std::fs::write(&book_path, b"catalog").expect("write catalog book");
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

    let output_path = temp_dir.path().join("catalog.csv");
    let output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "catalog",
            "--id",
            &id.to_string(),
            "--output",
            output_path.to_str().unwrap(),
        ])
        .output()
        .expect("run calibredb catalog");
    assert!(output.status.success());
    let contents = std::fs::read_to_string(output_path).expect("read catalog");
    assert!(contents.contains("title"));
    assert!(contents.contains("catalog"));
}

#[test]
fn calibredb_saved_searches() {
    let (_temp_dir, config_path) = setup_library_config();
    let exe = env!("CARGO_BIN_EXE_calibredb");

    let list_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "saved-searches",
            "list",
        ])
        .output()
        .expect("run calibredb saved-searches list");
    assert!(list_output.status.success());
    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(list_stdout.contains("No saved searches"));

    let add_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "saved-searches",
            "add",
            "--name",
            "favorites",
            "--query",
            "title:favorites",
        ])
        .output()
        .expect("run calibredb saved-searches add");
    assert!(add_output.status.success());

    let list_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "saved-searches",
            "list",
        ])
        .output()
        .expect("run calibredb saved-searches list after add");
    assert!(list_output.status.success());
    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(list_stdout.contains("favorites"));
    assert!(list_stdout.contains("title:favorites"));

    let remove_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "saved-searches",
            "remove",
            "--name",
            "favorites",
        ])
        .output()
        .expect("run calibredb saved-searches remove");
    assert!(remove_output.status.success());
}

#[test]
fn calibredb_fts_search() {
    let (temp_dir, config_path) = setup_library_config_with_fts();
    let exe = env!("CARGO_BIN_EXE_calibredb");

    let book_path = temp_dir.path().join("fts.epub");
    std::fs::write(&book_path, b"fts").expect("write fts book");
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

    let output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "fts",
            "search",
            "--query",
            "fts",
        ])
        .output()
        .expect("run calibredb fts search");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("fts"));
}

#[test]
fn calibredb_fts_enable_disable() {
    let (_temp_dir, config_path) = setup_library_config();
    let exe = env!("CARGO_BIN_EXE_calibredb");

    let enable_output = Command::new(exe)
        .args(["--config", config_path.to_str().unwrap(), "fts", "enable"])
        .output()
        .expect("run calibredb fts enable");
    assert!(enable_output.status.success());
    let config = ControlPlane::load_from_path(&config_path).expect("load config");
    assert!(config.fts.enabled);

    let disable_output = Command::new(exe)
        .args(["--config", config_path.to_str().unwrap(), "fts", "disable"])
        .output()
        .expect("run calibredb fts disable");
    assert!(disable_output.status.success());
    let config = ControlPlane::load_from_path(&config_path).expect("load config");
    assert!(!config.fts.enabled);
}

#[test]
fn calibredb_custom_columns_and_set_custom() {
    let (temp_dir, config_path) = setup_library_config();
    let exe = env!("CARGO_BIN_EXE_calibredb");

    let add_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "custom-columns",
            "add",
            "--label",
            "rating_text",
            "--name",
            "Rating Text",
            "--datatype",
            "text",
        ])
        .output()
        .expect("run calibredb custom-columns add");
    assert!(add_output.status.success());

    let list_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "custom-columns",
            "list",
        ])
        .output()
        .expect("run calibredb custom-columns list");
    assert!(list_output.status.success());
    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(list_stdout.contains("rating_text"));

    let book_path = temp_dir.path().join("custom.epub");
    std::fs::write(&book_path, b"custom").expect("write custom book");
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

    let set_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "set-custom",
            "--id",
            &id.to_string(),
            "--label",
            "rating_text",
            "--value",
            "Loved it",
        ])
        .output()
        .expect("run calibredb set-custom");
    assert!(set_output.status.success());

    let config = ControlPlane::load_from_path(&config_path).expect("load config");
    let db = Database::open_with_fts(&config.db, &config.fts).expect("open db");
    let value = db
        .get_custom_value(id, "rating_text")
        .expect("get custom value");
    assert_eq!(value.as_deref(), Some("Loved it"));

    let remove_output = Command::new(exe)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "custom-columns",
            "remove",
            "--label",
            "rating_text",
        ])
        .output()
        .expect("run calibredb custom-columns remove");
    assert!(remove_output.status.success());
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

fn setup_library_config_with_fts() -> (TempDir, std::path::PathBuf) {
    let (temp_dir, config_path) = setup_library_config();
    let mut config = ControlPlane::load_from_path(&config_path).expect("load config");
    config.fts.enabled = true;
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
