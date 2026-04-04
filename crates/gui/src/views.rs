//! GUI views and models.

use caliberate_core::config::{ControlPlane, GuiConfig};
use caliberate_core::error::{CoreError, CoreResult};
use caliberate_db::cache::MetadataCache;
use caliberate_db::database::{AssetRow, BookRecord, Database, IdentifierEntry, SeriesEntry};
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::info;

#[derive(Debug, Clone)]
pub struct BookRow {
    pub id: i64,
    pub title: String,
    pub format: String,
    pub path: String,
    pub authors: String,
    pub tags: String,
    pub series: String,
    pub rating: String,
    pub publisher: String,
    pub languages: String,
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
    pub extras: caliberate_db::database::BookExtras,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Table,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortMode {
    Title,
    Authors,
    Series,
    Tags,
    Formats,
    Rating,
    Publisher,
    Languages,
    Id,
}

impl SortMode {
    fn label(&self) -> &'static str {
        match self {
            SortMode::Title => "Title",
            SortMode::Authors => "Authors",
            SortMode::Series => "Series",
            SortMode::Tags => "Tags",
            SortMode::Formats => "Formats",
            SortMode::Rating => "Rating",
            SortMode::Publisher => "Publisher",
            SortMode::Languages => "Languages",
            SortMode::Id => "ID",
        }
    }
}

#[derive(Debug, Clone)]
struct ColumnVisibility {
    title: bool,
    authors: bool,
    series: bool,
    tags: bool,
    formats: bool,
    rating: bool,
    publisher: bool,
    languages: bool,
}

#[derive(Debug, Clone)]
struct ColumnWidths {
    title: f32,
    authors: f32,
    series: f32,
    tags: f32,
    formats: f32,
    rating: f32,
    publisher: f32,
    languages: f32,
}

impl ColumnVisibility {
    fn from_config(gui: &GuiConfig) -> Self {
        Self {
            title: gui.show_title,
            authors: gui.show_authors,
            series: gui.show_series,
            tags: gui.show_tags,
            formats: gui.show_formats,
            rating: gui.show_rating,
            publisher: gui.show_publisher,
            languages: gui.show_languages,
        }
    }

    fn apply_to_config(&self, gui: &mut GuiConfig) {
        gui.show_title = self.title;
        gui.show_authors = self.authors;
        gui.show_series = self.series;
        gui.show_tags = self.tags;
        gui.show_formats = self.formats;
        gui.show_rating = self.rating;
        gui.show_publisher = self.publisher;
        gui.show_languages = self.languages;
    }
}

impl ColumnWidths {
    fn from_config(gui: &GuiConfig) -> Self {
        Self {
            title: gui.width_title,
            authors: gui.width_authors,
            series: gui.width_series,
            tags: gui.width_tags,
            formats: gui.width_formats,
            rating: gui.width_rating,
            publisher: gui.width_publisher,
            languages: gui.width_languages,
        }
    }

    fn apply_to_config(&self, gui: &mut GuiConfig) {
        gui.width_title = self.title;
        gui.width_authors = self.authors;
        gui.width_series = self.series;
        gui.width_tags = self.tags;
        gui.width_formats = self.formats;
        gui.width_rating = self.rating;
        gui.width_publisher = self.publisher;
        gui.width_languages = self.languages;
    }
}

pub struct LibraryView {
    db: Database,
    cache: MetadataCache,
    books: Vec<BookRow>,
    all_books: Vec<BookRow>,
    available_formats: Vec<String>,
    available_tags: Vec<String>,
    available_languages: Vec<String>,
    available_publishers: Vec<String>,
    selected_ids: Vec<i64>,
    last_selected: Option<i64>,
    details: Option<BookDetails>,
    edit_mode: bool,
    show_edit_dialog: bool,
    edit: EditState,
    format_filter: Option<String>,
    sort_mode: SortMode,
    sort_dir: SortDirection,
    search_query: String,
    status: String,
    last_error: Option<String>,
    needs_refresh: bool,
    search_focus: bool,
    view_mode: ViewMode,
    columns: ColumnVisibility,
    column_widths: ColumnWidths,
    layout_dirty: bool,
    pending_save: bool,
    open_logs_requested: bool,
    log_dir: PathBuf,
}

