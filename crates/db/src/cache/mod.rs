//! Cache layer for metadata access.

use crate::database::NoteRecord;
use crate::database::{AssetRow, BookExtras, BookRecord, Database, IdentifierEntry, SeriesEntry};
use caliberate_core::error::CoreResult;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct BookDetailsCached {
    pub book: BookRecord,
    pub assets: Vec<AssetRow>,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub series: Option<SeriesEntry>,
    pub identifiers: Vec<IdentifierEntry>,
    pub comment: Option<String>,
    pub extras: BookExtras,
    pub notes: Vec<NoteRecord>,
}

#[derive(Debug, Default)]
pub struct MetadataCache {
    books: Vec<BookRecord>,
    details: HashMap<i64, BookDetailsCached>,
}

impl MetadataCache {
    pub fn new() -> Self {
        Self {
            books: Vec::new(),
            details: HashMap::new(),
        }
    }

    pub fn refresh_books(&mut self, db: &Database) -> CoreResult<()> {
        self.books = db.list_books()?;
        self.details.clear();
        Ok(())
    }

    pub fn list_books(&self) -> &[BookRecord] {
        &self.books
    }

    pub fn get_book_details(
        &mut self,
        db: &Database,
        book_id: i64,
    ) -> CoreResult<Option<&BookDetailsCached>> {
        if !self.details.contains_key(&book_id) {
            if let Some(details) = Self::load_details(db, book_id)? {
                self.details.insert(book_id, details);
            }
        }
        Ok(self.details.get(&book_id))
    }

    pub fn invalidate_book(&mut self, book_id: i64) {
        self.details.remove(&book_id);
    }

    fn load_details(db: &Database, book_id: i64) -> CoreResult<Option<BookDetailsCached>> {
        let Some(book) = db.get_book(book_id)? else {
            return Ok(None);
        };
        let assets = db.list_assets_for_book(book_id)?;
        let authors = db.list_book_authors(book_id)?;
        let tags = db.list_book_tags(book_id)?;
        let series = db.get_book_series(book_id)?;
        let identifiers = db.list_book_identifiers(book_id)?;
        let comment = db.get_book_comment(book_id)?;
        let extras = db.get_book_extras(book_id)?;
        let notes = db.list_notes_for_book(book_id)?;
        Ok(Some(BookDetailsCached {
            book,
            assets,
            authors,
            tags,
            series,
            identifiers,
            comment,
            extras,
            notes,
        }))
    }
}
