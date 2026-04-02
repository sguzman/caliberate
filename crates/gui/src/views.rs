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
    all_books: Vec<BookRow>,
    available_formats: Vec<String>,
    selected: Option<i64>,
    details: Option<BookDetails>,
    edit_mode: bool,
    edit: EditState,
    format_filter: Option<String>,
    sort_mode: SortMode,
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
            all_books: Vec::new(),
            available_formats: Vec::new(),
            selected: None,
            details: None,
            edit_mode: false,
            edit: EditState::default(),
            format_filter: None,
            sort_mode: SortMode::TitleAsc,
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
                    ui.label("Sort");
                    egui::ComboBox::from_id_salt("sort_mode")
                        .selected_text(self.sort_mode.label())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.sort_mode,
                                SortMode::TitleAsc,
                                "Title A-Z",
                            );
                            ui.selectable_value(
                                &mut self.sort_mode,
                                SortMode::TitleDesc,
                                "Title Z-A",
                            );
                            ui.selectable_value(
                                &mut self.sort_mode,
                                SortMode::FormatAsc,
                                "Format A-Z",
                            );
                            ui.selectable_value(&mut self.sort_mode, SortMode::IdAsc, "ID Asc");
                            ui.selectable_value(&mut self.sort_mode, SortMode::IdDesc, "ID Desc");
                        });
                    if ui.button("Apply").clicked() {
                        self.apply_filters();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Format");
                    egui::ComboBox::from_id_salt("format_filter")
                        .selected_text(self.format_filter.as_deref().unwrap_or("All formats"))
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(self.format_filter.is_none(), "All formats")
                                .clicked()
                            {
                                self.format_filter = None;
                            }
                            for format in &self.available_formats {
                                if ui
                                    .selectable_label(
                                        self.format_filter.as_deref() == Some(format.as_str()),
                                        format,
                                    )
                                    .clicked()
                                {
                                    self.format_filter = Some(format.clone());
                                }
                            }
                        });
                    if ui.button("Apply").clicked() {
                        self.apply_filters();
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

        let mut action = None;
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Details");
            ui.separator();
            match &self.details {
                Some(details) => {
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(!self.edit_mode, egui::Button::new("Edit"))
                            .clicked()
                        {
                            action = Some(DetailAction::BeginEdit);
                        }
                        if ui
                            .add_enabled(self.edit_mode, egui::Button::new("Save"))
                            .clicked()
                        {
                            action = Some(DetailAction::SaveEdit);
                        }
                        if ui
                            .add_enabled(self.edit_mode, egui::Button::new("Cancel"))
                            .clicked()
                        {
                            action = Some(DetailAction::CancelEdit);
                        }
                    });
                    ui.separator();
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

                    if self.edit_mode {
                        ui.separator();
                        ui.heading("Edit Metadata");
                        ui.label("Title");
                        ui.text_edit_singleline(&mut self.edit.title);
                        ui.label("Authors (comma separated)");
                        ui.text_edit_singleline(&mut self.edit.authors);
                        ui.label("Tags (comma separated)");
                        ui.text_edit_singleline(&mut self.edit.tags);
                        ui.label("Series");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.edit.series_name);
                            ui.add(
                                egui::DragValue::new(&mut self.edit.series_index)
                                    .speed(0.1)
                                    .range(0.0..=999.0),
                            );
                        });
                        ui.label("Identifiers (one per line, format: type:value)");
                        ui.text_edit_multiline(&mut self.edit.identifiers);
                        ui.label("Comment");
                        ui.text_edit_multiline(&mut self.edit.comment);
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

        if let Some(action) = action {
            match action {
                DetailAction::BeginEdit => {
                    self.begin_edit();
                }
                DetailAction::CancelEdit => {
                    self.cancel_edit();
                }
                DetailAction::SaveEdit => {
                    if let Err(err) = self.save_edit() {
                        self.set_error(err);
                    } else {
                        self.clear_error();
                    }
                }
            }
        }
    }

    fn refresh_books(&mut self) -> CoreResult<()> {
        let query = self.search_query.trim().to_string();
        let list = if query.is_empty() {
            self.db.list_books()?
        } else {
            self.db.search_books(&query)?
        };
        self.all_books = list
            .into_iter()
            .map(|book| BookRow {
                id: book.id,
                title: book.title,
                format: book.format,
            })
            .collect();
        self.available_formats = self
            .all_books
            .iter()
            .map(|book| book.format.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        self.apply_filters();
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
        self.edit = EditState::from_details(self.details.as_ref().expect("details"));
        self.edit_mode = false;
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

    fn apply_filters(&mut self) {
        let mut list: Vec<BookRow> = self
            .all_books
            .iter()
            .filter(|book| {
                if let Some(format) = &self.format_filter {
                    book.format.eq_ignore_ascii_case(format)
                } else {
                    true
                }
            })
            .cloned()
            .collect();
        self.sort_mode.sort(&mut list);
        self.books = list;
    }

    fn begin_edit(&mut self) {
        if let Some(details) = &self.details {
            self.edit = EditState::from_details(details);
            self.edit_mode = true;
        }
    }

    fn cancel_edit(&mut self) {
        if let Some(details) = &self.details {
            self.edit = EditState::from_details(details);
        }
        self.edit_mode = false;
        self.status = "Edit cancelled".to_string();
    }

    fn save_edit(&mut self) -> CoreResult<()> {
        let Some(details) = &self.details else {
            return Ok(());
        };
        let book_id = details.book.id;
        let title = self.edit.title.trim();
        if title.is_empty() {
            return Err(CoreError::ConfigValidate(
                "title cannot be empty".to_string(),
            ));
        }
        self.db.update_book_title(book_id, title)?;
        let authors = parse_list(&self.edit.authors);
        let tags = parse_list(&self.edit.tags);
        let identifiers = parse_identifiers(&self.edit.identifiers);
        self.db.replace_book_authors(book_id, &authors)?;
        self.db.replace_book_tags(book_id, &tags)?;
        if self.edit.series_name.trim().is_empty() {
            self.db.clear_book_series(book_id)?;
        } else {
            self.db.set_book_series(
                book_id,
                self.edit.series_name.trim(),
                self.edit.series_index,
            )?;
        }
        self.db.replace_book_identifiers(book_id, &identifiers)?;
        let comment = self.edit.comment.trim();
        if comment.is_empty() {
            self.db.clear_book_comment(book_id)?;
        } else {
            self.db.set_book_comment(book_id, comment)?;
        }
        self.status = "Metadata saved".to_string();
        self.edit_mode = false;
        self.refresh_books()?;
        self.load_details(book_id)?;
        info!(component = "gui", book_id, "saved metadata edits");
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct EditState {
    title: String,
    authors: String,
    tags: String,
    series_name: String,
    series_index: f64,
    identifiers: String,
    comment: String,
}

impl Default for EditState {
    fn default() -> Self {
        Self {
            title: String::new(),
            authors: String::new(),
            tags: String::new(),
            series_name: String::new(),
            series_index: 1.0,
            identifiers: String::new(),
            comment: String::new(),
        }
    }
}

impl EditState {
    fn from_details(details: &BookDetails) -> Self {
        let identifiers = details
            .identifiers
            .iter()
            .map(|entry| format!("{}:{}", entry.id_type, entry.value))
            .collect::<Vec<_>>()
            .join("\n");
        Self {
            title: details.book.title.clone(),
            authors: details.authors.join(", "),
            tags: details.tags.join(", "),
            series_name: details
                .series
                .as_ref()
                .map(|s| s.name.clone())
                .unwrap_or_default(),
            series_index: details.series.as_ref().map(|s| s.index).unwrap_or(1.0),
            identifiers,
            comment: details.comment.clone().unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum DetailAction {
    BeginEdit,
    SaveEdit,
    CancelEdit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortMode {
    TitleAsc,
    TitleDesc,
    FormatAsc,
    IdAsc,
    IdDesc,
}

impl SortMode {
    fn label(self) -> &'static str {
        match self {
            SortMode::TitleAsc => "Title A-Z",
            SortMode::TitleDesc => "Title Z-A",
            SortMode::FormatAsc => "Format A-Z",
            SortMode::IdAsc => "ID Asc",
            SortMode::IdDesc => "ID Desc",
        }
    }

    fn sort(self, books: &mut [BookRow]) {
        match self {
            SortMode::TitleAsc => {
                books.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            }
            SortMode::TitleDesc => {
                books.sort_by(|a, b| b.title.to_lowercase().cmp(&a.title.to_lowercase()))
            }
            SortMode::FormatAsc => {
                books.sort_by(|a, b| a.format.to_lowercase().cmp(&b.format.to_lowercase()))
            }
            SortMode::IdAsc => books.sort_by_key(|book| book.id),
            SortMode::IdDesc => books.sort_by_key(|book| std::cmp::Reverse(book.id)),
        }
    }
}

fn parse_list(input: &str) -> Vec<String> {
    input
        .split(|c| c == ',' || c == '\n')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect()
}

fn parse_identifiers(input: &str) -> Vec<(String, String)> {
    let mut items = Vec::new();
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (id_type, value) = if let Some((left, right)) = line.split_once(':') {
            (left, right)
        } else if let Some((left, right)) = line.split_once('=') {
            (left, right)
        } else {
            continue;
        };
        let id_type = id_type.trim();
        let value = value.trim();
        if id_type.is_empty() || value.is_empty() {
            continue;
        }
        items.push((id_type.to_string(), value.to_string()));
    }
    items
}

#[cfg(test)]
mod tests {
    use super::{parse_identifiers, parse_list};

    #[test]
    fn parses_list_values() {
        let items = parse_list("Alice, Bob\nCara");
        assert_eq!(
            items,
            vec!["Alice".to_string(), "Bob".to_string(), "Cara".to_string()]
        );
    }

    #[test]
    fn parses_identifiers_lines() {
        let items = parse_identifiers("isbn:123\nasin=456\nbadline");
        assert_eq!(
            items,
            vec![
                ("isbn".to_string(), "123".to_string()),
                ("asin".to_string(), "456".to_string())
            ]
        );
    }
}
