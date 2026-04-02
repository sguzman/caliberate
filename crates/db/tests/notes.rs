use caliberate_db::database::Database;
use tempfile::TempDir;

#[test]
fn adds_and_lists_notes() {
    let (mut db, _tmp) = open_db();
    let book_id = db
        .add_book(
            "Title",
            "epub",
            "/library/book.epub",
            "2026-04-01T00:00:00Z",
        )
        .expect("add book");
    let note_id = db
        .add_note(book_id, "First note", "2026-04-02T00:00:00Z")
        .expect("add note");
    let notes = db.list_notes_for_book(book_id).expect("list notes");
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, note_id);
    assert_eq!(notes[0].text, "First note");
}

#[test]
fn deletes_note() {
    let (mut db, _tmp) = open_db();
    let book_id = db
        .add_book(
            "Title",
            "epub",
            "/library/book.epub",
            "2026-04-01T00:00:00Z",
        )
        .expect("add book");
    let note_id = db
        .add_note(book_id, "First note", "2026-04-02T00:00:00Z")
        .expect("add note");
    let deleted = db.delete_note(note_id).expect("delete note");
    assert!(deleted);
    let notes = db.list_notes_for_book(book_id).expect("list notes");
    assert!(notes.is_empty());
}

fn open_db() -> (Database, TempDir) {
    let temp_dir = tempfile::Builder::new()
        .prefix("caliberate-test-notes-")
        .tempdir()
        .expect("tempdir");
    let path = temp_dir.path().join("notes.db");
    let db = Database::open_path(&path, 100).expect("open db");
    (db, temp_dir)
}
