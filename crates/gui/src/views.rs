//! GUI views and models.

use caliberate_core::config::ControlPlane;
use caliberate_core::error::{CoreError, CoreResult};
use caliberate_db::database::{AssetRow, BookRecord, Database, IdentifierEntry, SeriesEntry};
use eframe::egui;
use tracing::info;

#[derive(Debug, Clone)]
pub struct BookRow {
    pub id: i64,
    pub title: String,
    pub format: String,
}

#[derive(Debug, Clone)]
pub struct BookDetails {
    pub book: BookRecord,
    pub assets: Vec<AssetRow>,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub series: Option<SeriesEntry>,
    pub identifiers: Vec<IdentifierEntry>,
    pub comment: Option<String>,
}

pub struct LibraryView {
    db: Database,
    books: Vec<BookRow>,
    selected: Option<i64>,
    details: Option<BookDetails>,
    search_query: String,
    status: String,
    last_error: Option<String>,
    needs_refresh: bool,
}

impl LibraryView {
    pub fn new(config: &ControlPlane) -> CoreResult<Self> {
        let db = Database::open_with_fts(&config.db, &config.fts)?;
        let mut view = Self {
            db,
            books: Vec::new(),
            selected: None,
            details: None,
            search_query: String::new(),
            status: "Ready".to_string(),
            last_error: None,
            needs_refresh: true,
        };
        view.refresh_books()?;
        Ok(view)
    }

    pub fn status_line(&self) -> (&str, Option<&str>) {
        (self.status.as_str(), self.last_error.as_deref())
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_rect_before_wrap();
        let left_width = (available.width() * 0.3).max(240.0);

        egui::Panel::left("library_list")
            .resizable(true)
            .default_size(left_width)
            .show_inside(ui, |ui| {
                ui.heading("Library");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Search");
                    ui.text_edit_singleline(&mut self.search_query);
                    if ui.button("Go").clicked() {
                        self.needs_refresh = true;
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Refresh").clicked() {
                        self.needs_refresh = true;
                    }
                    ui.label(format!("{} books", self.books.len()));
                });
                ui.separator();

                if self.needs_refresh {
                    if let Err(err) = self.refresh_books() {
                        self.set_error(err);
                        self.needs_refresh = false;
                    } else {
                        self.clear_error();
                    }
                }

                let mut selected_id = None;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for book in &self.books {
                        let selected = self.selected == Some(book.id);
                        if ui
                            .selectable_label(selected, format!("{} ({})", book.title, book.format))
                            .clicked()
                        {
                            selected_id = Some(book.id);
                        }
                    }
                });
                if let Some(book_id) = selected_id {
                    self.selected = Some(book_id);
                    if let Err(err) = self.load_details(book_id) {
                        self.set_error(err);
                    } else {
                        self.clear_error();
                    }
                }
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Details");
            ui.separator();
            match &self.details {
                Some(details) => {
                    ui.label(format!("Title: {}", details.book.title));
                    ui.label(format!("Format: {}", details.book.format));
                    ui.label(format!("Path: {}", details.book.path));
                    if details.authors.is_empty() {
                        ui.label("Authors: none");
                    } else {
                        ui.label(format!("Authors: {}", details.authors.join(", ")));
                    }
                    if details.tags.is_empty() {
                        ui.label("Tags: none");
                    } else {
                        ui.label(format!("Tags: {}", details.tags.join(", ")));
                    }
                    match &details.series {
                        Some(series) => {
                            ui.label(format!("Series: {} ({})", series.name, series.index));
                        }
                        None => {
                            ui.label("Series: none");
                        }
                    }
                    if details.identifiers.is_empty() {
                        ui.label("Identifiers: none");
                    } else {
                        ui.label("Identifiers:");
                        for identifier in &details.identifiers {
                            ui.label(format!("{}: {}", identifier.id_type, identifier.value));
                        }
                    }
                    match &details.comment {
                        Some(comment) => {
                            ui.label(format!("Comment: {comment}"));
                        }
                        None => {
                            ui.label("Comment: none");
                        }
                    }

                    ui.separator();
                    ui.heading("Assets");
                    if details.assets.is_empty() {
                        ui.label("No assets recorded.");
                    } else {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for asset in &details.assets {
                                ui.label(format!(
                                    "{} | {} | {} | {} bytes",
                                    asset.storage_mode,
                                    asset.stored_path,
                                    asset.created_at,
                                    asset.size_bytes
                                ));
                            }
                        });
                    }
                }
                None => {
                    ui.label("Select a book to view details.");
                }
            }
        });
    }

    fn refresh_books(&mut self) -> CoreResult<()> {
        let query = self.search_query.trim();
        let list = if query.is_empty() {
            self.db.list_books()?
        } else {
            self.db.search_books(query)?
        };
        self.books = list
            .into_iter()
            .map(|book| BookRow {
                id: book.id,
                title: book.title,
                format: book.format,
            })
            .collect();
        self.status = format!("Loaded {} books", self.books.len());
        self.needs_refresh = false;
        info!(
            component = "gui",
            count = self.books.len(),
            query = %query,
            "library refreshed"
        );
        Ok(())
    }

    fn load_details(&mut self, id: i64) -> CoreResult<()> {
        let Some(book) = self.db.get_book(id)? else {
            return Err(CoreError::ConfigValidate(format!("book not found: {id}")));
        };
        let assets = self.db.list_assets_for_book(id)?;
        let authors = self.db.list_book_authors(id)?;
        let tags = self.db.list_book_tags(id)?;
        let series = self.db.get_book_series(id)?;
        let identifiers = self.db.list_book_identifiers(id)?;
        let comment = self.db.get_book_comment(id)?;
        self.details = Some(BookDetails {
            book,
            assets,
            authors,
            tags,
            series,
            identifiers,
            comment,
        });
        info!(component = "gui", book_id = id, "loaded book details");
        Ok(())
    }

    fn set_error(&mut self, err: CoreError) {
        self.last_error = Some(err.to_string());
        self.status = "Error".to_string();
    }

    fn clear_error(&mut self) {
        self.last_error = None;
    }
}