impl LibraryView {
    pub fn new(config: &ControlPlane) -> CoreResult<Self> {
        let db = Database::open_with_fts(&config.db, &config.fts)?;
        let mut cache = MetadataCache::new();
        cache.refresh_books(&db)?;
        let mut view = Self {
            db,
            cache,
            books: Vec::new(),
            all_books: Vec::new(),
            available_formats: Vec::new(),
            available_tags: Vec::new(),
            available_languages: Vec::new(),
            available_publishers: Vec::new(),
            selected_ids: Vec::new(),
            last_selected: None,
            details: None,
            edit_mode: false,
            show_edit_dialog: false,
            edit: EditState::default(),
            format_filter: None,
            sort_mode: SortMode::Title,
            sort_dir: SortDirection::Asc,
            search_query: String::new(),
            status: "Ready".to_string(),
            last_error: None,
            needs_refresh: true,
            search_focus: false,
            view_mode: parse_view_mode(&config.gui.list_view_mode),
            columns: ColumnVisibility::from_config(&config.gui),
            column_widths: ColumnWidths::from_config(&config.gui),
            layout_dirty: false,
            pending_save: false,
            open_logs_requested: false,
            log_dir: config.paths.log_dir.clone(),
        };
        view.refresh_books()?;
        Ok(view)
    }

    pub fn status_line(&self) -> (&str, Option<&str>) {
        (self.status.as_str(), self.last_error.as_deref())
    }

    pub fn request_search_focus(&mut self) {
        self.search_focus = true;
    }

    pub fn request_refresh(&mut self) {
        self.needs_refresh = true;
    }

    pub fn request_save(&mut self) {
        self.pending_save = true;
    }

    pub fn request_open_logs(&mut self) {
        self.open_logs_requested = true;
    }

    pub fn notify_unimplemented(&mut self, message: &str) {
        self.status = message.to_string();
    }

    pub fn begin_edit(&mut self) {
        if self.details.is_some() {
            self.show_edit_dialog = true;
            self.edit_mode = true;
            self.edit = EditState::from_details(self.details.as_ref().expect("details"));
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        config: &mut ControlPlane,
        config_path: &Path,
    ) {
        let available = ui.available_rect_before_wrap();
        let left_width = (available.width() * 0.45).max(320.0);

        egui::Panel::left("library_list")
            .resizable(true)
            .default_size(left_width)
            .show_inside(ui, |ui| {
                ui.heading("Library");
                ui.separator();
                self.toolbar_controls(ui, config, config_path);
                ui.separator();
                self.search_controls(ui);
                ui.separator();
                self.sort_controls(ui);
                self.format_controls(ui);
                ui.separator();
                self.layout_controls(ui, config, config_path);
                ui.separator();
                if self.needs_refresh {
                    if let Err(err) = self.refresh_books() {
                        self.set_error(err);
                        self.needs_refresh = false;
                    } else {
                        self.clear_error();
                    }
                }
                match self.view_mode {
                    ViewMode::Table => self.table_view(ui),
                    ViewMode::Grid => self.grid_view(ui),
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(format!("{} books", self.books.len()));
                    if self.books.len() != self.all_books.len() {
                        ui.label(format!("(filtered from {})", self.all_books.len()));
                    }
                });
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.details_view(ui);
        });

        if self.show_edit_dialog {
            self.edit_dialog(ui);
        }

        if self.pending_save {
            self.pending_save = false;
            if let Err(err) = self.save_edit() {
                self.set_error(err);
            } else {
                self.clear_error();
            }
        }

        if self.open_logs_requested {
            self.open_logs_requested = false;
            if let Err(err) = open_path(&self.log_dir) {
                self.set_error(err);
            } else {
                self.status = "Opened logs directory".to_string();
            }
        }
    }

    fn toolbar_controls(&mut self, ui: &mut egui::Ui, config: &mut ControlPlane, config_path: &Path) {
        ui.horizontal(|ui| {
            if ui.button("Refresh").clicked() {
                self.needs_refresh = true;
            }
            if ui.button("Edit Metadata").clicked() {
                self.begin_edit();
            }
            if ui.button("Open Logs").clicked() {
                self.request_open_logs();
            }
            if ui.button("Save Layout").clicked() {
                self.persist_layout(config, config_path);
            }
        });
    }

