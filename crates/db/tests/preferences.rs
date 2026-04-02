use caliberate_db::database::Database;
use serde_json::json;
use tempfile::TempDir;

#[test]
fn preference_roundtrip_json() {
    let (db, _tmp) = open_db();
    let payload = json!({"saved_searches": {"favorites": "title:favorites"}});
    db.set_preference_json("prefs", &payload)
        .expect("set preference");
    let loaded = db
        .get_preference_json("prefs")
        .expect("get preference")
        .expect("missing preference");
    assert_eq!(loaded, payload);
}

#[test]
fn saved_searches_add_list_remove() {
    let (db, _tmp) = open_db();
    db.add_saved_search("recent", "timestamp:>2026-01-01")
        .expect("add saved search");
    let searches = db.list_saved_searches().expect("list searches");
    assert_eq!(
        searches.get("recent"),
        Some(&"timestamp:>2026-01-01".to_string())
    );
    let removed = db.remove_saved_search("recent").expect("remove search");
    assert!(removed);
    let searches = db.list_saved_searches().expect("list searches");
    assert!(searches.is_empty());
}

fn open_db() -> (Database, TempDir) {
    let temp_dir = tempfile::Builder::new()
        .prefix("caliberate-test-preferences-")
        .tempdir()
        .expect("tempdir");
    let path = temp_dir.path().join("prefs.db");
    let db = Database::open_path(&path, 100).expect("open db");
    (db, temp_dir)
}