    fn search_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Search");
            let search_id = egui::Id::new("library_search");
            let response = ui.add(egui::TextEdit::singleline(&mut self.search_query).id(search_id));
            if self.search_focus {
                ui.memory_mut(|mem| mem.request_focus(search_id));
                self.search_focus = false;
            }
            if response.changed() {
                self.needs_refresh = true;
            }
            if ui.button("Go").clicked() {
                self.needs_refresh = true;
            }
        });
    }

    fn sort_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Sort");
            egui::ComboBox::from_id_salt("sort_mode")
                .selected_text(self.sort_mode.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.sort_mode, SortMode::Title, "Title");
                    ui.selectable_value(&mut self.sort_mode, SortMode::Authors, "Authors");
                    ui.selectable_value(&mut self.sort_mode, SortMode::Series, "Series");
                    ui.selectable_value(&mut self.sort_mode, SortMode::Tags, "Tags");
                    ui.selectable_value(&mut self.sort_mode, SortMode::Formats, "Formats");
                    ui.selectable_value(&mut self.sort_mode, SortMode::Rating, "Rating");
                    ui.selectable_value(&mut self.sort_mode, SortMode::Publisher, "Publisher");
                    ui.selectable_value(&mut self.sort_mode, SortMode::Languages, "Languages");
                    ui.selectable_value(&mut self.sort_mode, SortMode::Id, "ID");
                });
            egui::ComboBox::from_id_salt("sort_dir")
                .selected_text(match self.sort_dir {
                    SortDirection::Asc => "Asc",
                    SortDirection::Desc => "Desc",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.sort_dir, SortDirection::Asc, "Asc");
                    ui.selectable_value(&mut self.sort_dir, SortDirection::Desc, "Desc");
                });
            if ui.button("Apply").clicked() {
                self.apply_filters();
            }
        });
    }

    fn format_controls(&mut self, ui: &mut egui::Ui) {
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
    }

    fn layout_controls(&mut self, ui: &mut egui::Ui, config: &mut ControlPlane, config_path: &Path) {
        ui.horizontal(|ui| {
            ui.label("View");
            egui::ComboBox::from_id_salt("view_mode")
                .selected_text(match self.view_mode {
                    ViewMode::Table => "Table",
                    ViewMode::Grid => "Grid",
                })
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(&mut self.view_mode, ViewMode::Table, "Table")
                        .clicked()
                    {
                        self.layout_dirty = true;
                    }
                    if ui
                        .selectable_value(&mut self.view_mode, ViewMode::Grid, "Grid")
                        .clicked()
                    {
                        self.layout_dirty = true;
                    }
                });
            if self.layout_dirty {
                ui.label("Layout changed");
            }
        });

        egui::CollapsingHeader::new("Columns")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("Visible columns");
                ui.checkbox(&mut self.columns.title, "Title");
                ui.checkbox(&mut self.columns.authors, "Authors");
                ui.checkbox(&mut self.columns.series, "Series");
                ui.checkbox(&mut self.columns.tags, "Tags");
                ui.checkbox(&mut self.columns.formats, "Formats");
                ui.checkbox(&mut self.columns.rating, "Rating");
                ui.checkbox(&mut self.columns.publisher, "Publisher");
                ui.checkbox(&mut self.columns.languages, "Languages");
                ui.separator();
                ui.label("Column widths");
                column_width_control(
                    ui,
                    "Title",
                    &mut self.column_widths.title,
                    &mut self.layout_dirty,
                );
                column_width_control(
                    ui,
                    "Authors",
                    &mut self.column_widths.authors,
                    &mut self.layout_dirty,
                );
                column_width_control(
                    ui,
                    "Series",
                    &mut self.column_widths.series,
                    &mut self.layout_dirty,
                );
                column_width_control(
                    ui,
                    "Tags",
                    &mut self.column_widths.tags,
                    &mut self.layout_dirty,
                );
                column_width_control(
                    ui,
                    "Formats",
                    &mut self.column_widths.formats,
                    &mut self.layout_dirty,
                );
                column_width_control(
                    ui,
                    "Rating",
                    &mut self.column_widths.rating,
                    &mut self.layout_dirty,
                );
                column_width_control(
                    ui,
                    "Publisher",
                    &mut self.column_widths.publisher,
                    &mut self.layout_dirty,
                );
                column_width_control(
                    ui,
                    "Languages",
                    &mut self.column_widths.languages,
                    &mut self.layout_dirty,
                );
                if ui.button("Save Layout").clicked() {
                    self.persist_layout(config, config_path);
                }
            });
    }


    fn table_view(&mut self, ui: &mut egui::Ui) {
        let row_height = ui.text_style_height(&egui::TextStyle::Body).max(18.0) + 8.0;
        let row_height = row_height.max(32.0);
        let mut table = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0);

        if self.columns.title {
            table = table.column(column_with_width(self.column_widths.title));
        }
        if self.columns.authors {
            table = table.column(column_with_width(self.column_widths.authors));
        }
        if self.columns.series {
            table = table.column(column_with_width(self.column_widths.series));
        }
        if self.columns.tags {
            table = table.column(column_with_width(self.column_widths.tags));
        }
        if self.columns.formats {
            table = table.column(column_with_width(self.column_widths.formats));
        }
        if self.columns.rating {
            table = table.column(column_with_width(self.column_widths.rating));
        }
        if self.columns.publisher {
            table = table.column(column_with_width(self.column_widths.publisher));
        }
        if self.columns.languages {
            table = table.column(column_with_width(self.column_widths.languages));
        }

        table
            .header(row_height, |mut header| {
                if self.columns.title {
                    header.col(|ui| self.sort_header(ui, "Title", SortMode::Title));
                }
                if self.columns.authors {
                    header.col(|ui| self.sort_header(ui, "Authors", SortMode::Authors));
                }
                if self.columns.series {
                    header.col(|ui| self.sort_header(ui, "Series", SortMode::Series));
                }
                if self.columns.tags {
                    header.col(|ui| self.sort_header(ui, "Tags", SortMode::Tags));
                }
                if self.columns.formats {
                    header.col(|ui| self.sort_header(ui, "Formats", SortMode::Formats));
                }
                if self.columns.rating {
                    header.col(|ui| self.sort_header(ui, "Rating", SortMode::Rating));
                }
                if self.columns.publisher {
                    header.col(|ui| self.sort_header(ui, "Publisher", SortMode::Publisher));
                }
                if self.columns.languages {
                    header.col(|ui| self.sort_header(ui, "Languages", SortMode::Languages));
                }
            })
            .body(|body| {
                body.rows(row_height, self.books.len(), |mut row| {
                    let row_index = row.index();
                    let book = &self.books[row_index];
                    let selected = self.selected_ids.contains(&book.id);
                    let mut row_clicked = false;
                    let mut modifiers = egui::Modifiers::default();
                    if self.columns.title {
                        row.col(|ui: &mut egui::Ui| {
                            let response = ui.selectable_label(selected, highlight_text(&book.title, &self.search_query));
                            if response.clicked() {
                                row_clicked = true;
                                modifiers = response.ctx.input(|i| i.modifiers);
                            }
                        });
                    }
                    if self.columns.authors {
                        row.col(|ui: &mut egui::Ui| {
                            let response = ui.selectable_label(selected, highlight_text(&book.authors, &self.search_query));
                            if response.clicked() {
                                row_clicked = true;
                                modifiers = response.ctx.input(|i| i.modifiers);
                            }
                        });
                    }
                    if self.columns.series {
                        row.col(|ui: &mut egui::Ui| {
                            let response = ui.selectable_label(selected, highlight_text(&book.series, &self.search_query));
                            if response.clicked() {
                                row_clicked = true;
                                modifiers = response.ctx.input(|i| i.modifiers);
                            }
                        });
                    }
                    if self.columns.tags {
                        row.col(|ui: &mut egui::Ui| {
                            let response = ui.selectable_label(selected, highlight_text(&book.tags, &self.search_query));
                            if response.clicked() {
                                row_clicked = true;
                                modifiers = response.ctx.input(|i| i.modifiers);
                            }
                        });
                    }
                    if self.columns.formats {
                        row.col(|ui: &mut egui::Ui| {
                            let response = ui.selectable_label(selected, &book.format);
                            if response.clicked() {
                                row_clicked = true;
                                modifiers = response.ctx.input(|i| i.modifiers);
                            }
                        });
                    }
                    if self.columns.rating {
                        row.col(|ui: &mut egui::Ui| {
                            let response = ui.selectable_label(selected, &book.rating);
                            if response.clicked() {
                                row_clicked = true;
                                modifiers = response.ctx.input(|i| i.modifiers);
                            }
                        });
                    }
                    if self.columns.publisher {
                        row.col(|ui: &mut egui::Ui| {
                            let response = ui.selectable_label(selected, &book.publisher);
                            if response.clicked() {
                                row_clicked = true;
                                modifiers = response.ctx.input(|i| i.modifiers);
                            }
                        });
                    }
                    if self.columns.languages {
                        row.col(|ui: &mut egui::Ui| {
                            let response = ui.selectable_label(selected, &book.languages);
                            if response.clicked() {
                                row_clicked = true;
                                modifiers = response.ctx.input(|i| i.modifiers);
                            }
                        });
                    }
                    if row_clicked {
                        self.handle_selection(row_index, modifiers);
                    }
                });
            });
    }

    fn grid_view(&mut self, ui: &mut egui::Ui) {
        let books = self.books.clone();
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut row = 0;
            let mut col = 0;
            let columns = 3;
            for book in &books {
                if col == 0 {
                    ui.horizontal(|ui| {
                        self.grid_cell(ui, book);
                        col += 1;
                    });
                } else {
                    self.grid_cell(ui, book);
                    col += 1;
                }
                if col >= columns {
                    row += 1;
                    col = 0;
                    ui.separator();
                }
            }
            if row == 0 && books.is_empty() {
                ui.label("No books to display.");
            }
        });
    }

    fn grid_cell(&mut self, ui: &mut egui::Ui, book: &BookRow) {
        let selected = self.selected_ids.contains(&book.id);
        let frame = egui::Frame::group(ui.style()).fill(if selected {
            egui::Color32::from_gray(60)
        } else {
            egui::Color32::from_gray(30)
        });
        frame.show(ui, |ui| {
            ui.set_min_width(140.0);
            ui.label(egui::RichText::new("Cover").color(egui::Color32::from_gray(160)));
            ui.label(&book.title);
            if !book.authors.is_empty() {
                ui.label(&book.authors);
            }
            if ui.button("Select").clicked() {
                self.select_book(book.id);
            }
        });
    }

    fn details_view(&mut self, ui: &mut egui::Ui) {
        ui.heading("Details");
        ui.separator();
        let mut action = DetailAction::None;
        let mut open_paths: Vec<PathBuf> = Vec::new();
        let details_snapshot = self.details.clone();

        match &details_snapshot {
            Some(details) => {
                ui.horizontal(|ui| {
                    if ui.add_enabled(!self.edit_mode, egui::Button::new("Edit")).clicked() {
                        action = DetailAction::BeginEdit;
                    }
                    if ui
                        .add_enabled(self.edit_mode, egui::Button::new("Save"))
                        .clicked()
                    {
                        action = DetailAction::Save;
                    }
                    if ui
                        .add_enabled(self.edit_mode, egui::Button::new("Cancel"))
                        .clicked()
                    {
                        action = DetailAction::Cancel;
                    }
                });
                ui.separator();
                ui.label(format!("Title: {}", details.book.title));
                ui.label(format!("Format: {}", details.book.format));
                ui.label(format!("Path: {}", details.book.path));
                ui.label(format!("Authors: {}", details.authors.join(", ")));
                ui.label(format!("Tags: {}", details.tags.join(", ")));
                ui.label(format!(
                    "Series: {}",
                    details
                        .series
                        .as_ref()
                        .map(|series| format!("{} ({})", series.name, series.index))
                        .unwrap_or_else(|| "none".to_string())
                ));
                ui.label(format!(
                    "Publisher: {}",
                    details.extras.publisher.clone().unwrap_or_else(|| "none".to_string())
                ));
                ui.label(format!(
                    "Rating: {}",
                    details
                        .extras
                        .rating
                        .map(|rating| rating.to_string())
                        .unwrap_or_else(|| "none".to_string())
                ));
                ui.label(format!(
                    "Languages: {}",
                    if details.extras.languages.is_empty() {
                        "none".to_string()
                    } else {
                        details.extras.languages.join(", ")
                    }
                ));
                ui.label(format!(
                    "UUID: {}",
                    details
                        .extras
                        .uuid
                        .clone()
                        .unwrap_or_else(|| "none".to_string())
                ));

                ui.separator();
                ui.heading("Formats");
                if details.assets.is_empty() {
                    ui.label("No assets recorded.");
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for asset in &details.assets {
                            ui.horizontal(|ui| {
                                ui.label(format!(
                                    "{} | {} bytes",
                                    asset.stored_path, asset.size_bytes
                                ));
                                if ui.button("Open").clicked() {
                                    open_paths.push(PathBuf::from(&asset.stored_path));
                                }
                                if ui.button("Convert").clicked() {
                                    action = DetailAction::Convert;
                                }
                                if ui.button("Remove").clicked() {
                                    action = DetailAction::RemoveAsset;
                                }
                            });
                        }
                    });
                }

                ui.separator();
                ui.heading("Actions");
                ui.horizontal(|ui| {
                    if ui.button("Open file").clicked() {
                        open_paths.push(PathBuf::from(&details.book.path));
                    }
                    if ui.button("Open folder").clicked() {
                        if let Some(parent) = Path::new(&details.book.path).parent() {
                            open_paths.push(parent.to_path_buf());
                        }
                    }
                    if ui.button("Open with external viewer").clicked() {
                        open_paths.push(PathBuf::from(&details.book.path));
                    }
                });
            }
            None => {
                ui.label("Select a book to view details.");
            }
        }

        for path in open_paths {
            if let Err(err) = open_path(&path) {
                self.set_error(err);
            }
        }

        match action {
            DetailAction::BeginEdit => self.begin_edit(),
            DetailAction::Save => self.pending_save = true,
            DetailAction::Cancel => self.cancel_edit(),
            DetailAction::Convert => self.notify_unimplemented("Convert asset not wired yet."),
            DetailAction::RemoveAsset => self.notify_unimplemented("Remove asset not wired yet."),
            DetailAction::None => {}
        }
    }

    fn edit_dialog(&mut self, ui: &mut egui::Ui) {
        let mut open = self.show_edit_dialog;
        egui::Window::new("Edit Metadata")
            .open(&mut open)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                ui.label("Title");
                ui.text_edit_singleline(&mut self.edit.title);
                ui.label("Authors (comma separated)");
                ui.text_edit_singleline(&mut self.edit.authors);
                ui.label("Tags (comma separated)");
                ui.text_edit_singleline(&mut self.edit.tags);
                self.tag_autocomplete(ui);
                ui.label("Series");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.edit.series_name);
                    if ui.small_button("-").clicked() {
                        self.edit.series_index = (self.edit.series_index - 0.1).max(0.0);
                    }
                    ui.add(
                        egui::DragValue::new(&mut self.edit.series_index)
                            .speed(0.1)
                            .range(0.0..=999.0),
                    );
                    if ui.small_button("+").clicked() {
                        self.edit.series_index += 0.1;
                    }
                });
                ui.label("Identifiers (one per line, format: type:value)");
                ui.text_edit_multiline(&mut self.edit.identifiers);
                ui.label("ISBN");
                ui.text_edit_singleline(&mut self.edit.isbn);
                ui.label("Publisher");
                ui.text_edit_singleline(&mut self.edit.publisher);
                ui.label("Languages (comma separated)");
                ui.text_edit_singleline(&mut self.edit.languages);
                self.language_autocomplete(ui);
                ui.label("Rating");
                rating_stars(ui, &mut self.edit.rating);
                ui.label("Comment");
                ui.text_edit_multiline(&mut self.edit.comment);
                ui.label(format!("UUID: {}", self.edit.uuid));
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        self.pending_save = true;
                        self.show_edit_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.cancel_edit();
                        self.show_edit_dialog = false;
                    }
                });
            });
        self.show_edit_dialog = open;
        self.edit_mode = open;
    }

    fn tag_autocomplete(&mut self, ui: &mut egui::Ui) {
        let query = self.edit.tags.split(',').last().unwrap_or("").trim().to_lowercase();
        if query.is_empty() || self.available_tags.is_empty() {
            return;
        }
        ui.label("Tag suggestions");
        for tag in self
            .available_tags
            .iter()
            .filter(|tag| tag.to_lowercase().contains(&query))
            .take(5)
        {
            if ui.button(tag).clicked() {
                apply_autocomplete(&mut self.edit.tags, tag);
            }
        }
    }

    fn language_autocomplete(&mut self, ui: &mut egui::Ui) {
        let query = self
            .edit
            .languages
            .split(',')
            .last()
            .unwrap_or("")
            .trim()
            .to_lowercase();
        if query.is_empty() || self.available_languages.is_empty() {
            return;
        }
        ui.label("Language suggestions");
        for lang in self
            .available_languages
            .iter()
            .filter(|lang| lang.to_lowercase().contains(&query))
            .take(5)
        {
            if ui.button(lang).clicked() {
                apply_autocomplete(&mut self.edit.languages, lang);
            }
        }
    }

    fn refresh_books(&mut self) -> CoreResult<()> {
        self.cache.refresh_books(&self.db)?;
        let query = self.search_query.trim().to_string();
        let list = if query.is_empty() {
            self.db.list_books()?
        } else {
            self.db.search_books(&query)?
        };
        let mut rows = Vec::new();
        for book in list {
            let row = self.build_row(&book)?;
            rows.push(row);
        }
        self.all_books = rows;
        self.available_formats = self
            .all_books
            .iter()
            .map(|book| book.format.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        self.available_tags = self.db.list_tags().unwrap_or_default();
        self.available_languages = self.db.list_languages().unwrap_or_default();
        self.available_publishers = self.db.list_publishers().unwrap_or_default();
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

    fn build_row(&mut self, book: &BookRecord) -> CoreResult<BookRow> {
        let details = self
            .cache
            .get_book_details(&self.db, book.id)?
            .cloned();
        let (authors, tags, series, rating, publisher, languages) = if let Some(details) = details
        {
            (
                details.authors.join(", "),
                details.tags.join(", "),
                details
                    .series
                    .map(|series| format!("{} ({})", series.name, series.index))
                    .unwrap_or_default(),
                details
                    .extras
                    .rating
                    .map(|rating| rating.to_string())
                    .unwrap_or_default(),
                details.extras.publisher.unwrap_or_default(),
                details.extras.languages.join(", "),
            )
        } else {
            (String::new(), String::new(), String::new(), String::new(), String::new(), String::new())
        };
        Ok(BookRow {
            id: book.id,
            title: book.title.clone(),
            format: book.format.clone(),
            path: book.path.clone(),
            authors,
            tags,
            series,
            rating,
            publisher,
            languages,
        })
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
        let extras = self.db.get_book_extras(id)?;
        self.details = Some(BookDetails {
            book,
            assets,
            authors,
            tags,
            series,
            identifiers,
            comment,
            extras,
        });
        self.edit = EditState::from_details(self.details.as_ref().expect("details"));
        self.edit_mode = false;
        info!(component = "gui", book_id = id, "loaded book details");
        Ok(())
    }

    fn select_book(&mut self, book_id: i64) {
        self.selected_ids = vec![book_id];
        self.last_selected = Some(book_id);
        if let Err(err) = self.load_details(book_id) {
            self.set_error(err);
        } else {
            self.clear_error();
        }
    }

    fn handle_selection(&mut self, row_index: usize, modifiers: egui::Modifiers) {
        let book_id = self.books[row_index].id;
        if modifiers.shift {
            if let Some(last_id) = self.last_selected {
                let mut start = row_index;
                let mut end = row_index;
                for (idx, row) in self.books.iter().enumerate() {
                    if row.id == last_id {
                        start = start.min(idx);
                        end = end.max(idx);
                    }
                }
                self.selected_ids = self.books[start..=end].iter().map(|row| row.id).collect();
            } else {
                self.selected_ids = vec![book_id];
            }
        } else if modifiers.ctrl || modifiers.command {
            if let Some(pos) = self.selected_ids.iter().position(|id| *id == book_id) {
                self.selected_ids.remove(pos);
            } else {
                self.selected_ids.push(book_id);
            }
            self.last_selected = Some(book_id);
        } else {
            self.selected_ids = vec![book_id];
            self.last_selected = Some(book_id);
        }
        if let Err(err) = self.load_details(book_id) {
            self.set_error(err);
        } else {
            self.clear_error();
        }
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
        self.sort_rows(&mut list);
        self.books = list;
    }

    fn sort_rows(&mut self, list: &mut Vec<BookRow>) {
        match self.sort_mode {
            SortMode::Title => list.sort_by(|a, b| a.title.cmp(&b.title)),
            SortMode::Authors => list.sort_by(|a, b| a.authors.cmp(&b.authors)),
            SortMode::Series => list.sort_by(|a, b| a.series.cmp(&b.series)),
            SortMode::Tags => list.sort_by(|a, b| a.tags.cmp(&b.tags)),
            SortMode::Formats => list.sort_by(|a, b| a.format.cmp(&b.format)),
            SortMode::Rating => list.sort_by(|a, b| a.rating.cmp(&b.rating)),
            SortMode::Publisher => list.sort_by(|a, b| a.publisher.cmp(&b.publisher)),
            SortMode::Languages => list.sort_by(|a, b| a.languages.cmp(&b.languages)),
            SortMode::Id => list.sort_by(|a, b| a.id.cmp(&b.id)),
        }
        if self.sort_dir == SortDirection::Desc {
            list.reverse();
        }
    }

    fn sort_header(&mut self, ui: &mut egui::Ui, label: &str, mode: SortMode) {
        let mut text = label.to_string();
        if self.sort_mode == mode {
            text.push_str(match self.sort_dir {
                SortDirection::Asc => " ↑",
                SortDirection::Desc => " ↓",
            });
        }
        if ui.button(text).clicked() {
            if self.sort_mode == mode {
                self.sort_dir = match self.sort_dir {
                    SortDirection::Asc => SortDirection::Desc,
                    SortDirection::Desc => SortDirection::Asc,
                };
            } else {
                self.sort_mode = mode;
                self.sort_dir = SortDirection::Asc;
            }
            self.apply_filters();
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
        self.validate_identifiers()?;
        self.db.update_book_title(book_id, title)?;
        let authors = parse_list(&self.edit.authors);
        let tags = parse_list(&self.edit.tags);
        let identifiers = parse_identifiers(&self.edit.identifiers, &self.edit.isbn);
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
        if self.edit.publisher.trim().is_empty() {
            self.db.clear_book_publisher(book_id)?;
        } else {
            self.db
                .set_book_publisher(book_id, self.edit.publisher.trim())?;
        }
        self.db.set_book_rating(book_id, self.edit.rating)?;
        let languages = parse_list(&self.edit.languages);
        self.db.set_book_languages(book_id, &languages)?;
        self.status = "Metadata saved".to_string();
        self.edit_mode = false;
        self.refresh_books()?;
        self.load_details(book_id)?;
        info!(component = "gui", book_id, "saved metadata edits");
        Ok(())
    }

    fn validate_identifiers(&self) -> CoreResult<()> {
        for line in self.edit.identifiers.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !trimmed.contains(':') {
                return Err(CoreError::ConfigValidate(
                    "identifiers must be in type:value format".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn persist_layout(&mut self, config: &mut ControlPlane, config_path: &Path) {
        self.columns.apply_to_config(&mut config.gui);
        self.column_widths.apply_to_config(&mut config.gui);
        config.gui.list_view_mode = match self.view_mode {
            ViewMode::Table => "table".to_string(),
            ViewMode::Grid => "grid".to_string(),
        };
        if let Err(err) = config.save_to_path(config_path) {
            self.set_error(err);
        } else {
            self.layout_dirty = false;
            self.status = "Layout saved".to_string();
        }
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
    isbn: String,
    comment: String,
    publisher: String,
    languages: String,
    rating: i64,
    uuid: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetailAction {
    None,
    BeginEdit,
    Save,
    Cancel,
    Convert,
    RemoveAsset,
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
            isbn: String::new(),
            comment: String::new(),
            publisher: String::new(),
            languages: String::new(),
            rating: 0,
            uuid: String::new(),
        }
    }
}

impl EditState {
    fn from_details(details: &BookDetails) -> Self {
        let isbn = details
            .identifiers
            .iter()
            .find(|id| id.id_type.eq_ignore_ascii_case("isbn"))
            .map(|id| id.value.clone())
            .unwrap_or_default();
        Self {
            title: details.book.title.clone(),
            authors: details.authors.join(", "),
            tags: details.tags.join(", "),
            series_name: details
                .series
                .as_ref()
                .map(|series| series.name.clone())
                .unwrap_or_default(),
            series_index: details.series.as_ref().map(|series| series.index).unwrap_or(1.0),
            identifiers: details
                .identifiers
                .iter()
                .map(|id| format!("{}:{}", id.id_type, id.value))
                .collect::<Vec<_>>()
                .join("\n"),
            isbn,
            comment: details.comment.clone().unwrap_or_default(),
            publisher: details.extras.publisher.clone().unwrap_or_default(),
            languages: details.extras.languages.join(", "),
            rating: details.extras.rating.unwrap_or(0),
            uuid: details.extras.uuid.clone().unwrap_or_default(),
        }
    }
}

fn parse_list(text: &str) -> Vec<String> {
    text.split(',')
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(|item| item.to_string())
        .collect()
}

fn parse_identifiers(text: &str, isbn: &str) -> Vec<(String, String)> {
    let mut identifiers: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((id_type, value)) = trimmed.split_once(':') {
            identifiers.push((id_type.trim().to_string(), value.trim().to_string()));
        }
    }
    if !isbn.trim().is_empty() {
        identifiers.retain(|id| !id.0.eq_ignore_ascii_case("isbn"));
        identifiers.push(("isbn".to_string(), isbn.trim().to_string()));
    }
    identifiers
}

fn highlight_text(text: &str, query: &str) -> egui::RichText {
    let query = query.trim();
    if query.is_empty() {
        return egui::RichText::new(text);
    }
    let lowercase = text.to_lowercase();
    let query_lower = query.to_lowercase();
    if lowercase.contains(&query_lower) {
        egui::RichText::new(text).color(egui::Color32::YELLOW)
    } else {
        egui::RichText::new(text)
    }
}

fn apply_autocomplete(field: &mut String, value: &str) {
    let mut parts: Vec<&str> = field.split(',').collect();
    if let Some(last) = parts.last_mut() {
        *last = value;
    }
    let rebuilt = parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    *field = rebuilt;
}

fn rating_stars(ui: &mut egui::Ui, rating: &mut i64) {
    ui.horizontal(|ui| {
        for star in 1..=5 {
            let star_value = star * 2;
            let filled = *rating >= star_value;
            let label = if filled { "★" } else { "☆" };
            if ui.button(label).clicked() {
                *rating = star_value as i64;
            }
        }
        if ui.button("Clear").clicked() {
            *rating = 0;
        }
    });
}

fn parse_view_mode(value: &str) -> ViewMode {
    match value {
        "grid" => ViewMode::Grid,
        _ => ViewMode::Table,
    }
}

fn column_width_control(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    layout_dirty: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).range(60.0..=720.0).speed(1.0));
    });
    *layout_dirty = true;
}

fn column_with_width(width: f32) -> Column {
    Column::initial(width).resizable(true)
}

fn open_path(path: &Path) -> CoreResult<()> {
    if !path.exists() {
        return Err(CoreError::ConfigValidate(format!(
            "path does not exist: {}",
            path.display()
        )));
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|err| CoreError::Io("open path".to_string(), err))?;
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        tracing::warn!(component = "gui", path = %path.display(), "open path not supported");
        Err(CoreError::ConfigValidate(
            "open path not supported on this platform".to_string(),
        ))
    }
}
