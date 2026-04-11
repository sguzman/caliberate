//! GUI views and models.

use caliberate_assets::compression::decompress_file;
use caliberate_assets::storage::{AssetStore, LocalAssetStore, StorageMode};
use caliberate_conversion::pipeline::convert_file;
use caliberate_conversion::settings::ConversionSettings;
use caliberate_core::config::{ControlPlane, GuiConfig, IngestMode};
use caliberate_core::error::{CoreError, CoreResult};
use caliberate_db::cache::MetadataCache;
use caliberate_db::database::{
    AssetRow, BookRecord, CategoryCount, CustomColumn, Database, IdentifierEntry, NoteRecord,
    SeriesEntry,
};
use caliberate_device::detection::{DeviceInfo, detect_devices};
use caliberate_device::sync::send_to_device;
use caliberate_library::ingest::{IngestOutcome, IngestRequest, Ingestor};
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use image::{DynamicImage, ImageFormat};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use time::OffsetDateTime;
use tracing::{info, warn};
use walkdir::WalkDir;

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
    pub has_cover: bool,
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
    pub notes: Vec<NoteRecord>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserCategory {
    Authors,
    Tags,
    Series,
    Publishers,
    Ratings,
    Languages,
}

impl BrowserCategory {
    fn label(self) -> &'static str {
        match self {
            BrowserCategory::Authors => "Authors",
            BrowserCategory::Tags => "Tags",
            BrowserCategory::Series => "Series",
            BrowserCategory::Publishers => "Publishers",
            BrowserCategory::Ratings => "Ratings",
            BrowserCategory::Languages => "Languages",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserSort {
    Name,
    Count,
}

impl BrowserSort {
    fn label(self) -> &'static str {
        match self {
            BrowserSort::Name => "Name",
            BrowserSort::Count => "Count",
        }
    }
}

#[derive(Debug, Clone)]
struct BrowserFilter {
    category: BrowserCategory,
    value: String,
    mode: BrowserFilterMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserFilterMode {
    Include,
    Exclude,
}

impl BrowserFilter {
    fn label(&self) -> String {
        let prefix = match self.mode {
            BrowserFilterMode::Include => "+",
            BrowserFilterMode::Exclude => "-",
        };
        format!("{prefix} {}: {}", self.category.label(), self.value)
    }
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
    cover: bool,
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
    cover: f32,
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
            cover: gui.show_cover,
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
        gui.show_cover = self.cover;
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
            cover: gui.width_cover,
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
        gui.width_cover = self.cover;
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
    browser_query: String,
    browser_filters: Vec<BrowserFilter>,
    browser_authors: Vec<CategoryCount>,
    browser_tags: Vec<CategoryCount>,
    browser_series: Vec<CategoryCount>,
    browser_publishers: Vec<CategoryCount>,
    browser_ratings: Vec<CategoryCount>,
    browser_languages: Vec<CategoryCount>,
    browser_saved_searches: Vec<(String, String)>,
    browser_sort: BrowserSort,
    browser_sort_desc: bool,
    browser_open_authors: bool,
    browser_open_tags: bool,
    browser_open_series: bool,
    browser_open_publishers: bool,
    browser_open_ratings: bool,
    browser_open_languages: bool,
    browser_open_dirty: bool,
    active_virtual_library: Option<String>,
    virtual_library_filters: HashMap<String, Vec<BrowserFilter>>,
    selected_ids: Vec<i64>,
    last_selected: Option<i64>,
    details: Option<BookDetails>,
    edit_mode: bool,
    show_edit_dialog: bool,
    edit: EditState,
    format_filter: Option<String>,
    sort_mode: SortMode,
    sort_dir: SortDirection,
    secondary_sort: Option<SortMode>,
    search_query: String,
    search_history: Vec<String>,
    search_history_max: usize,
    search_commit_requested: bool,
    status: String,
    last_error: Option<String>,
    needs_refresh: bool,
    search_focus: bool,
    view_mode: ViewMode,
    view_density: ViewDensity,
    quick_details_panel: bool,
    columns: ColumnVisibility,
    column_widths: ColumnWidths,
    layout_dirty: bool,
    config_dirty: bool,
    pending_save: bool,
    open_logs_requested: bool,
    log_dir: PathBuf,
    tmp_dir: PathBuf,
    cover_thumb_size: f32,
    cover_preview_size: f32,
    show_format_badges: bool,
    show_language_badges: bool,
    cover_dir: PathBuf,
    cover_cache_dir: PathBuf,
    cover_max_bytes: u64,
    last_cover_thumb_size: f32,
    last_cover_preview_size: f32,
    table_row_height: f32,
    toast_duration_secs: f64,
    toast_max: usize,
    toasts: Vec<Toast>,
    jobs: Vec<JobEntry>,
    next_job_id: u64,
    last_tick: f64,
    comment_preview: bool,
    comment_render_markdown: bool,
    comment_render_overrides: HashMap<i64, bool>,
    cover_cache: HashMap<i64, egui::TextureHandle>,
    cover_preview_cache: HashMap<i64, egui::TextureHandle>,
    cover_state: CoverDialogState,
    reader: ReaderState,
    reader_progress: HashMap<i64, usize>,
    note_input: String,
    note_delete_id: Option<i64>,
    note_delete_open: bool,
    remove_asset_dialog: RemoveAssetDialogState,
    pending_convert_book: Option<i64>,
    add_books: AddBooksDialogState,
    remove_books: RemoveBooksDialogState,
    bulk_edit: BulkEditDialogState,
    convert_books: ConvertBooksDialogState,
    save_to_disk: SaveToDiskDialogState,
    device_sync: DeviceSyncDialogState,
    manage_tags: ManageTagsDialogState,
    manage_series: ManageSeriesDialogState,
    manage_custom_columns: ManageCustomColumnsDialogState,
    manage_virtual_libraries: ManageVirtualLibrariesDialogState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewDensity {
    Compact,
    Comfortable,
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
            browser_query: String::new(),
            browser_filters: Vec::new(),
            browser_authors: Vec::new(),
            browser_tags: Vec::new(),
            browser_series: Vec::new(),
            browser_publishers: Vec::new(),
            browser_ratings: Vec::new(),
            browser_languages: Vec::new(),
            browser_saved_searches: Vec::new(),
            browser_sort: BrowserSort::Name,
            browser_sort_desc: false,
            browser_open_authors: false,
            browser_open_tags: false,
            browser_open_series: false,
            browser_open_publishers: false,
            browser_open_ratings: false,
            browser_open_languages: false,
            browser_open_dirty: false,
            active_virtual_library: config.gui.active_virtual_library.clone(),
            virtual_library_filters: decode_virtual_library_filters(
                &config.gui.virtual_library_filters,
            ),
            selected_ids: Vec::new(),
            last_selected: None,
            details: None,
            edit_mode: false,
            show_edit_dialog: false,
            edit: EditState::default(),
            format_filter: None,
            sort_mode: SortMode::Title,
            sort_dir: SortDirection::Asc,
            secondary_sort: None,
            search_query: String::new(),
            search_history: Vec::new(),
            search_history_max: config.gui.search_history_max,
            search_commit_requested: false,
            status: "Ready".to_string(),
            last_error: None,
            needs_refresh: true,
            search_focus: false,
            view_mode: parse_view_mode(&config.gui.list_view_mode),
            view_density: parse_view_density(&config.gui.view_density),
            quick_details_panel: config.gui.quick_details_panel,
            columns: ColumnVisibility::from_config(&config.gui),
            column_widths: ColumnWidths::from_config(&config.gui),
            layout_dirty: false,
            config_dirty: false,
            pending_save: false,
            open_logs_requested: false,
            log_dir: config.paths.log_dir.clone(),
            tmp_dir: config.paths.tmp_dir.clone(),
            cover_thumb_size: config.gui.cover_thumb_size,
            cover_preview_size: config.gui.cover_preview_size,
            show_format_badges: config.gui.show_format_badges,
            show_language_badges: config.gui.show_language_badges,
            cover_dir: config.gui.cover_dir.clone(),
            cover_cache_dir: config.gui.cover_cache_dir.clone(),
            cover_max_bytes: config.gui.cover_max_bytes,
            last_cover_thumb_size: config.gui.cover_thumb_size,
            last_cover_preview_size: config.gui.cover_preview_size,
            table_row_height: config.gui.table_row_height,
            toast_duration_secs: config.gui.toast_duration_secs,
            toast_max: config.gui.toast_max,
            toasts: Vec::new(),
            jobs: Vec::new(),
            next_job_id: 1,
            last_tick: 0.0,
            comment_preview: false,
            comment_render_markdown: true,
            comment_render_overrides: HashMap::new(),
            cover_cache: HashMap::new(),
            cover_preview_cache: HashMap::new(),
            cover_state: CoverDialogState::default(),
            reader: ReaderState::from_config(config),
            reader_progress: HashMap::new(),
            note_input: String::new(),
            note_delete_id: None,
            note_delete_open: false,
            remove_asset_dialog: RemoveAssetDialogState::default(),
            pending_convert_book: None,
            add_books: AddBooksDialogState::default(),
            remove_books: RemoveBooksDialogState::default(),
            bulk_edit: BulkEditDialogState::default(),
            convert_books: ConvertBooksDialogState::default(),
            save_to_disk: SaveToDiskDialogState::default(),
            device_sync: DeviceSyncDialogState::default(),
            manage_tags: ManageTagsDialogState::default(),
            manage_series: ManageSeriesDialogState::default(),
            manage_custom_columns: ManageCustomColumnsDialogState::default(),
            manage_virtual_libraries: ManageVirtualLibrariesDialogState::default(),
        };
        if let Some(active) = &view.active_virtual_library {
            view.browser_filters = view
                .virtual_library_filters
                .get(active)
                .cloned()
                .unwrap_or_default();
        }
        view.refresh_books()?;
        Ok(view)
    }

    pub fn status_line(&self) -> (&str, Option<&str>) {
        (self.status.as_str(), self.last_error.as_deref())
    }

    pub fn error_message(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn clear_error_message(&mut self) {
        self.last_error = None;
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

    pub fn open_add_books(&mut self, config: &ControlPlane) {
        self.add_books.apply_defaults(config);
        self.add_books.open = true;
    }

    pub fn open_remove_books(&mut self, config: &ControlPlane) {
        self.remove_books.apply_defaults(config);
        self.remove_books.open = true;
    }

    pub fn open_bulk_edit(&mut self) {
        self.bulk_edit.reset();
    }

    pub fn open_convert_books(&mut self, config: &ControlPlane) {
        self.convert_books.apply_defaults(config);
        self.convert_books.open = true;
    }

    pub fn open_save_to_disk(&mut self, config: &ControlPlane) {
        self.save_to_disk.apply_defaults(config);
        self.save_to_disk.open = true;
    }

    pub fn open_device_sync(&mut self, config: &ControlPlane) {
        self.device_sync.apply_defaults(config);
        self.device_sync.open = true;
    }

    pub fn open_manage_tags(&mut self) {
        self.manage_tags.open = true;
        self.manage_tags.needs_refresh = true;
    }

    pub fn open_manage_series(&mut self) {
        self.manage_series.open = true;
        self.manage_series.needs_refresh = true;
    }

    pub fn open_manage_custom_columns(&mut self) {
        self.manage_custom_columns.open = true;
        self.manage_custom_columns.needs_refresh = true;
    }

    pub fn open_manage_virtual_libraries(&mut self) {
        self.manage_virtual_libraries.open = true;
        self.manage_virtual_libraries.needs_refresh = true;
    }

    pub fn notify_unimplemented(&mut self, message: &str) {
        self.status = message.to_string();
        self.push_toast(message, ToastLevel::Warn);
    }

    pub fn enqueue_job_action(&mut self, name: &str) {
        let now = if self.last_tick == 0.0 {
            0.0
        } else {
            self.last_tick
        };
        self.enqueue_job(name, now);
    }

    pub fn begin_edit(&mut self) {
        if self.details.is_some() {
            self.show_edit_dialog = true;
            self.edit_mode = true;
            self.edit = EditState::from_details(self.details.as_ref().expect("details"));
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, config: &mut ControlPlane, config_path: &Path) {
        let now = ui.ctx().input(|i| i.time);
        self.sync_cover_config(config);
        self.tick_jobs(now);
        self.prune_toasts(now);
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
                self.filter_summary_controls(ui);
                ui.separator();
                self.layout_controls(ui, config, config_path);
                ui.separator();
                self.operations_controls(ui, config);
                ui.separator();
                self.management_controls(ui);
                ui.separator();
                self.browser_controls(ui);
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
                    ViewMode::Table => self.table_view(ui, config),
                    ViewMode::Grid => self.grid_view(ui, config),
                }
                if self.quick_details_panel {
                    ui.separator();
                    self.quick_details_preview(ui);
                }
                ui.separator();
                self.library_stats_panel(ui, &config.paths.cache_dir);
                ui.separator();
                self.status_bar(ui);
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.details_view(ui, config);
        });

        if let Some(book_id) = self.pending_convert_book.take() {
            self.selected_ids = vec![book_id];
            self.open_convert_books(config);
        }

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

        self.add_books_dialog(ui, config);
        self.remove_books_dialog(ui, config);
        self.bulk_edit_dialog(ui, config);
        self.convert_books_dialog(ui, config);
        self.save_to_disk_dialog(ui, config);
        self.device_sync_dialog(ui, config);
        self.manage_tags_dialog(ui);
        self.manage_series_dialog(ui);
        self.manage_custom_columns_dialog(ui);
        self.manage_virtual_libraries_dialog(ui);
        self.reader_dialog(ui);
        self.remove_asset_dialog(ui, config);
        self.note_delete_dialog(ui);

        if self.config_dirty {
            self.sync_gui_runtime_config(config);
            if let Err(err) = config.save_to_path(config_path) {
                self.set_error(err);
            } else {
                self.config_dirty = false;
            }
        }

        self.render_jobs(ui);
        self.render_toasts(ui);
    }

    fn toolbar_controls(
        &mut self,
        ui: &mut egui::Ui,
        config: &mut ControlPlane,
        config_path: &Path,
    ) {
        ui.horizontal(|ui| {
            if ui
                .button("Refresh")
                .on_hover_text("Reload library (F5)")
                .clicked()
            {
                self.needs_refresh = true;
            }
            if ui
                .button("Edit Metadata")
                .on_hover_text("Edit selected book metadata (E)")
                .clicked()
            {
                self.begin_edit();
            }
            if ui
                .button("Open Logs")
                .on_hover_text("Open logs folder")
                .clicked()
            {
                self.request_open_logs();
            }
            ui.separator();
            if ui
                .button("Save Layout")
                .on_hover_text("Persist column widths and view mode")
                .clicked()
            {
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
                self.search_commit_requested = true;
            }
            if ui.button("Clear").clicked() {
                self.search_query.clear();
                self.needs_refresh = true;
            }
            ui.menu_button("Recent", |ui| {
                if self.search_history.is_empty() {
                    ui.label("No recent searches.");
                } else {
                    for query in &self.search_history {
                        if ui.button(query).clicked() {
                            self.search_query = query.clone();
                            self.search_commit_requested = true;
                            self.needs_refresh = true;
                            ui.close_menu();
                        }
                    }
                }
                ui.separator();
                if ui.button("Clear history").clicked() {
                    self.search_history.clear();
                    ui.close_menu();
                }
            });
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
            egui::ComboBox::from_id_salt("secondary_sort_mode")
                .selected_text(
                    self.secondary_sort
                        .map(|mode| mode.label())
                        .unwrap_or("Secondary: none"),
                )
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(self.secondary_sort.is_none(), "Secondary: none")
                        .clicked()
                    {
                        self.secondary_sort = None;
                    }
                    ui.selectable_value(
                        &mut self.secondary_sort,
                        Some(SortMode::Title),
                        "Secondary: Title",
                    );
                    ui.selectable_value(
                        &mut self.secondary_sort,
                        Some(SortMode::Authors),
                        "Secondary: Authors",
                    );
                    ui.selectable_value(
                        &mut self.secondary_sort,
                        Some(SortMode::Series),
                        "Secondary: Series",
                    );
                    ui.selectable_value(
                        &mut self.secondary_sort,
                        Some(SortMode::Tags),
                        "Secondary: Tags",
                    );
                    ui.selectable_value(
                        &mut self.secondary_sort,
                        Some(SortMode::Formats),
                        "Secondary: Formats",
                    );
                    ui.selectable_value(
                        &mut self.secondary_sort,
                        Some(SortMode::Rating),
                        "Secondary: Rating",
                    );
                    ui.selectable_value(
                        &mut self.secondary_sort,
                        Some(SortMode::Publisher),
                        "Secondary: Publisher",
                    );
                    ui.selectable_value(
                        &mut self.secondary_sort,
                        Some(SortMode::Languages),
                        "Secondary: Languages",
                    );
                    ui.selectable_value(
                        &mut self.secondary_sort,
                        Some(SortMode::Id),
                        "Secondary: ID",
                    );
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

    fn filter_summary_controls(&mut self, ui: &mut egui::Ui) {
        let has_filters = !self.search_query.trim().is_empty()
            || self.format_filter.is_some()
            || !self.browser_filters.is_empty();
        if !has_filters {
            return;
        }
        ui.horizontal(|ui| {
            ui.label("Filters");
            if !self.search_query.trim().is_empty() {
                let label = format!("Search: {}", self.search_query.trim());
                if ui.button(label).clicked() {
                    self.search_query.clear();
                    self.needs_refresh = true;
                }
            }
            if let Some(format) = &self.format_filter {
                let label = format!("Format: {format}");
                if ui.button(label).clicked() {
                    self.format_filter = None;
                    self.apply_filters();
                }
            }
            for filter in self.browser_filters.clone() {
                if ui.button(filter.label()).clicked() {
                    self.remove_browser_filter(&filter);
                    self.apply_filters();
                }
            }
            if ui.button("Clear all").clicked() {
                self.clear_all_filters();
            }
        });
    }

    fn browser_controls(&mut self, ui: &mut egui::Ui) {
        ui.heading("Browser");
        ui.horizontal(|ui| {
            ui.label("Find");
            ui.text_edit_singleline(&mut self.browser_query);
            if ui.button("Clear").clicked() {
                self.browser_query.clear();
            }
            if ui.button("Clear filter").clicked() {
                self.browser_filters.clear();
                self.persist_active_virtual_filters();
                self.apply_filters();
            }
        });
        ui.horizontal(|ui| {
            ui.label("Virtual library");
            let selected_text = self
                .active_virtual_library
                .as_deref()
                .unwrap_or("All books")
                .to_string();
            let mut selected_library = self.active_virtual_library.clone();
            egui::ComboBox::from_id_salt("active_virtual_library")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(selected_library.is_none(), "All books")
                        .clicked()
                    {
                        selected_library = None;
                    }
                    for (name, _) in &self.browser_saved_searches {
                        if ui
                            .selectable_label(
                                selected_library.as_deref() == Some(name.as_str()),
                                name,
                            )
                            .clicked()
                        {
                            selected_library = Some(name.clone());
                        }
                    }
                });
            if selected_library != self.active_virtual_library {
                self.set_active_virtual_library(selected_library);
            }
        });
        ui.horizontal(|ui| {
            ui.label("Sort");
            egui::ComboBox::from_id_salt("browser_sort")
                .selected_text(self.browser_sort.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.browser_sort, BrowserSort::Name, "Name");
                    ui.selectable_value(&mut self.browser_sort, BrowserSort::Count, "Count");
                });
            if ui
                .button(if self.browser_sort_desc {
                    "Desc"
                } else {
                    "Asc"
                })
                .clicked()
            {
                self.browser_sort_desc = !self.browser_sort_desc;
            }
            if ui.button("Refresh counts").clicked() {
                if let Err(err) = self.refresh_browser() {
                    self.set_error(err);
                }
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Expand all").clicked() {
                self.browser_open_authors = true;
                self.browser_open_tags = true;
                self.browser_open_series = true;
                self.browser_open_publishers = true;
                self.browser_open_ratings = true;
                self.browser_open_languages = true;
                self.browser_open_dirty = true;
            }
            if ui.button("Collapse all").clicked() {
                self.browser_open_authors = false;
                self.browser_open_tags = false;
                self.browser_open_series = false;
                self.browser_open_publishers = false;
                self.browser_open_ratings = false;
                self.browser_open_languages = false;
                self.browser_open_dirty = true;
            }
        });
        let authors = self.browser_authors.clone();
        let tags = self.browser_tags.clone();
        let series = self.browser_series.clone();
        let publishers = self.browser_publishers.clone();
        let ratings = self.browser_ratings.clone();
        let languages = self.browser_languages.clone();
        self.browser_category_section(ui, BrowserCategory::Authors, &authors);
        self.browser_category_section(ui, BrowserCategory::Tags, &tags);
        self.browser_category_section(ui, BrowserCategory::Series, &series);
        self.browser_category_section(ui, BrowserCategory::Publishers, &publishers);
        self.browser_category_section(ui, BrowserCategory::Ratings, &ratings);
        self.browser_category_section(ui, BrowserCategory::Languages, &languages);
        if self.browser_open_dirty {
            self.browser_open_dirty = false;
        }
        ui.separator();
        ui.label("Saved searches");
        if self.browser_saved_searches.is_empty() {
            ui.label("No saved searches.");
        } else {
            egui::ScrollArea::vertical()
                .max_height(120.0)
                .show(ui, |ui| {
                    for (name, query) in &self.browser_saved_searches {
                        if ui.button(format!("{name}")).clicked() {
                            self.search_query = query.clone();
                            self.search_focus = true;
                            self.search_commit_requested = true;
                            self.needs_refresh = true;
                        }
                    }
                });
        }
    }

    fn browser_category_section(
        &mut self,
        ui: &mut egui::Ui,
        category: BrowserCategory,
        items: &[CategoryCount],
    ) {
        let query = self.browser_query.trim().to_lowercase();
        let mut entries = items.to_vec();
        match self.browser_sort {
            BrowserSort::Name => entries.sort_by(|a, b| a.name.cmp(&b.name)),
            BrowserSort::Count => entries.sort_by(|a, b| a.count.cmp(&b.count)),
        }
        if self.browser_sort_desc {
            entries.reverse();
        }
        let open_state = match category {
            BrowserCategory::Authors => self.browser_open_authors,
            BrowserCategory::Tags => self.browser_open_tags,
            BrowserCategory::Series => self.browser_open_series,
            BrowserCategory::Publishers => self.browser_open_publishers,
            BrowserCategory::Ratings => self.browser_open_ratings,
            BrowserCategory::Languages => self.browser_open_languages,
        };
        let id = ui.make_persistent_id(("browser_category", category.label()));
        let mut header = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            id,
            open_state,
        )
        .show_header(ui, |ui| {
            ui.label(category.label());
        });
        if self.browser_open_dirty {
            header.set_open(open_state);
        }
        let is_open = header.is_open();
        header.body_unindented(|ui| {
            if entries.is_empty() {
                ui.label("No entries.");
                return;
            }
            egui::ScrollArea::vertical()
                .max_height(140.0)
                .show(ui, |ui| {
                    for entry in &entries {
                        if !query.is_empty() && !entry.name.to_lowercase().contains(&query) {
                            continue;
                        }
                        let mode = self.browser_filter_mode(category, &entry.name);
                        let selected = mode.is_some();
                        let label = format!(
                            "{} ({})",
                            hierarchical_category_label(category, &entry.name),
                            entry.count
                        );
                        if ui.selectable_label(selected, label).clicked() {
                            self.cycle_browser_filter(category, &entry.name);
                            self.apply_filters();
                        }
                    }
                });
        });
        match category {
            BrowserCategory::Authors => self.browser_open_authors = is_open,
            BrowserCategory::Tags => self.browser_open_tags = is_open,
            BrowserCategory::Series => self.browser_open_series = is_open,
            BrowserCategory::Publishers => self.browser_open_publishers = is_open,
            BrowserCategory::Ratings => self.browser_open_ratings = is_open,
            BrowserCategory::Languages => self.browser_open_languages = is_open,
        }
    }

    fn clear_all_filters(&mut self) {
        self.search_query.clear();
        self.format_filter = None;
        self.browser_filters.clear();
        self.persist_active_virtual_filters();
        self.needs_refresh = true;
        self.apply_filters();
    }

    fn browser_filter_mode(
        &self,
        category: BrowserCategory,
        value: &str,
    ) -> Option<BrowserFilterMode> {
        self.browser_filters
            .iter()
            .find(|filter| filter.category == category && filter.value == value)
            .map(|filter| filter.mode)
    }

    fn cycle_browser_filter(&mut self, category: BrowserCategory, value: &str) {
        if let Some(pos) = self
            .browser_filters
            .iter()
            .position(|filter| filter.category == category && filter.value == value)
        {
            match self.browser_filters[pos].mode {
                BrowserFilterMode::Include => {
                    self.browser_filters[pos].mode = BrowserFilterMode::Exclude;
                }
                BrowserFilterMode::Exclude => {
                    self.browser_filters.remove(pos);
                }
            }
        } else {
            self.browser_filters.push(BrowserFilter {
                category,
                value: value.to_string(),
                mode: BrowserFilterMode::Include,
            });
        }
        self.persist_active_virtual_filters();
        self.config_dirty = true;
    }

    fn remove_browser_filter(&mut self, filter: &BrowserFilter) {
        self.browser_filters
            .retain(|item| !(item.category == filter.category && item.value == filter.value));
        self.persist_active_virtual_filters();
        self.config_dirty = true;
    }

    fn set_active_virtual_library(&mut self, selected: Option<String>) {
        self.active_virtual_library = selected.clone();
        if let Some(name) = selected {
            if let Some((_, query)) = self
                .browser_saved_searches
                .iter()
                .find(|(saved_name, _)| saved_name == &name)
            {
                self.search_query = query.clone();
                self.search_commit_requested = true;
                self.search_focus = false;
            }
            self.browser_filters = self
                .virtual_library_filters
                .get(&name)
                .cloned()
                .unwrap_or_default();
        } else {
            self.browser_filters.clear();
        }
        self.needs_refresh = true;
        self.apply_filters();
        self.config_dirty = true;
    }

    fn persist_active_virtual_filters(&mut self) {
        if let Some(name) = &self.active_virtual_library {
            self.virtual_library_filters
                .insert(name.clone(), self.browser_filters.clone());
            self.config_dirty = true;
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(format!("Books: {}", self.books.len()));
            if self.books.len() != self.all_books.len() {
                ui.label(format!("Filtered from {}", self.all_books.len()));
            }
            ui.label(format!("Selected: {}", self.selected_ids.len()));
            ui.label(format!("Jobs: {}", self.jobs.len()));
            if let Some(active) = &self.active_virtual_library {
                ui.label(format!("Virtual library: {active}"));
            }
            if !self.browser_saved_searches.is_empty() {
                let mut selected = None::<String>;
                egui::ComboBox::from_id_salt("saved_search_quick")
                    .selected_text("Saved search")
                    .show_ui(ui, |ui| {
                        for (name, _) in &self.browser_saved_searches {
                            if ui.selectable_label(false, name).clicked() {
                                selected = Some(name.clone());
                            }
                        }
                    });
                if let Some(name) = selected {
                    self.set_active_virtual_library(Some(name));
                }
            }
        });
    }

    fn quick_details_preview(&mut self, ui: &mut egui::Ui) {
        ui.label("Quick details");
        if let Some(details) = &self.details {
            ui.label(format!("Title: {}", details.book.title));
            ui.label(format!("Authors: {}", details.authors.join(", ")));
            ui.label(format!(
                "Series: {}",
                details
                    .series
                    .as_ref()
                    .map(|s| s.name.clone())
                    .unwrap_or_default()
            ));
            ui.label(format!("Tags: {}", details.tags.join(", ")));
        } else {
            ui.label("No selected book.");
        }
    }

    fn library_stats_panel(&mut self, ui: &mut egui::Ui, cache_dir: &Path) {
        egui::CollapsingHeader::new("Library stats")
            .default_open(false)
            .show(ui, |ui| {
                let stats = compute_library_stats(&self.all_books);
                ui.label(format!("Formats: {}", stats.formats.len()));
                ui.label(format!("Languages: {}", stats.languages.len()));
                ui.label(format!("Tags: {}", stats.tags.len()));
                ui.label(format!("Authors: {}", stats.authors.len()));
                ui.label(format!("Series: {}", stats.series.len()));
                ui.separator();
                ui.label("Formats distribution");
                for (format, count) in stats.formats.iter().take(8) {
                    ui.label(format!("{format}: {count}"));
                }
                ui.separator();
                if ui.button("Refresh stats").clicked() {
                    self.needs_refresh = true;
                }
                if ui.button("Export stats CSV").clicked() {
                    match export_stats_csv(cache_dir, &stats) {
                        Ok(path) => {
                            self.push_toast(
                                &format!("Exported stats: {}", path.display()),
                                ToastLevel::Info,
                            );
                        }
                        Err(err) => self.set_error(err),
                    }
                }
            });
    }

    fn row_context_menu(&mut self, ui: &mut egui::Ui, config: &ControlPlane, book: &BookRow) {
        if ui.button("Edit metadata").clicked() {
            self.select_book(book.id);
            self.begin_edit();
            ui.close_menu();
        }
        if ui.button("Remove book").clicked() {
            self.selected_ids = vec![book.id];
            self.open_remove_books(config);
            ui.close_menu();
        }
        if ui.button("Convert").clicked() {
            self.selected_ids = vec![book.id];
            self.open_convert_books(config);
            ui.close_menu();
        }
        if ui.button("Save to disk").clicked() {
            self.selected_ids = vec![book.id];
            self.open_save_to_disk(config);
            ui.close_menu();
        }
        if ui.button("Open in reader").clicked() {
            if let Err(err) = self.open_reader(book.id) {
                self.set_error(err);
            }
            ui.close_menu();
        }
        if ui.button("Open file").clicked() {
            if let Err(err) = open_path(Path::new(&book.path)) {
                self.set_error(err);
            }
            ui.close_menu();
        }
        if ui.button("Open folder").clicked() {
            if let Some(parent) = Path::new(&book.path).parent() {
                if let Err(err) = open_path(parent) {
                    self.set_error(err);
                }
            }
            ui.close_menu();
        }
    }

    fn layout_controls(
        &mut self,
        ui: &mut egui::Ui,
        config: &mut ControlPlane,
        config_path: &Path,
    ) {
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
        ui.horizontal(|ui| {
            ui.label("Density");
            egui::ComboBox::from_id_salt("view_density")
                .selected_text(match self.view_density {
                    ViewDensity::Compact => "Compact",
                    ViewDensity::Comfortable => "Comfortable",
                })
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(&mut self.view_density, ViewDensity::Compact, "Compact")
                        .clicked()
                    {
                        self.layout_dirty = true;
                    }
                    if ui
                        .selectable_value(
                            &mut self.view_density,
                            ViewDensity::Comfortable,
                            "Comfortable",
                        )
                        .clicked()
                    {
                        self.layout_dirty = true;
                    }
                });
            if ui
                .checkbox(&mut self.quick_details_panel, "Quick details")
                .changed()
            {
                self.layout_dirty = true;
            }
            if self.view_mode == ViewMode::Grid {
                let mut zoom = self.cover_thumb_size;
                if ui
                    .add(egui::Slider::new(&mut zoom, 48.0..=140.0).text("Cover zoom"))
                    .changed()
                {
                    self.cover_thumb_size = zoom;
                    self.layout_dirty = true;
                }
            }
        });
        ui.horizontal(|ui| {
            if ui
                .checkbox(&mut self.show_format_badges, "Format badges")
                .changed()
            {
                self.config_dirty = true;
            }
            if ui
                .checkbox(&mut self.show_language_badges, "Language badges")
                .changed()
            {
                self.config_dirty = true;
            }
        });

        egui::CollapsingHeader::new("Columns")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("Visible columns");
                if ui.checkbox(&mut self.columns.title, "Title").changed() {
                    self.layout_dirty = true;
                }
                if ui.checkbox(&mut self.columns.authors, "Authors").changed() {
                    self.layout_dirty = true;
                }
                if ui.checkbox(&mut self.columns.series, "Series").changed() {
                    self.layout_dirty = true;
                }
                if ui.checkbox(&mut self.columns.tags, "Tags").changed() {
                    self.layout_dirty = true;
                }
                if ui.checkbox(&mut self.columns.formats, "Formats").changed() {
                    self.layout_dirty = true;
                }
                if ui.checkbox(&mut self.columns.rating, "Rating").changed() {
                    self.layout_dirty = true;
                }
                if ui
                    .checkbox(&mut self.columns.publisher, "Publisher")
                    .changed()
                {
                    self.layout_dirty = true;
                }
                if ui
                    .checkbox(&mut self.columns.languages, "Languages")
                    .changed()
                {
                    self.layout_dirty = true;
                }
                if ui.checkbox(&mut self.columns.cover, "Cover").changed() {
                    self.layout_dirty = true;
                }
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
                column_width_control(
                    ui,
                    "Cover",
                    &mut self.column_widths.cover,
                    &mut self.layout_dirty,
                );
                if ui.button("Save Layout").clicked() {
                    self.persist_layout(config, config_path);
                }
            });
    }

    fn operations_controls(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        egui::CollapsingHeader::new("Operations")
            .default_open(true)
            .show(ui, |ui| {
                if ui.button("Add books…").clicked() {
                    self.open_add_books(config);
                }
                if ui
                    .add_enabled(
                        !self.selected_ids.is_empty(),
                        egui::Button::new("Remove books…"),
                    )
                    .clicked()
                {
                    self.open_remove_books(config);
                }
                if ui
                    .add_enabled(self.selected_ids.len() > 1, egui::Button::new("Bulk edit…"))
                    .clicked()
                {
                    self.open_bulk_edit();
                }
                if ui
                    .add_enabled(
                        !self.selected_ids.is_empty(),
                        egui::Button::new("Convert books…"),
                    )
                    .clicked()
                {
                    self.open_convert_books(config);
                }
                if ui
                    .add_enabled(
                        !self.selected_ids.is_empty(),
                        egui::Button::new("Save to disk…"),
                    )
                    .clicked()
                {
                    self.open_save_to_disk(config);
                }
                if ui
                    .add_enabled(
                        !self.selected_ids.is_empty(),
                        egui::Button::new("Send to device…"),
                    )
                    .clicked()
                {
                    self.open_device_sync(config);
                }
            });
    }

    fn management_controls(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Manage")
            .default_open(false)
            .show(ui, |ui| {
                if ui.button("Tags…").clicked() {
                    self.open_manage_tags();
                }
                if ui.button("Series…").clicked() {
                    self.open_manage_series();
                }
                if ui.button("Custom columns…").clicked() {
                    self.open_manage_custom_columns();
                }
                if ui.button("Virtual libraries…").clicked() {
                    self.open_manage_virtual_libraries();
                }
            });
    }

    fn table_view(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        let density_factor = match self.view_density {
            ViewDensity::Compact => 0.85,
            ViewDensity::Comfortable => 1.0,
        };
        let row_height = (self.table_row_height * density_factor).max(30.0);
        let mut table = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0);

        if self.columns.title {
            table = table.column(column_with_width(self.column_widths.title));
        }
        if self.columns.cover {
            table = table.column(column_with_width(self.column_widths.cover));
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
                if self.columns.cover {
                    header.col(|ui| {
                        ui.label("Cover");
                    });
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
                    let book = self.books[row_index].clone();
                    let selected = self.selected_ids.contains(&book.id);
                    let mut row_clicked = false;
                    let mut modifiers = egui::Modifiers::default();
                    if self.columns.title {
                        row.col(|ui: &mut egui::Ui| {
                            ui.horizontal(|ui| {
                                let response = ui.selectable_label(
                                    selected,
                                    highlight_text(&book.title, &self.search_query),
                                );
                                if response.clicked() {
                                    row_clicked = true;
                                    modifiers = response.ctx.input(|i| i.modifiers);
                                }
                                response.context_menu(|ui| {
                                    self.row_context_menu(ui, config, &book);
                                });
                                if ui
                                    .small_button("E")
                                    .on_hover_text("Edit metadata")
                                    .clicked()
                                {
                                    self.select_book(book.id);
                                    self.begin_edit();
                                }
                                if ui.small_button("R").on_hover_text("Remove book").clicked() {
                                    self.selected_ids = vec![book.id];
                                    self.open_remove_books(config);
                                }
                                if ui.small_button("C").on_hover_text("Convert book").clicked() {
                                    self.selected_ids = vec![book.id];
                                    self.open_convert_books(config);
                                }
                            });
                        });
                    }
                    if self.columns.cover {
                        row.col(|ui: &mut egui::Ui| {
                            let texture =
                                self.cover_thumb_texture(ui.ctx(), book.id, book.has_cover);
                            render_cover_thumbnail(
                                ui,
                                texture.as_ref(),
                                book.has_cover,
                                self.cover_thumb_size,
                            );
                        });
                    }
                    if self.columns.authors {
                        row.col(|ui: &mut egui::Ui| {
                            let response = ui.selectable_label(
                                selected,
                                highlight_text(&book.authors, &self.search_query),
                            );
                            if response.clicked() {
                                row_clicked = true;
                                modifiers = response.ctx.input(|i| i.modifiers);
                            }
                        });
                    }
                    if self.columns.series {
                        row.col(|ui: &mut egui::Ui| {
                            let response = ui.selectable_label(
                                selected,
                                highlight_text(&book.series, &self.search_query),
                            );
                            if response.clicked() {
                                row_clicked = true;
                                modifiers = response.ctx.input(|i| i.modifiers);
                            }
                        });
                    }
                    if self.columns.tags {
                        row.col(|ui: &mut egui::Ui| {
                            let response = ui.selectable_label(
                                selected,
                                highlight_text(&book.tags, &self.search_query),
                            );
                            if response.clicked() {
                                row_clicked = true;
                                modifiers = response.ctx.input(|i| i.modifiers);
                            }
                        });
                    }
                    if self.columns.formats {
                        row.col(|ui: &mut egui::Ui| {
                            let text = if self.show_format_badges {
                                format_badge_text(&book.format)
                            } else {
                                book.format.clone()
                            };
                            let response = ui.selectable_label(selected, text);
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
                            let text = if self.show_language_badges {
                                language_badge_text(&book.languages)
                            } else {
                                book.languages.clone()
                            };
                            let response = ui.selectable_label(selected, text);
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

    fn grid_view(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        let books = self.books.clone();
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut row = 0;
            let mut col = 0;
            let columns = match self.view_density {
                ViewDensity::Compact => 4,
                ViewDensity::Comfortable => 3,
            };
            for book in &books {
                if col == 0 {
                    ui.horizontal(|ui| {
                        self.grid_cell(ui, config, book);
                        col += 1;
                    });
                } else {
                    self.grid_cell(ui, config, book);
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

    fn grid_cell(&mut self, ui: &mut egui::Ui, config: &ControlPlane, book: &BookRow) {
        let selected = self.selected_ids.contains(&book.id);
        let frame = egui::Frame::group(ui.style()).fill(if selected {
            egui::Color32::from_gray(60)
        } else {
            egui::Color32::from_gray(30)
        });
        frame.show(ui, |ui| {
            ui.set_min_width(140.0);
            let texture = self.cover_thumb_texture(ui.ctx(), book.id, book.has_cover);
            render_cover_thumbnail(ui, texture.as_ref(), book.has_cover, self.cover_thumb_size);
            ui.label(&book.title);
            if !book.authors.is_empty() {
                ui.label(&book.authors);
            }
            if ui.button("Select").clicked() {
                self.select_book(book.id);
            }
        });
        let response = ui.interact(
            ui.min_rect(),
            egui::Id::new(("grid_cell", book.id)),
            egui::Sense::click(),
        );
        response.context_menu(|ui| {
            self.row_context_menu(ui, config, book);
        });
    }

    fn details_view(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        ui.heading("Details");
        ui.separator();
        let mut action = DetailAction::None;
        let mut open_paths: Vec<PathBuf> = Vec::new();
        let details_snapshot = self.details.clone();

        match &details_snapshot {
            Some(details) => {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!self.edit_mode, egui::Button::new("Edit"))
                        .clicked()
                    {
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
                    details
                        .extras
                        .publisher
                        .clone()
                        .unwrap_or_else(|| "none".to_string())
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
                ui.heading("Cover");
                let cover_texture =
                    self.cover_preview_texture(ui.ctx(), details.book.id, details.extras.has_cover);
                render_cover_preview(
                    ui,
                    cover_texture.as_ref(),
                    details.extras.has_cover,
                    self.cover_preview_size,
                );
                ui.horizontal(|ui| {
                    if ui.button("Set cover").clicked() {
                        action = DetailAction::SetCover;
                    }
                    if ui.button("Remove cover").clicked() {
                        action = DetailAction::RemoveCover;
                    }
                    if ui.button("Generate cover").clicked() {
                        action = DetailAction::GenerateCover;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Cover file");
                    ui.text_edit_singleline(&mut self.cover_state.cover_path_input);
                    if ui.button("Apply").clicked() {
                        action = DetailAction::SetCover;
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Use asset as cover").clicked() {
                        if let Some(asset) = details.assets.first() {
                            let candidate = Path::new(&asset.stored_path);
                            if is_image_path(candidate) {
                                action = DetailAction::SetCover;
                                self.cover_state.cover_path_input = asset.stored_path.clone();
                            } else {
                                self.push_toast(
                                    "First asset is not an image; choose a PNG/JPG file",
                                    ToastLevel::Warn,
                                );
                            }
                        }
                    }
                });

                ui.separator();
                ui.heading("Comment");
                if let Some(comment) = &details.comment {
                    if comment.is_empty() {
                        ui.label("No comment set.");
                    } else {
                        let render_markdown_enabled = self
                            .comment_render_overrides
                            .get(&details.book.id)
                            .copied()
                            .unwrap_or(self.comment_render_markdown);
                        if render_markdown_enabled {
                            render_markdown(ui, comment);
                        } else {
                            render_html_fallback(ui, comment);
                        }
                    }
                } else {
                    ui.label("No comment set.");
                }
                let mut render_toggle = self
                    .comment_render_overrides
                    .get(&details.book.id)
                    .copied()
                    .unwrap_or(self.comment_render_markdown);
                if ui
                    .checkbox(&mut render_toggle, "Render markdown for this book")
                    .changed()
                {
                    self.comment_render_overrides
                        .insert(details.book.id, render_toggle);
                }

                ui.separator();
                ui.heading("Notes");
                ui.label(format!("{} notes", details.notes.len()));
                if details.notes.is_empty() {
                    ui.label("No notes yet.");
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(160.0)
                        .show(ui, |ui| {
                            for note in &details.notes {
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(&note.created_at);
                                        if ui.button("Delete").clicked() {
                                            self.note_delete_id = Some(note.id);
                                            self.note_delete_open = true;
                                        }
                                    });
                                    let mut text = note.text.clone();
                                    ui.add(
                                        egui::TextEdit::multiline(&mut text)
                                            .desired_rows(2)
                                            .interactive(false),
                                    );
                                });
                                ui.add_space(6.0);
                            }
                        });
                }
                ui.label("Add note");
                ui.text_edit_multiline(&mut self.note_input);
                if ui.button("Save note").clicked() {
                    if let Err(err) = self.add_note_for_book(details.book.id) {
                        self.set_error(err);
                    } else {
                        let _ = self.load_details(details.book.id);
                        self.push_toast("Note added", ToastLevel::Info);
                    }
                }

                ui.separator();
                ui.heading("Formats");
                if details.assets.is_empty() {
                    ui.label("No assets recorded.");
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for asset in &details.assets {
                            let storage_label = asset.storage_mode.as_str();
                            let compression_label = if asset.is_compressed {
                                "compressed"
                            } else {
                                "raw"
                            };
                            let size_label = format!(
                                "{} bytes (stored {} bytes)",
                                asset.size_bytes, asset.stored_size_bytes
                            );
                            ui.horizontal(|ui| {
                                ui.label(format!(
                                    "{} | {} | {} | {}",
                                    asset.stored_path, storage_label, compression_label, size_label
                                ));
                                if ui.button("Open").clicked() {
                                    if asset.is_compressed {
                                        self.push_toast(
                                            "Compressed asset: use Save to Disk to extract",
                                            ToastLevel::Warn,
                                        );
                                    } else {
                                        open_paths.push(PathBuf::from(&asset.stored_path));
                                    }
                                }
                                if ui.button("Convert").clicked() {
                                    self.pending_convert_book = Some(details.book.id);
                                }
                                if ui.button("Remove").clicked() {
                                    self.remove_asset_dialog.apply_defaults(config, asset);
                                }
                            });
                        }
                    });
                }

                ui.separator();
                ui.heading("Actions");
                ui.horizontal(|ui| {
                    if ui.button("Open in reader").clicked() {
                        action = DetailAction::OpenReader;
                    }
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

        if let Some(details) = &details_snapshot {
            let dropped = ui.ctx().input(|i| i.raw.dropped_files.clone());
            for file in dropped {
                if let Some(path) = file.path {
                    if is_image_path(&path) {
                        if let Err(err) = self.apply_cover_from_path(details.book.id, &path) {
                            self.set_error(err);
                        } else {
                            let _ = self.load_details(details.book.id);
                            self.push_toast("Cover updated from drop", ToastLevel::Info);
                        }
                        break;
                    }
                }
            }
        }

        match action {
            DetailAction::BeginEdit => self.begin_edit(),
            DetailAction::Save => self.pending_save = true,
            DetailAction::Cancel => self.cancel_edit(),
            DetailAction::SetCover => {
                if let Some(details) = &details_snapshot {
                    if let Err(err) = self.set_cover_from_input(details.book.id) {
                        self.set_error(err);
                    } else {
                        let _ = self.load_details(details.book.id);
                        self.push_toast("Cover updated", ToastLevel::Info);
                    }
                }
            }
            DetailAction::RemoveCover => {
                if let Some(details) = &details_snapshot {
                    if let Err(err) = self.remove_cover(details.book.id) {
                        self.set_error(err);
                    } else {
                        let _ = self.load_details(details.book.id);
                        self.push_toast("Cover removed", ToastLevel::Info);
                    }
                }
            }
            DetailAction::GenerateCover => {
                if let Some(details) = &details_snapshot {
                    if let Err(err) = self.generate_cover(details.book.id, &details.book.title) {
                        self.set_error(err);
                    } else {
                        let _ = self.load_details(details.book.id);
                        self.push_toast("Generated cover", ToastLevel::Info);
                    }
                }
            }
            DetailAction::OpenReader => {
                if let Some(details) = &details_snapshot {
                    if let Err(err) = self.open_reader(details.book.id) {
                        self.set_error(err);
                    }
                }
            }
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
                ui.checkbox(&mut self.comment_preview, "Preview comment");
                if self.comment_preview {
                    ui.separator();
                    ui.label("Preview");
                    render_markdown(ui, &self.edit.comment);
                }
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

    fn add_books_dialog(&mut self, ui: &mut egui::Ui, config: &mut ControlPlane) {
        if !self.add_books.open {
            return;
        }
        let mut open = self.add_books.open;
        let mut confirmed = false;
        let mut close_requested = false;
        egui::Window::new("Add books")
            .open(&mut open)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                ui.label("Files (one per line)");
                ui.text_edit_multiline(&mut self.add_books.files_input);
                ui.label("Folder");
                ui.text_edit_singleline(&mut self.add_books.folder_input);
                ui.horizontal(|ui| {
                    ui.label("Mode");
                    egui::ComboBox::from_id_salt("add_books_mode")
                        .selected_text(format!("{:?}", self.add_books.mode))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.add_books.mode, IngestMode::Copy, "Copy");
                            ui.selectable_value(
                                &mut self.add_books.mode,
                                IngestMode::Reference,
                                "Reference",
                            );
                        });
                });
                if config.ingest.archive_reference_enabled {
                    ui.checkbox(
                        &mut self.add_books.archive_reference,
                        "Treat archives as references",
                    );
                } else {
                    ui.label("Archive reference disabled in config");
                }
                ui.checkbox(
                    &mut self.add_books.include_archives,
                    "Include archive formats",
                );
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Add").clicked() {
                        confirmed = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_requested = true;
                    }
                });
            });
        if confirmed {
            match self.run_add_books(config) {
                Ok(()) => {
                    close_requested = true;
                }
                Err(err) => {
                    self.set_error(err);
                }
            }
        }
        if close_requested {
            open = false;
        }
        self.add_books.open = open;
    }

    fn remove_books_dialog(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        if !self.remove_books.open {
            return;
        }
        let mut open = self.remove_books.open;
        let mut confirmed = false;
        let mut close_requested = false;
        egui::Window::new("Remove books")
            .open(&mut open)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.label(format!(
                    "Remove {} selected book(s)",
                    self.selected_ids.len()
                ));
                ui.checkbox(&mut self.remove_books.delete_files, "Delete stored files");
                ui.checkbox(
                    &mut self.remove_books.delete_reference_files,
                    "Delete referenced files",
                );
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Remove").clicked() {
                        confirmed = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_requested = true;
                    }
                });
            });
        if confirmed {
            match self.run_remove_books(config) {
                Ok(()) => {
                    close_requested = true;
                }
                Err(err) => {
                    self.set_error(err);
                }
            }
        }
        if close_requested {
            open = false;
        }
        self.remove_books.open = open;
    }

    fn remove_asset_dialog(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        if !self.remove_asset_dialog.open {
            return;
        }
        let Some(asset) = self.remove_asset_dialog.asset.clone() else {
            self.remove_asset_dialog.open = false;
            return;
        };
        let mut open = self.remove_asset_dialog.open;
        let mut confirmed = false;
        let mut close_requested = false;
        egui::Window::new("Remove asset")
            .open(&mut open)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.label(format!("Remove asset {}", asset.stored_path));
                ui.checkbox(
                    &mut self.remove_asset_dialog.delete_files,
                    "Delete stored file",
                );
                ui.checkbox(
                    &mut self.remove_asset_dialog.delete_reference_files,
                    "Delete referenced file",
                );
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Remove").clicked() {
                        confirmed = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_requested = true;
                    }
                });
            });
        if confirmed {
            if let Err(err) =
                self.run_remove_asset(config, &asset, self.remove_asset_dialog.delete_files)
            {
                self.set_error(err);
            } else {
                if let Some(details) = &self.details {
                    let _ = self.load_details(details.book.id);
                }
                self.push_toast("Asset removed", ToastLevel::Info);
                close_requested = true;
            }
        }
        if close_requested {
            open = false;
            self.remove_asset_dialog.asset = None;
        }
        self.remove_asset_dialog.open = open;
    }

    fn note_delete_dialog(&mut self, ui: &mut egui::Ui) {
        if !self.note_delete_open {
            return;
        }
        let mut open = self.note_delete_open;
        let mut confirmed = false;
        let mut close_requested = false;
        egui::Window::new("Delete note")
            .open(&mut open)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.label("Delete this note?");
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        confirmed = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_requested = true;
                    }
                });
            });
        if confirmed {
            if let Some(note_id) = self.note_delete_id {
                if let Err(err) = self.db.delete_note(note_id) {
                    self.set_error(err);
                } else if let Some(details) = &self.details {
                    let _ = self.load_details(details.book.id);
                    self.push_toast("Note deleted", ToastLevel::Info);
                }
            }
            close_requested = true;
        }
        if close_requested {
            open = false;
            self.note_delete_id = None;
        }
        self.note_delete_open = open;
    }

    fn bulk_edit_dialog(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        if !self.bulk_edit.open {
            return;
        }
        let mut open = self.bulk_edit.open;
        let mut confirmed = false;
        let mut close_requested = false;
        egui::Window::new("Bulk edit metadata")
            .open(&mut open)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                ui.label(format!(
                    "Editing {} selected book(s)",
                    self.selected_ids.len()
                ));
                ui.separator();
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.bulk_edit.apply_tags, "Apply tags");
                    ui.checkbox(&mut self.bulk_edit.replace_tags, "Replace");
                });
                ui.text_edit_singleline(&mut self.bulk_edit.tags);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.bulk_edit.apply_series, "Apply series");
                    ui.text_edit_singleline(&mut self.bulk_edit.series_name);
                    ui.add(
                        egui::DragValue::new(&mut self.bulk_edit.series_index)
                            .speed(0.1)
                            .range(0.0..=999.0),
                    );
                });
                ui.separator();
                ui.checkbox(&mut self.bulk_edit.apply_publisher, "Apply publisher");
                ui.text_edit_singleline(&mut self.bulk_edit.publisher);
                ui.checkbox(&mut self.bulk_edit.clear_publisher, "Clear publisher");
                ui.separator();
                ui.checkbox(&mut self.bulk_edit.apply_languages, "Apply languages");
                ui.text_edit_singleline(&mut self.bulk_edit.languages);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.bulk_edit.apply_rating, "Apply rating");
                    ui.add(
                        egui::DragValue::new(&mut self.bulk_edit.rating)
                            .speed(1.0)
                            .range(0..=5),
                    );
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked() {
                        confirmed = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_requested = true;
                    }
                });
            });
        if confirmed {
            match self.run_bulk_edit(config) {
                Ok(()) => {
                    close_requested = true;
                }
                Err(err) => {
                    self.set_error(err);
                }
            }
        }
        if close_requested {
            open = false;
        }
        self.bulk_edit.open = open;
    }

    fn convert_books_dialog(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        if !self.convert_books.open {
            return;
        }
        let mut open = self.convert_books.open;
        let mut confirmed = false;
        let mut close_requested = false;
        egui::Window::new("Convert books")
            .open(&mut open)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                ui.label(format!(
                    "Convert {} selected book(s)",
                    self.selected_ids.len()
                ));
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Output format");
                    egui::ComboBox::from_id_salt("convert_output_format")
                        .selected_text(self.convert_books.output_format.as_str())
                        .show_ui(ui, |ui| {
                            for format in &config.formats.supported {
                                ui.selectable_value(
                                    &mut self.convert_books.output_format,
                                    format.clone(),
                                    format,
                                );
                            }
                        });
                });
                ui.label("Output directory");
                ui.text_edit_singleline(&mut self.convert_books.output_dir);
                ui.checkbox(
                    &mut self.convert_books.add_to_library,
                    "Add converted format to library",
                );
                ui.checkbox(&mut self.convert_books.keep_output, "Keep output file");
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Convert").clicked() {
                        confirmed = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_requested = true;
                    }
                });
            });
        if confirmed {
            match self.run_convert_books(config) {
                Ok(()) => {
                    close_requested = true;
                }
                Err(err) => {
                    self.set_error(err);
                }
            }
        }
        if close_requested {
            open = false;
        }
        self.convert_books.open = open;
    }

    fn save_to_disk_dialog(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        if !self.save_to_disk.open {
            return;
        }
        let mut open = self.save_to_disk.open;
        let mut confirmed = false;
        let mut close_requested = false;
        egui::Window::new("Save to disk")
            .open(&mut open)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                ui.label(format!(
                    "Export {} selected book(s)",
                    self.selected_ids.len()
                ));
                ui.label("Output directory");
                ui.text_edit_singleline(&mut self.save_to_disk.output_dir);
                ui.checkbox(
                    &mut self.save_to_disk.export_all_formats,
                    "Export all formats",
                );
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Export").clicked() {
                        confirmed = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_requested = true;
                    }
                });
            });
        if confirmed {
            match self.run_save_to_disk(config) {
                Ok(()) => {
                    close_requested = true;
                }
                Err(err) => {
                    self.set_error(err);
                }
            }
        }
        if close_requested {
            open = false;
        }
        self.save_to_disk.open = open;
    }

    fn device_sync_dialog(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        if !self.device_sync.open {
            return;
        }
        let mut open = self.device_sync.open;
        let mut confirmed = false;
        let mut close_requested = false;
        egui::Window::new("Send to device")
            .open(&mut open)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                ui.label(format!("Send {} selected book(s)", self.selected_ids.len()));
                if let Some(err) = &self.device_sync.error {
                    ui.colored_label(egui::Color32::from_rgb(190, 0, 0), err);
                }
                ui.horizontal(|ui| {
                    ui.label("Device");
                    egui::ComboBox::from_id_salt("device_select")
                        .selected_text(
                            self.device_sync
                                .selected_device
                                .and_then(|idx| self.device_sync.devices.get(idx))
                                .map(|device| device.name.as_str())
                                .unwrap_or("None"),
                        )
                        .show_ui(ui, |ui| {
                            for (idx, device) in self.device_sync.devices.iter().enumerate() {
                                ui.selectable_value(
                                    &mut self.device_sync.selected_device,
                                    Some(idx),
                                    device.name.as_str(),
                                );
                            }
                        });
                });
                ui.label("Destination name override (optional)");
                ui.text_edit_singleline(&mut self.device_sync.destination_name);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Send").clicked() {
                        confirmed = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_requested = true;
                    }
                });
            });
        if confirmed {
            match self.run_device_sync(config) {
                Ok(()) => {
                    close_requested = true;
                }
                Err(err) => {
                    self.set_error(err);
                }
            }
        }
        if close_requested {
            open = false;
        }
        self.device_sync.open = open;
    }

    fn manage_tags_dialog(&mut self, ui: &mut egui::Ui) {
        if !self.manage_tags.open {
            return;
        }
        if self.manage_tags.needs_refresh {
            if let Err(err) = self.refresh_manage_tags() {
                self.set_error(err);
            }
        }
        let mut open = self.manage_tags.open;
        let mut rename = false;
        let mut delete = false;
        egui::Window::new("Manage tags")
            .open(&mut open)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                ui.label("Tags");
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for tag in &self.manage_tags.tags {
                            ui.label(format!("{} ({})", tag.name, tag.count));
                        }
                    });
                ui.separator();
                ui.label("Rename tag");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.manage_tags.rename_from);
                    ui.label("→");
                    ui.text_edit_singleline(&mut self.manage_tags.rename_to);
                });
                if ui.button("Rename").clicked() {
                    rename = true;
                }
                ui.separator();
                ui.label("Delete tag");
                ui.text_edit_singleline(&mut self.manage_tags.delete_name);
                if ui.button("Delete").clicked() {
                    delete = true;
                }
            });
        if rename {
            if let Err(err) = self
                .db
                .rename_tag(&self.manage_tags.rename_from, &self.manage_tags.rename_to)
            {
                self.set_error(err);
            } else {
                self.manage_tags.needs_refresh = true;
                self.needs_refresh = true;
            }
        }
        if delete {
            if let Err(err) = self.db.delete_tag(&self.manage_tags.delete_name) {
                self.set_error(err);
            } else {
                self.manage_tags.needs_refresh = true;
                self.needs_refresh = true;
            }
        }
        self.manage_tags.open = open;
    }

    fn manage_series_dialog(&mut self, ui: &mut egui::Ui) {
        if !self.manage_series.open {
            return;
        }
        if self.manage_series.needs_refresh {
            if let Err(err) = self.refresh_manage_series() {
                self.set_error(err);
            }
        }
        let mut open = self.manage_series.open;
        let mut rename = false;
        let mut delete = false;
        egui::Window::new("Manage series")
            .open(&mut open)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                ui.label("Series");
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for series in &self.manage_series.series {
                            ui.label(format!("{} ({})", series.name, series.count));
                        }
                    });
                ui.separator();
                ui.label("Rename series");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.manage_series.rename_from);
                    ui.label("→");
                    ui.text_edit_singleline(&mut self.manage_series.rename_to);
                });
                if ui.button("Rename").clicked() {
                    rename = true;
                }
                ui.separator();
                ui.label("Delete series");
                ui.text_edit_singleline(&mut self.manage_series.delete_name);
                if ui.button("Delete").clicked() {
                    delete = true;
                }
            });
        if rename {
            if let Err(err) = self.db.rename_series(
                &self.manage_series.rename_from,
                &self.manage_series.rename_to,
            ) {
                self.set_error(err);
            } else {
                self.manage_series.needs_refresh = true;
                self.needs_refresh = true;
            }
        }
        if delete {
            if let Err(err) = self.db.delete_series(&self.manage_series.delete_name) {
                self.set_error(err);
            } else {
                self.manage_series.needs_refresh = true;
                self.needs_refresh = true;
            }
        }
        self.manage_series.open = open;
    }

    fn manage_custom_columns_dialog(&mut self, ui: &mut egui::Ui) {
        if !self.manage_custom_columns.open {
            return;
        }
        if self.manage_custom_columns.needs_refresh {
            if let Err(err) = self.refresh_manage_custom_columns() {
                self.set_error(err);
            }
        }
        let mut open = self.manage_custom_columns.open;
        let mut create = false;
        let mut delete = false;
        egui::Window::new("Manage custom columns")
            .open(&mut open)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                ui.label("Custom columns");
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for column in &self.manage_custom_columns.columns {
                            ui.label(format!(
                                "{} ({}, {})",
                                column.label, column.name, column.datatype
                            ));
                        }
                    });
                ui.separator();
                ui.label("Create column");
                ui.text_edit_singleline(&mut self.manage_custom_columns.new_label);
                ui.text_edit_singleline(&mut self.manage_custom_columns.new_name);
                egui::ComboBox::from_id_salt("custom_column_datatype")
                    .selected_text(self.manage_custom_columns.new_datatype.as_str())
                    .show_ui(ui, |ui| {
                        for datatype in ["text", "int", "float", "bool"] {
                            ui.selectable_value(
                                &mut self.manage_custom_columns.new_datatype,
                                datatype.to_string(),
                                datatype,
                            );
                        }
                    });
                ui.text_edit_singleline(&mut self.manage_custom_columns.new_display);
                if ui.button("Create").clicked() {
                    create = true;
                }
                ui.separator();
                ui.label("Delete column (label)");
                ui.text_edit_singleline(&mut self.manage_custom_columns.delete_label);
                if ui.button("Delete").clicked() {
                    delete = true;
                }
            });
        if create {
            if let Err(err) = self.db.create_custom_column(
                &self.manage_custom_columns.new_label,
                &self.manage_custom_columns.new_name,
                &self.manage_custom_columns.new_datatype,
                &self.manage_custom_columns.new_display,
            ) {
                self.set_error(err);
            } else {
                self.manage_custom_columns.needs_refresh = true;
            }
        }
        if delete {
            if let Err(err) = self
                .db
                .delete_custom_column(&self.manage_custom_columns.delete_label)
            {
                self.set_error(err);
            } else {
                self.manage_custom_columns.needs_refresh = true;
            }
        }
        self.manage_custom_columns.open = open;
    }

    fn manage_virtual_libraries_dialog(&mut self, ui: &mut egui::Ui) {
        if !self.manage_virtual_libraries.open {
            return;
        }
        if self.manage_virtual_libraries.needs_refresh {
            if let Err(err) = self.refresh_manage_virtual_libraries() {
                self.set_error(err);
            }
        }
        let mut open = self.manage_virtual_libraries.open;
        let mut add = false;
        let mut delete = false;
        egui::Window::new("Manage virtual libraries")
            .open(&mut open)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                ui.label("Saved searches");
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for (name, query) in &self.manage_virtual_libraries.searches {
                            ui.label(format!("{name}: {query}"));
                        }
                    });
                ui.separator();
                ui.label("Add saved search");
                ui.text_edit_singleline(&mut self.manage_virtual_libraries.new_name);
                ui.text_edit_singleline(&mut self.manage_virtual_libraries.new_query);
                if ui.button("Add").clicked() {
                    add = true;
                }
                ui.separator();
                ui.label("Remove saved search");
                ui.text_edit_singleline(&mut self.manage_virtual_libraries.delete_name);
                if ui.button("Remove").clicked() {
                    delete = true;
                }
            });
        if add {
            if let Err(err) = self.db.add_saved_search(
                &self.manage_virtual_libraries.new_name,
                &self.manage_virtual_libraries.new_query,
            ) {
                self.set_error(err);
            } else {
                self.manage_virtual_libraries.needs_refresh = true;
            }
        }
        if delete {
            if let Err(err) = self
                .db
                .remove_saved_search(&self.manage_virtual_libraries.delete_name)
            {
                self.set_error(err);
            } else {
                self.manage_virtual_libraries.needs_refresh = true;
            }
        }
        self.manage_virtual_libraries.open = open;
    }

    fn reader_dialog(&mut self, ui: &mut egui::Ui) {
        if !self.reader.open {
            return;
        }
        let mut open = self.reader.open;
        let mut close_requested = false;
        egui::Window::new("Reader")
            .open(&mut open)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                ui.heading(self.reader.title.as_str());
                ui.label(format!("Format: {}", self.reader.format));
                ui.horizontal(|ui| {
                    ui.label("Search");
                    ui.text_edit_singleline(&mut self.reader.search_query);
                    if ui.button("Find").clicked() {
                        self.reader.find_next();
                    }
                });
                if let Some(error) = &self.reader.error {
                    ui.colored_label(egui::Color32::from_rgb(190, 0, 0), error);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Previous").clicked() {
                        self.reader.prev_page();
                    }
                    if ui.button("Next").clicked() {
                        self.reader.next_page();
                    }
                    ui.label(format!(
                        "Page {} / {}",
                        self.reader.page + 1,
                        self.reader.page_count().max(1)
                    ));
                });
                if ui.ctx().input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                    self.reader.next_page();
                }
                if ui.ctx().input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                    self.reader.prev_page();
                }
                if ui.ctx().input(|i| i.key_pressed(egui::Key::PageDown)) {
                    self.reader.next_page();
                }
                if ui.ctx().input(|i| i.key_pressed(egui::Key::PageUp)) {
                    self.reader.prev_page();
                }
                ui.horizontal(|ui| {
                    ui.label("Font size");
                    ui.add(egui::DragValue::new(&mut self.reader.font_size).range(10.0..=28.0));
                    ui.label("Line spacing");
                    ui.add(
                        egui::DragValue::new(&mut self.reader.line_spacing)
                            .speed(0.05)
                            .range(1.1..=2.2),
                    );
                });
                let mut page_chars = self.reader.page_chars;
                ui.horizontal(|ui| {
                    ui.label("Page chars");
                    ui.add(egui::DragValue::new(&mut page_chars).range(600..=6000));
                });
                self.reader.update_page_chars(page_chars);
                ui.horizontal(|ui| {
                    ui.label("Theme");
                    egui::ComboBox::from_id_salt("reader_theme")
                        .selected_text(self.reader.theme.label())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.reader.theme,
                                ReaderTheme::Light,
                                "Light",
                            );
                            ui.selectable_value(&mut self.reader.theme, ReaderTheme::Dark, "Dark");
                            ui.selectable_value(
                                &mut self.reader.theme,
                                ReaderTheme::Sepia,
                                "Sepia",
                            );
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("Recent");
                    let recent = self
                        .reader
                        .recent
                        .iter()
                        .take(5)
                        .cloned()
                        .collect::<Vec<_>>();
                    for entry in recent {
                        if ui.button(&entry.title).clicked() {
                            self.reader.jump_to(entry.book_id, entry.page);
                        }
                    }
                });
                ui.separator();
                let background = self.reader.theme.background();
                let text_color = self.reader.theme.text_color();
                egui::Frame::none().fill(background).show(ui, |ui| {
                    ui.visuals_mut().override_text_color = Some(text_color);
                    ui.set_width(ui.available_width());
                    ui.add_space(4.0);
                    self.reader.render(ui);
                    ui.add_space(4.0);
                });
                if let Some(book_id) = self.reader.book_id {
                    self.reader_progress.insert(book_id, self.reader.page);
                }
                ui.separator();
                ui.label("Table of contents (stub)");
                ui.label("• Chapter 1");
                ui.label("• Chapter 2");
                if ui.button("Close").clicked() {
                    close_requested = true;
                }
            });
        if close_requested {
            open = false;
        }
        if !open {
            if let Some(book_id) = self.reader.book_id {
                self.reader_progress.insert(book_id, self.reader.page);
            }
            self.reader.close();
        }
        self.reader.open = open;
    }

    fn tag_autocomplete(&mut self, ui: &mut egui::Ui) {
        let query = self
            .edit
            .tags
            .split(',')
            .last()
            .unwrap_or("")
            .trim()
            .to_lowercase();
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

    fn cover_thumb_texture(
        &mut self,
        ctx: &egui::Context,
        book_id: i64,
        has_cover: bool,
    ) -> Option<egui::TextureHandle> {
        if !has_cover {
            return None;
        }
        if let Some(texture) = self.cover_cache.get(&book_id) {
            return Some(texture.clone());
        }
        let thumb_path = self.cover_thumb_path(book_id);
        if !thumb_path.exists() {
            let _ = self.ensure_cover_thumb(book_id);
        }
        if !thumb_path.exists() {
            return None;
        }
        match load_texture_from_path(ctx, &thumb_path) {
            Ok(texture) => {
                self.cover_cache.insert(book_id, texture.clone());
                Some(texture)
            }
            Err(err) => {
                warn!(
                    component = "gui",
                    book_id,
                    error = %err,
                    "failed to load cover thumbnail"
                );
                None
            }
        }
    }

    fn cover_preview_texture(
        &mut self,
        ctx: &egui::Context,
        book_id: i64,
        has_cover: bool,
    ) -> Option<egui::TextureHandle> {
        if !has_cover {
            return None;
        }
        if let Some(texture) = self.cover_preview_cache.get(&book_id) {
            return Some(texture.clone());
        }
        let cover_path = self.cover_path(book_id);
        if !cover_path.exists() {
            return None;
        }
        match load_texture_from_path(ctx, &cover_path) {
            Ok(texture) => {
                self.cover_preview_cache.insert(book_id, texture.clone());
                Some(texture)
            }
            Err(err) => {
                warn!(
                    component = "gui",
                    book_id,
                    error = %err,
                    "failed to load cover preview"
                );
                None
            }
        }
    }

    fn set_cover_from_input(&mut self, book_id: i64) -> CoreResult<()> {
        let path = self.cover_state.cover_path_input.trim().to_string();
        if path.is_empty() {
            return Err(CoreError::ConfigValidate(
                "cover path is required".to_string(),
            ));
        }
        self.apply_cover_from_path(book_id, Path::new(&path))
    }

    fn apply_cover_from_path(&mut self, book_id: i64, source: &Path) -> CoreResult<()> {
        if !source.is_file() {
            return Err(CoreError::ConfigValidate(
                "cover source must be a file".to_string(),
            ));
        }
        if !is_image_path(source) {
            return Err(CoreError::ConfigValidate(
                "cover source must be a PNG or JPG image".to_string(),
            ));
        }
        let metadata = fs::metadata(source)
            .map_err(|err| CoreError::Io("read cover metadata".to_string(), err))?;
        if metadata.len() > self.cover_max_bytes {
            return Err(CoreError::ConfigValidate(format!(
                "cover exceeds max size ({} bytes)",
                self.cover_max_bytes
            )));
        }
        let image =
            image::open(source).map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
        self.ensure_cover_dirs()?;
        let cover_path = self.cover_path(book_id);
        image
            .save_with_format(&cover_path, ImageFormat::Png)
            .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
        self.generate_cover_thumb_from_image(book_id, &image)?;
        self.db.update_book_has_cover(book_id, true)?;
        self.clear_cover_cache(book_id);
        Ok(())
    }

    fn generate_cover(&mut self, book_id: i64, title: &str) -> CoreResult<()> {
        self.ensure_cover_dirs()?;
        let cover_path = self.cover_path(book_id);
        let base = image::Rgb([45, 60, 90]);
        let mut img = image::RgbImage::from_pixel(400, 600, base);
        let banner = image::Rgb([80, 110, 160]);
        for y in 0..80 {
            for x in 0..400 {
                img.put_pixel(x, y, banner);
            }
        }
        let mut dynamic = DynamicImage::ImageRgb8(img);
        dynamic = dynamic.resize(400, 600, image::imageops::FilterType::Triangle);
        dynamic
            .save_with_format(&cover_path, ImageFormat::Png)
            .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
        self.generate_cover_thumb_from_image(book_id, &dynamic)?;
        self.db.update_book_has_cover(book_id, true)?;
        self.clear_cover_cache(book_id);
        info!(
            component = "gui",
            book_id,
            title = %title,
            "generated cover placeholder"
        );
        Ok(())
    }

    fn remove_cover(&mut self, book_id: i64) -> CoreResult<()> {
        let cover_path = self.cover_path(book_id);
        let thumb_path = self.cover_thumb_path(book_id);
        if cover_path.exists() {
            fs::remove_file(&cover_path)
                .map_err(|err| CoreError::Io("remove cover".to_string(), err))?;
        }
        if thumb_path.exists() {
            fs::remove_file(&thumb_path)
                .map_err(|err| CoreError::Io("remove cover thumb".to_string(), err))?;
        }
        self.db.update_book_has_cover(book_id, false)?;
        self.clear_cover_cache(book_id);
        Ok(())
    }

    fn ensure_cover_dirs(&self) -> CoreResult<()> {
        fs::create_dir_all(&self.cover_dir)
            .map_err(|err| CoreError::Io("create cover dir".to_string(), err))?;
        fs::create_dir_all(&self.cover_cache_dir)
            .map_err(|err| CoreError::Io("create cover cache dir".to_string(), err))?;
        Ok(())
    }

    fn ensure_cover_thumb(&mut self, book_id: i64) -> CoreResult<()> {
        let cover_path = self.cover_path(book_id);
        if !cover_path.exists() {
            return Ok(());
        }
        let image =
            image::open(&cover_path).map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
        self.generate_cover_thumb_from_image(book_id, &image)?;
        Ok(())
    }

    fn generate_cover_thumb_from_image(
        &self,
        book_id: i64,
        image: &DynamicImage,
    ) -> CoreResult<()> {
        self.ensure_cover_dirs()?;
        let width = self.cover_thumb_size.max(32.0) as u32;
        let height = (self.cover_thumb_size * 1.3).max(42.0) as u32;
        let resized = image.resize(width, height, image::imageops::FilterType::Triangle);
        let thumb_path = self.cover_thumb_path(book_id);
        resized
            .save_with_format(&thumb_path, ImageFormat::Png)
            .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
        Ok(())
    }

    fn clear_cover_cache(&mut self, book_id: i64) {
        self.cover_cache.remove(&book_id);
        self.cover_preview_cache.remove(&book_id);
    }

    fn cover_path(&self, book_id: i64) -> PathBuf {
        self.cover_dir.join(format!("cover-{book_id}.png"))
    }

    fn cover_thumb_path(&self, book_id: i64) -> PathBuf {
        self.cover_cache_dir
            .join(format!("cover-{book_id}-thumb.png"))
    }

    fn open_reader(&mut self, book_id: i64) -> CoreResult<()> {
        let Some(book) = self.db.get_book(book_id)? else {
            return Err(CoreError::ConfigValidate("book not found".to_string()));
        };
        let assets = self.db.list_assets_for_book(book_id)?;
        let Some(asset) = choose_asset(&assets) else {
            return Err(CoreError::ConfigValidate("no assets available".to_string()));
        };
        let (input_path, temp_path) = resolve_asset_input_path(asset, &self.tmp_dir)?;
        let format = asset_format(asset, &book.format).unwrap_or_else(|| book.format.clone());
        self.reader
            .open_book(book_id, &book.title, &format, &input_path, temp_path);
        if let Some(progress) = self.reader_progress.get(&book_id).copied() {
            self.reader.page = progress.min(self.reader.page_count().saturating_sub(1));
        }
        Ok(())
    }

    fn run_add_books(&mut self, config: &ControlPlane) -> CoreResult<()> {
        let paths = self.collect_ingest_paths(config)?;
        if paths.is_empty() {
            return Err(CoreError::ConfigValidate(
                "no files selected for ingest".to_string(),
            ));
        }
        let store = LocalAssetStore::from_config(config);
        let ingestor = Ingestor::new(std::sync::Arc::new(store), config.clone());
        let mut added = 0;
        let mut skipped = 0;
        for path in paths {
            let is_archive = is_archive_path(&path, &config.formats);
            let outcome = if is_archive && self.add_books.archive_reference {
                ingestor.ingest_archive_reference(IngestRequest {
                    source_path: &path,
                    mode: Some(self.add_books.mode),
                })?
            } else {
                ingestor.ingest(IngestRequest {
                    source_path: &path,
                    mode: Some(self.add_books.mode),
                })?
            };
            match outcome {
                IngestOutcome::Ingested(result) => {
                    let id = self.insert_ingested_book(&result)?;
                    info!(
                        component = "gui",
                        book_id = id,
                        path = %path.display(),
                        "book ingested"
                    );
                    added += 1;
                }
                IngestOutcome::Skipped(skip) => {
                    warn!(
                        component = "gui",
                        path = %path.display(),
                        reason = ?skip.reason,
                        "skipped ingest duplicate"
                    );
                    skipped += 1;
                }
            }
        }
        self.needs_refresh = true;
        self.status = format!("Added {added} book(s), skipped {skipped}");
        let status = self.status.clone();
        self.push_toast(&status, ToastLevel::Info);
        Ok(())
    }

    fn run_remove_books(&mut self, _config: &ControlPlane) -> CoreResult<()> {
        let ids = self.selected_ids.clone();
        if ids.is_empty() {
            return Ok(());
        }
        let mut removed = 0;
        let mut files_removed = 0;
        for book_id in ids {
            let assets = self.db.list_assets_for_book(book_id)?;
            if self.remove_books.delete_files {
                for asset in &assets {
                    if should_delete_asset(asset, self.remove_books.delete_reference_files) {
                        let path = Path::new(&asset.stored_path);
                        if path.exists() {
                            fs::remove_file(path).map_err(|err| {
                                CoreError::Io("remove asset file".to_string(), err)
                            })?;
                            files_removed += 1;
                        }
                    }
                }
            }
            let summary = self.db.delete_book_with_assets(book_id)?;
            if summary.book_deleted {
                removed += 1;
            }
        }
        self.selected_ids.clear();
        self.details = None;
        self.needs_refresh = true;
        self.status = format!("Removed {removed} book(s), deleted {files_removed} file(s)");
        let status = self.status.clone();
        self.push_toast(&status, ToastLevel::Info);
        Ok(())
    }

    fn run_remove_asset(
        &mut self,
        _config: &ControlPlane,
        asset: &AssetRow,
        delete_files: bool,
    ) -> CoreResult<()> {
        if delete_files
            && should_delete_asset(asset, self.remove_asset_dialog.delete_reference_files)
        {
            let path = Path::new(&asset.stored_path);
            if path.exists() {
                fs::remove_file(path)
                    .map_err(|err| CoreError::Io("remove asset file".to_string(), err))?;
            }
        }
        let deleted = self.db.delete_assets(&[asset.id])?;
        if deleted == 0 {
            return Err(CoreError::ConfigValidate("asset not found".to_string()));
        }
        self.needs_refresh = true;
        Ok(())
    }

    fn add_note_for_book(&mut self, book_id: i64) -> CoreResult<()> {
        let text = self.note_input.trim();
        if text.is_empty() {
            return Err(CoreError::ConfigValidate(
                "note text is required".to_string(),
            ));
        }
        let created_at = now_timestamp()?;
        self.db.add_note(book_id, text, &created_at)?;
        self.note_input.clear();
        Ok(())
    }

    fn run_bulk_edit(&mut self, _config: &ControlPlane) -> CoreResult<()> {
        let ids = self.selected_ids.clone();
        if ids.is_empty() {
            return Ok(());
        }
        let tags = parse_list(&self.bulk_edit.tags);
        let languages = parse_list(&self.bulk_edit.languages);
        for book_id in ids {
            if self.bulk_edit.apply_tags {
                if self.bulk_edit.replace_tags {
                    self.db.replace_book_tags(book_id, &tags)?;
                } else {
                    self.db.add_book_tags(book_id, &tags)?;
                }
            }
            if self.bulk_edit.apply_series {
                if self.bulk_edit.series_name.trim().is_empty() {
                    self.db.clear_book_series(book_id)?;
                } else {
                    self.db.set_book_series(
                        book_id,
                        self.bulk_edit.series_name.trim(),
                        self.bulk_edit.series_index,
                    )?;
                }
            }
            if self.bulk_edit.apply_publisher {
                if self.bulk_edit.clear_publisher {
                    self.db.set_book_publisher(book_id, "")?;
                } else if !self.bulk_edit.publisher.trim().is_empty() {
                    self.db
                        .set_book_publisher(book_id, self.bulk_edit.publisher.trim())?;
                }
            }
            if self.bulk_edit.apply_languages {
                self.db.set_book_languages(book_id, &languages)?;
            }
            if self.bulk_edit.apply_rating {
                self.db
                    .set_book_rating(book_id, self.bulk_edit.rating as i64)?;
            }
        }
        self.needs_refresh = true;
        self.status = "Bulk edit applied".to_string();
        let status = self.status.clone();
        self.push_toast(&status, ToastLevel::Info);
        Ok(())
    }

    fn run_convert_books(&mut self, config: &ControlPlane) -> CoreResult<()> {
        let ids = self.selected_ids.clone();
        if ids.is_empty() {
            return Ok(());
        }
        let output_dir = output_dir_or_default(
            &self.convert_books.output_dir,
            &config.conversion.output_dir,
        );
        ensure_dir(&output_dir)?;
        let mut converted = 0;
        for book_id in ids {
            let Some(book) = self.db.get_book(book_id)? else {
                continue;
            };
            let assets = self.db.list_assets_for_book(book_id)?;
            let Some(asset) = choose_asset(&assets) else {
                continue;
            };
            let (input_path, temp_input) = resolve_asset_input_path(asset, &config.paths.tmp_dir)?;
            let input_format = asset_format(asset, &book.format);
            let output_path = build_output_path(
                &output_dir,
                &book.title,
                book_id,
                &self.convert_books.output_format,
            );
            let settings = ConversionSettings::from_config(&config.conversion)
                .with_input_format(input_format)
                .with_output_format(Some(self.convert_books.output_format.clone()));
            let _report = convert_file(&input_path, &output_path, &settings)?;
            if self.convert_books.add_to_library {
                match LocalAssetStore::from_config(config).store(&output_path, StorageMode::Copy)? {
                    caliberate_assets::storage::StoreOutcome::Stored(asset_record) => {
                        let created_at = now_timestamp()?;
                        let storage_mode = match asset_record.storage_mode {
                            StorageMode::Copy => "copy",
                            StorageMode::Reference => "reference",
                        };
                        let _asset_id = self.db.add_asset(
                            book_id,
                            storage_mode,
                            &asset_record.stored_path.display().to_string(),
                            asset_record
                                .source_path
                                .as_ref()
                                .map(|path| path.display().to_string())
                                .as_deref(),
                            asset_record.size_bytes,
                            asset_record.stored_size_bytes,
                            asset_record.checksum.as_deref(),
                            asset_record.is_compressed,
                            &created_at,
                        )?;
                    }
                    caliberate_assets::storage::StoreOutcome::Skipped(skip) => {
                        warn!(
                            component = "gui",
                            path = %skip.existing_path.display(),
                            "skipped storing converted asset"
                        );
                    }
                }
            }
            if !self.convert_books.keep_output {
                let _ = fs::remove_file(&output_path);
            }
            if let Some(temp_path) = temp_input {
                let _ = fs::remove_file(temp_path);
            }
            converted += 1;
        }
        self.needs_refresh = true;
        self.status = format!("Converted {converted} book(s)");
        let status = self.status.clone();
        self.push_toast(&status, ToastLevel::Info);
        Ok(())
    }

    fn run_save_to_disk(&mut self, config: &ControlPlane) -> CoreResult<()> {
        let ids = self.selected_ids.clone();
        if ids.is_empty() {
            return Ok(());
        }
        let output_dir =
            output_dir_or_default(&self.save_to_disk.output_dir, &config.conversion.output_dir);
        ensure_dir(&output_dir)?;
        let mut exported = 0;
        for book_id in ids {
            let Some(book) = self.db.get_book(book_id)? else {
                continue;
            };
            let assets = self.db.list_assets_for_book(book_id)?;
            let assets = if self.save_to_disk.export_all_formats {
                assets
            } else {
                choose_asset(&assets)
                    .map(|asset| vec![asset.clone()])
                    .unwrap_or_default()
            };
            for asset in assets {
                let format =
                    asset_format(&asset, &book.format).unwrap_or_else(|| book.format.clone());
                let dest = build_output_path(&output_dir, &book.title, book_id, &format);
                let (input_path, temp_input) =
                    resolve_asset_input_path(&asset, &config.paths.tmp_dir)?;
                if asset.is_compressed {
                    fs::copy(&input_path, &dest).map_err(|err| {
                        CoreError::Io("write decompressed export".to_string(), err)
                    })?;
                } else {
                    fs::copy(&input_path, &dest)
                        .map_err(|err| CoreError::Io("copy export".to_string(), err))?;
                }
                if let Some(temp_path) = temp_input {
                    let _ = fs::remove_file(temp_path);
                }
                exported += 1;
            }
        }
        self.status = format!("Exported {exported} file(s)");
        let status = self.status.clone();
        self.push_toast(&status, ToastLevel::Info);
        Ok(())
    }

    fn run_device_sync(&mut self, config: &ControlPlane) -> CoreResult<()> {
        let ids = self.selected_ids.clone();
        if ids.is_empty() {
            return Ok(());
        }
        let Some(device_index) = self.device_sync.selected_device else {
            return Err(CoreError::ConfigValidate("no device selected".to_string()));
        };
        let device = self
            .device_sync
            .devices
            .get(device_index)
            .ok_or_else(|| CoreError::ConfigValidate("device selection invalid".to_string()))?
            .clone();
        let mut sent = 0;
        for book_id in ids {
            let Some(book) = self.db.get_book(book_id)? else {
                continue;
            };
            let assets = self.db.list_assets_for_book(book_id)?;
            let Some(asset) = choose_asset(&assets) else {
                continue;
            };
            let (input_path, temp_input) = resolve_asset_input_path(asset, &config.paths.tmp_dir)?;
            let format = asset_format(asset, &book.format).unwrap_or_else(|| book.format.clone());
            let dest_name = if self.device_sync.destination_name.trim().is_empty() {
                Some(build_output_name(&book.title, book_id, &format))
            } else {
                Some(self.device_sync.destination_name.trim().to_string())
            };
            let _result = send_to_device(&input_path, &device, dest_name.as_deref())?;
            if let Some(temp_path) = temp_input {
                let _ = fs::remove_file(temp_path);
            }
            sent += 1;
        }
        self.status = format!("Sent {sent} file(s) to device {}", device.name);
        let status = self.status.clone();
        self.push_toast(&status, ToastLevel::Info);
        Ok(())
    }

    fn refresh_manage_tags(&mut self) -> CoreResult<()> {
        self.manage_tags.tags = self.db.list_tag_categories()?;
        self.manage_tags.needs_refresh = false;
        Ok(())
    }

    fn refresh_manage_series(&mut self) -> CoreResult<()> {
        self.manage_series.series = self.db.list_series_categories()?;
        self.manage_series.needs_refresh = false;
        Ok(())
    }

    fn refresh_manage_custom_columns(&mut self) -> CoreResult<()> {
        self.manage_custom_columns.columns = self.db.list_custom_columns()?;
        self.manage_custom_columns.needs_refresh = false;
        Ok(())
    }

    fn refresh_manage_virtual_libraries(&mut self) -> CoreResult<()> {
        let searches = self.db.list_saved_searches()?;
        self.manage_virtual_libraries.searches = searches.into_iter().collect();
        self.manage_virtual_libraries.needs_refresh = false;
        Ok(())
    }

    fn collect_ingest_paths(&self, config: &ControlPlane) -> CoreResult<Vec<PathBuf>> {
        let mut paths: Vec<PathBuf> = Vec::new();
        for line in self.add_books.files_input.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            paths.push(PathBuf::from(trimmed));
        }
        if !self.add_books.folder_input.trim().is_empty() {
            let folder = PathBuf::from(self.add_books.folder_input.trim());
            if folder.is_dir() {
                for entry in WalkDir::new(&folder).into_iter().flatten() {
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }
                    if !is_supported_path(path, &config.formats, self.add_books.include_archives) {
                        continue;
                    }
                    paths.push(path.to_path_buf());
                }
            }
        }
        Ok(paths)
    }

    fn insert_ingested_book(
        &mut self,
        result: &caliberate_library::ingest::IngestResult,
    ) -> CoreResult<i64> {
        let created_at = now_timestamp()?;
        let id = self.db.add_book(
            &result.metadata.title,
            &result.metadata.format,
            &result.asset.stored_path.display().to_string(),
            &created_at,
        )?;
        let storage_mode = match result.asset.storage_mode {
            StorageMode::Copy => "copy",
            StorageMode::Reference => "reference",
        };
        let _asset_id = self.db.add_asset(
            id,
            storage_mode,
            &result.asset.stored_path.display().to_string(),
            result
                .asset
                .source_path
                .as_ref()
                .map(|path| path.display().to_string())
                .as_deref(),
            result.asset.size_bytes,
            result.asset.stored_size_bytes,
            result.asset.checksum.as_deref(),
            result.asset.is_compressed,
            &created_at,
        )?;
        self.db.add_book_authors(id, &result.metadata.authors)?;
        self.db.add_book_tags(id, &result.metadata.tags)?;
        if let Some(series) = &result.metadata.series {
            self.db.set_book_series(id, &series.name, series.index)?;
        }
        self.db
            .add_book_identifiers(id, &result.metadata.identifiers)?;
        if let Some(comment) = &result.metadata.comment {
            self.db.set_book_comment(id, comment)?;
        }
        Ok(id)
    }

    fn refresh_books(&mut self) -> CoreResult<()> {
        self.cache.refresh_books(&self.db)?;
        let query = self.search_query.trim().to_string();
        if self.search_commit_requested {
            record_search_history(&mut self.search_history, self.search_history_max, &query);
            self.search_commit_requested = false;
        }
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
        self.refresh_browser()?;
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

    fn refresh_browser(&mut self) -> CoreResult<()> {
        self.browser_authors = self.db.list_author_categories()?;
        self.browser_tags = self.db.list_tag_categories()?;
        self.browser_series = self.db.list_series_categories()?;
        self.browser_publishers = self.db.list_publisher_categories()?;
        self.browser_ratings = self.db.list_rating_categories()?;
        self.browser_languages = self.db.list_language_categories()?;
        let searches = self.db.list_saved_searches()?;
        self.browser_saved_searches = searches.into_iter().collect();
        if let Some(active) = &self.active_virtual_library {
            if !self
                .browser_saved_searches
                .iter()
                .any(|(name, _)| name == active)
            {
                self.active_virtual_library = None;
                self.browser_filters.clear();
            }
        }
        Ok(())
    }

    fn build_row(&mut self, book: &BookRecord) -> CoreResult<BookRow> {
        let details = self.cache.get_book_details(&self.db, book.id)?.cloned();
        let (authors, tags, series, rating, publisher, languages, has_cover) =
            if let Some(details) = details {
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
                    details.extras.has_cover,
                )
            } else {
                (
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    false,
                )
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
            has_cover,
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
        let notes = self.db.list_notes_for_book(id)?;
        let extras = self.db.get_book_extras(id)?;
        self.details = Some(BookDetails {
            book,
            assets,
            authors,
            tags,
            series,
            identifiers,
            comment,
            notes,
            extras,
        });
        self.edit = EditState::from_details(self.details.as_ref().expect("details"));
        self.edit_mode = false;
        self.note_input.clear();
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
                let format_matches = if let Some(format) = &self.format_filter {
                    book.format.eq_ignore_ascii_case(format)
                } else {
                    true
                };
                let browser_matches = self.browser_filters.iter().all(|filter| {
                    let needle = filter.value.to_lowercase();
                    let matched = match filter.category {
                        BrowserCategory::Authors => field_contains(&book.authors, &needle),
                        BrowserCategory::Tags => field_contains(&book.tags, &needle),
                        BrowserCategory::Series => field_contains(&book.series, &needle),
                        BrowserCategory::Publishers => field_contains(&book.publisher, &needle),
                        BrowserCategory::Ratings => {
                            book.rating.trim().eq_ignore_ascii_case(&needle)
                        }
                        BrowserCategory::Languages => field_contains(&book.languages, &needle),
                    };
                    match filter.mode {
                        BrowserFilterMode::Include => matched,
                        BrowserFilterMode::Exclude => !matched,
                    }
                });
                format_matches && browser_matches
            })
            .cloned()
            .collect();
        self.sort_rows(&mut list);
        self.books = list;
    }

    fn sort_rows(&mut self, list: &mut Vec<BookRow>) {
        let primary = self.sort_mode;
        let secondary = self.secondary_sort;
        let mut indexed: Vec<(usize, BookRow)> = list.drain(..).enumerate().collect();
        indexed.sort_by(|(a_idx, a), (b_idx, b)| {
            let mut cmp = compare_row(primary, a, b);
            if cmp == std::cmp::Ordering::Equal {
                if let Some(sec) = secondary {
                    cmp = compare_row(sec, a, b);
                }
            }
            if cmp == std::cmp::Ordering::Equal {
                cmp = a_idx.cmp(b_idx);
            }
            cmp
        });
        if self.sort_dir == SortDirection::Desc {
            indexed.reverse();
        }
        list.extend(indexed.into_iter().map(|(_, row)| row));
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
        config.gui.view_density = match self.view_density {
            ViewDensity::Compact => "compact".to_string(),
            ViewDensity::Comfortable => "comfortable".to_string(),
        };
        config.gui.quick_details_panel = self.quick_details_panel;
        if let Err(err) = config.save_to_path(config_path) {
            self.set_error(err);
        } else {
            self.layout_dirty = false;
            self.status = "Layout saved".to_string();
        }
    }

    fn sync_gui_runtime_config(&self, config: &mut ControlPlane) {
        config.gui.active_virtual_library = self.active_virtual_library.clone();
        config.gui.virtual_library_filters =
            encode_virtual_library_filters(&self.virtual_library_filters);
        config.gui.show_format_badges = self.show_format_badges;
        config.gui.show_language_badges = self.show_language_badges;
        config.gui.view_density = match self.view_density {
            ViewDensity::Compact => "compact".to_string(),
            ViewDensity::Comfortable => "comfortable".to_string(),
        };
        config.gui.quick_details_panel = self.quick_details_panel;
    }

    fn push_toast(&mut self, message: &str, level: ToastLevel) {
        let now = self.last_tick;
        self.toasts.push(Toast {
            message: message.to_string(),
            level,
            created_at: now,
        });
        if self.toasts.len() > self.toast_max {
            let excess = self.toasts.len() - self.toast_max;
            self.toasts.drain(0..excess);
        }
    }

    fn prune_toasts(&mut self, now: f64) {
        let duration = self.toast_duration_secs;
        self.toasts
            .retain(|toast| now - toast.created_at <= duration);
    }

    fn render_toasts(&self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let mut offset = 0.0;
        for toast in self.toasts.iter().rev() {
            let color = match toast.level {
                ToastLevel::Info => egui::Color32::from_rgb(40, 130, 200),
                ToastLevel::Warn => egui::Color32::from_rgb(200, 140, 40),
                ToastLevel::Error => egui::Color32::from_rgb(200, 60, 60),
            };
            egui::Area::new(egui::Id::new(format!("toast-{}", toast.created_at)))
                .anchor(
                    egui::Align2::RIGHT_BOTTOM,
                    egui::vec2(-16.0, -16.0 - offset),
                )
                .show(&ctx, |ui| {
                    ui.visuals_mut().window_fill = color;
                    ui.label(egui::RichText::new(&toast.message).color(egui::Color32::WHITE));
                });
            offset += 28.0;
        }
    }

    fn enqueue_job(&mut self, name: &str, now: f64) {
        let job = JobEntry {
            id: self.next_job_id,
            name: name.to_string(),
            status: JobStatus::Queued,
            progress: 0.0,
            created_at: now,
            updated_at: now,
        };
        self.next_job_id += 1;
        self.jobs.push(job);
        self.push_toast(&format!("Queued job: {name}"), ToastLevel::Info);
    }

    fn tick_jobs(&mut self, now: f64) {
        if self.last_tick == 0.0 {
            self.last_tick = now;
            return;
        }
        let delta = now - self.last_tick;
        self.last_tick = now;
        let mut completed: Vec<String> = Vec::new();
        for job in &mut self.jobs {
            match job.status {
                JobStatus::Queued => {
                    job.status = JobStatus::Running;
                    job.updated_at = now;
                }
                JobStatus::Running => {
                    job.progress = (job.progress + (delta as f32 * 0.08)).min(1.0);
                    job.updated_at = now;
                    if job.progress >= 1.0 {
                        job.status = JobStatus::Completed;
                        completed.push(job.name.clone());
                    }
                }
                _ => {}
            }
        }
        for name in completed {
            self.push_toast(&format!("Job completed: {name}"), ToastLevel::Info);
        }
    }

    fn render_jobs(&mut self, ui: &mut egui::Ui) {
        if self.jobs.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("jobs_panel"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, 16.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.heading("Jobs");
                    ui.separator();
                    let mut toasts: Vec<(String, ToastLevel)> = Vec::new();
                    for job in &mut self.jobs {
                        ui.horizontal(|ui| {
                            ui.label(format!("#{} {}", job.id, job.name));
                            ui.label(job.status.label());
                        });
                        ui.add(
                            egui::ProgressBar::new(job.progress)
                                .show_percentage()
                                .animate(true),
                        );
                        ui.horizontal(|ui| {
                            if matches!(job.status, JobStatus::Running) {
                                if ui.button("Pause").clicked() {
                                    job.status = JobStatus::Paused;
                                    toasts
                                        .push((format!("Paused job {}", job.id), ToastLevel::Warn));
                                }
                            } else if matches!(job.status, JobStatus::Paused) {
                                if ui.button("Resume").clicked() {
                                    job.status = JobStatus::Running;
                                    toasts.push((
                                        format!("Resumed job {}", job.id),
                                        ToastLevel::Info,
                                    ));
                                }
                            }
                            if !matches!(job.status, JobStatus::Completed | JobStatus::Cancelled) {
                                if ui.button("Cancel").clicked() {
                                    job.status = JobStatus::Cancelled;
                                    toasts.push((
                                        format!("Cancelled job {}", job.id),
                                        ToastLevel::Warn,
                                    ));
                                }
                            }
                        });
                        ui.separator();
                    }
                    for (message, level) in toasts {
                        self.push_toast(&message, level);
                    }
                });
            });
    }

    fn sync_cover_config(&mut self, config: &ControlPlane) {
        let mut dirty = false;
        if (config.gui.cover_thumb_size - self.last_cover_thumb_size).abs() > f32::EPSILON {
            self.cover_thumb_size = config.gui.cover_thumb_size;
            self.last_cover_thumb_size = config.gui.cover_thumb_size;
            dirty = true;
        }
        if (config.gui.cover_preview_size - self.last_cover_preview_size).abs() > f32::EPSILON {
            self.cover_preview_size = config.gui.cover_preview_size;
            self.last_cover_preview_size = config.gui.cover_preview_size;
            dirty = true;
        }
        if dirty {
            self.cover_cache.clear();
            self.cover_preview_cache.clear();
        }
        if self.cover_dir != config.gui.cover_dir {
            self.cover_dir = config.gui.cover_dir.clone();
            self.cover_cache.clear();
            self.cover_preview_cache.clear();
        }
        if self.cover_cache_dir != config.gui.cover_cache_dir {
            self.cover_cache_dir = config.gui.cover_cache_dir.clone();
            self.cover_cache.clear();
            self.cover_preview_cache.clear();
        }
        self.cover_max_bytes = config.gui.cover_max_bytes;
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

#[derive(Debug, Clone)]
struct CoverDialogState {
    cover_path_input: String,
}

impl Default for CoverDialogState {
    fn default() -> Self {
        Self {
            cover_path_input: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct ReaderState {
    open: bool,
    book_id: Option<i64>,
    title: String,
    format: String,
    content: ReaderContent,
    page: usize,
    font_size: f32,
    line_spacing: f32,
    page_chars: usize,
    theme: ReaderTheme,
    search_query: String,
    last_match: Option<usize>,
    recent: Vec<ReaderRecent>,
    temp_path: Option<PathBuf>,
    error: Option<String>,
}

impl ReaderState {
    fn from_config(config: &ControlPlane) -> Self {
        Self {
            open: false,
            book_id: None,
            title: String::new(),
            format: String::new(),
            content: ReaderContent::Empty,
            page: 0,
            font_size: config.gui.reader_font_size,
            line_spacing: config.gui.reader_line_spacing,
            page_chars: config.gui.reader_page_chars,
            theme: ReaderTheme::from_config(&config.gui.reader_theme),
            search_query: String::new(),
            last_match: None,
            recent: Vec::new(),
            temp_path: None,
            error: None,
        }
    }

    fn open_book(
        &mut self,
        book_id: i64,
        title: &str,
        format: &str,
        path: &Path,
        temp_path: Option<PathBuf>,
    ) {
        if let Some(path) = self.temp_path.take() {
            let _ = fs::remove_file(path);
        }
        self.book_id = Some(book_id);
        self.title = title.to_string();
        self.format = format.to_string();
        self.page = 0;
        self.error = None;
        self.search_query.clear();
        self.last_match = None;
        self.temp_path = temp_path;
        self.content =
            ReaderContent::from_path(path, format, self.page_chars).unwrap_or_else(|err| {
                self.error = Some(err);
                ReaderContent::Unsupported
            });
        self.open = true;
        self.push_recent(book_id, title, self.page);
    }

    fn close(&mut self) {
        self.book_id = None;
        self.title.clear();
        self.format.clear();
        self.page = 0;
        self.error = None;
        self.content = ReaderContent::Empty;
        if let Some(path) = self.temp_path.take() {
            let _ = fs::remove_file(path);
        }
    }

    fn page_count(&self) -> usize {
        match &self.content {
            ReaderContent::Text { pages, .. } => pages.len(),
            ReaderContent::Unsupported | ReaderContent::Empty => 0,
        }
    }

    fn next_page(&mut self) {
        let count = self.page_count();
        if count == 0 {
            return;
        }
        self.page = (self.page + 1).min(count - 1);
        if let Some(book_id) = self.book_id {
            let title = self.title.clone();
            let page = self.page;
            self.push_recent(book_id, &title, page);
        }
    }

    fn prev_page(&mut self) {
        if self.page > 0 {
            self.page -= 1;
            if let Some(book_id) = self.book_id {
                let title = self.title.clone();
                let page = self.page;
                self.push_recent(book_id, &title, page);
            }
        }
    }

    fn update_page_chars(&mut self, page_chars: usize) {
        if page_chars == 0 || page_chars == self.page_chars {
            return;
        }
        self.page_chars = page_chars;
        if let ReaderContent::Text { raw, pages } = &mut self.content {
            *pages = paginate_text(raw, page_chars);
            if self.page >= pages.len() {
                self.page = pages.len().saturating_sub(1);
            }
        }
    }

    fn find_next(&mut self) {
        let query = self.search_query.trim().to_lowercase();
        if query.is_empty() {
            self.last_match = None;
            return;
        }
        if let ReaderContent::Text { pages, .. } = &self.content {
            let start = self.last_match.unwrap_or(0);
            for idx in start..pages.len() {
                if pages[idx].to_lowercase().contains(&query) {
                    self.page = idx;
                    self.last_match = Some(idx + 1);
                    return;
                }
            }
            self.last_match = Some(0);
        }
    }

    fn jump_to(&mut self, book_id: i64, page: usize) {
        if self.book_id == Some(book_id) {
            self.page = page.min(self.page_count().saturating_sub(1));
        }
    }

    fn push_recent(&mut self, book_id: i64, title: &str, page: usize) {
        self.recent.retain(|entry| entry.book_id != book_id);
        self.recent.insert(
            0,
            ReaderRecent {
                book_id,
                title: title.to_string(),
                page,
            },
        );
        self.recent.truncate(10);
    }

    fn render(&self, ui: &mut egui::Ui) {
        match &self.content {
            ReaderContent::Text { pages, .. } => {
                let raw_text = pages.get(self.page).map(|s| s.as_str()).unwrap_or("");
                let page_text = if self.line_spacing > 1.3 {
                    raw_text.replace('\n', "\n\n")
                } else {
                    raw_text.to_string()
                };
                render_text_with_highlight(ui, &page_text, &self.search_query, self.font_size);
            }
            ReaderContent::Unsupported => {
                ui.label("Preview not available for this format.");
            }
            ReaderContent::Empty => {
                ui.label("No content loaded.");
            }
        }
    }
}

#[derive(Debug, Clone)]
enum ReaderContent {
    Text { raw: String, pages: Vec<String> },
    Unsupported,
    Empty,
}

impl ReaderContent {
    fn from_path(path: &Path, format: &str, page_chars: usize) -> Result<Self, String> {
        match format {
            "txt" | "md" | "markdown" => {
                let raw = fs::read_to_string(path)
                    .map_err(|err| format!("read reader content: {err}"))?;
                Ok(ReaderContent::Text {
                    pages: paginate_text(&raw, page_chars),
                    raw,
                })
            }
            _ => Ok(ReaderContent::Unsupported),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReaderTheme {
    Light,
    Dark,
    Sepia,
}

#[derive(Debug, Clone)]
struct ReaderRecent {
    book_id: i64,
    title: String,
    page: usize,
}

impl ReaderTheme {
    fn from_config(value: &str) -> Self {
        match value {
            "dark" => ReaderTheme::Dark,
            "sepia" => ReaderTheme::Sepia,
            _ => ReaderTheme::Light,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            ReaderTheme::Light => "Light",
            ReaderTheme::Dark => "Dark",
            ReaderTheme::Sepia => "Sepia",
        }
    }

    fn background(&self) -> egui::Color32 {
        match self {
            ReaderTheme::Light => egui::Color32::from_rgb(245, 245, 245),
            ReaderTheme::Dark => egui::Color32::from_rgb(25, 25, 25),
            ReaderTheme::Sepia => egui::Color32::from_rgb(240, 230, 210),
        }
    }

    fn text_color(&self) -> egui::Color32 {
        match self {
            ReaderTheme::Light => egui::Color32::from_rgb(30, 30, 30),
            ReaderTheme::Dark => egui::Color32::from_rgb(230, 230, 230),
            ReaderTheme::Sepia => egui::Color32::from_rgb(50, 40, 30),
        }
    }
}

#[derive(Debug, Clone)]
struct AddBooksDialogState {
    open: bool,
    files_input: String,
    folder_input: String,
    mode: IngestMode,
    archive_reference: bool,
    include_archives: bool,
}

impl Default for AddBooksDialogState {
    fn default() -> Self {
        Self {
            open: false,
            files_input: String::new(),
            folder_input: String::new(),
            mode: IngestMode::Copy,
            archive_reference: false,
            include_archives: true,
        }
    }
}

impl AddBooksDialogState {
    fn apply_defaults(&mut self, config: &ControlPlane) {
        self.mode = config.ingest.default_mode;
        self.archive_reference = config.ingest.archive_reference_enabled;
        if self.output_fields_empty() {
            self.files_input.clear();
            self.folder_input.clear();
        }
    }

    fn output_fields_empty(&self) -> bool {
        self.files_input.trim().is_empty() && self.folder_input.trim().is_empty()
    }
}

#[derive(Debug, Clone)]
struct RemoveBooksDialogState {
    open: bool,
    delete_files: bool,
    delete_reference_files: bool,
}

impl Default for RemoveBooksDialogState {
    fn default() -> Self {
        Self {
            open: false,
            delete_files: false,
            delete_reference_files: false,
        }
    }
}

impl RemoveBooksDialogState {
    fn apply_defaults(&mut self, config: &ControlPlane) {
        self.delete_files = config.library.delete_files_on_remove;
        self.delete_reference_files = config.library.delete_reference_files;
    }
}

#[derive(Debug, Clone)]
struct RemoveAssetDialogState {
    open: bool,
    asset: Option<AssetRow>,
    delete_files: bool,
    delete_reference_files: bool,
}

impl Default for RemoveAssetDialogState {
    fn default() -> Self {
        Self {
            open: false,
            asset: None,
            delete_files: false,
            delete_reference_files: false,
        }
    }
}

impl RemoveAssetDialogState {
    fn apply_defaults(&mut self, config: &ControlPlane, asset: &AssetRow) {
        self.open = true;
        self.asset = Some(asset.clone());
        self.delete_files = config.library.delete_files_on_remove;
        self.delete_reference_files = config.library.delete_reference_files;
    }
}

#[derive(Debug, Clone)]
struct BulkEditDialogState {
    open: bool,
    apply_tags: bool,
    replace_tags: bool,
    tags: String,
    apply_series: bool,
    series_name: String,
    series_index: f64,
    apply_publisher: bool,
    publisher: String,
    clear_publisher: bool,
    apply_languages: bool,
    languages: String,
    apply_rating: bool,
    rating: i64,
}

impl Default for BulkEditDialogState {
    fn default() -> Self {
        Self {
            open: false,
            apply_tags: false,
            replace_tags: false,
            tags: String::new(),
            apply_series: false,
            series_name: String::new(),
            series_index: 1.0,
            apply_publisher: false,
            publisher: String::new(),
            clear_publisher: false,
            apply_languages: false,
            languages: String::new(),
            apply_rating: false,
            rating: 0,
        }
    }
}

impl BulkEditDialogState {
    fn reset(&mut self) {
        *self = Self::default();
        self.open = true;
    }
}

#[derive(Debug, Clone)]
struct ConvertBooksDialogState {
    open: bool,
    output_format: String,
    output_dir: String,
    add_to_library: bool,
    keep_output: bool,
}

impl Default for ConvertBooksDialogState {
    fn default() -> Self {
        Self {
            open: false,
            output_format: "epub".to_string(),
            output_dir: String::new(),
            add_to_library: false,
            keep_output: true,
        }
    }
}

impl ConvertBooksDialogState {
    fn apply_defaults(&mut self, config: &ControlPlane) {
        self.output_format = config.conversion.default_output_format.clone();
        self.output_dir = config.conversion.output_dir.display().to_string();
        self.add_to_library = false;
        self.keep_output = true;
    }
}

#[derive(Debug, Clone)]
struct SaveToDiskDialogState {
    open: bool,
    output_dir: String,
    export_all_formats: bool,
}

impl Default for SaveToDiskDialogState {
    fn default() -> Self {
        Self {
            open: false,
            output_dir: String::new(),
            export_all_formats: true,
        }
    }
}

impl SaveToDiskDialogState {
    fn apply_defaults(&mut self, config: &ControlPlane) {
        self.output_dir = config.conversion.output_dir.display().to_string();
        self.export_all_formats = true;
    }
}

#[derive(Debug, Clone)]
struct DeviceSyncDialogState {
    open: bool,
    devices: Vec<DeviceInfo>,
    selected_device: Option<usize>,
    destination_name: String,
    error: Option<String>,
}

impl Default for DeviceSyncDialogState {
    fn default() -> Self {
        Self {
            open: false,
            devices: Vec::new(),
            selected_device: None,
            destination_name: String::new(),
            error: None,
        }
    }
}

impl DeviceSyncDialogState {
    fn apply_defaults(&mut self, config: &ControlPlane) {
        self.error = None;
        match detect_devices(&config.device) {
            Ok(devices) => {
                self.devices = devices;
                self.selected_device = if self.devices.is_empty() {
                    None
                } else {
                    Some(0)
                };
            }
            Err(err) => {
                self.error = Some(err.to_string());
                self.devices.clear();
                self.selected_device = None;
            }
        }
    }
}

#[derive(Debug, Clone)]
struct ManageTagsDialogState {
    open: bool,
    tags: Vec<CategoryCount>,
    rename_from: String,
    rename_to: String,
    delete_name: String,
    needs_refresh: bool,
}

impl Default for ManageTagsDialogState {
    fn default() -> Self {
        Self {
            open: false,
            tags: Vec::new(),
            rename_from: String::new(),
            rename_to: String::new(),
            delete_name: String::new(),
            needs_refresh: true,
        }
    }
}

#[derive(Debug, Clone)]
struct ManageSeriesDialogState {
    open: bool,
    series: Vec<CategoryCount>,
    rename_from: String,
    rename_to: String,
    delete_name: String,
    needs_refresh: bool,
}

impl Default for ManageSeriesDialogState {
    fn default() -> Self {
        Self {
            open: false,
            series: Vec::new(),
            rename_from: String::new(),
            rename_to: String::new(),
            delete_name: String::new(),
            needs_refresh: true,
        }
    }
}

#[derive(Debug, Clone)]
struct ManageCustomColumnsDialogState {
    open: bool,
    columns: Vec<CustomColumn>,
    new_label: String,
    new_name: String,
    new_datatype: String,
    new_display: String,
    delete_label: String,
    needs_refresh: bool,
}

impl Default for ManageCustomColumnsDialogState {
    fn default() -> Self {
        Self {
            open: false,
            columns: Vec::new(),
            new_label: String::new(),
            new_name: String::new(),
            new_datatype: "text".to_string(),
            new_display: String::new(),
            delete_label: String::new(),
            needs_refresh: true,
        }
    }
}

#[derive(Debug, Clone)]
struct ManageVirtualLibrariesDialogState {
    open: bool,
    searches: Vec<(String, String)>,
    new_name: String,
    new_query: String,
    delete_name: String,
    needs_refresh: bool,
}

impl Default for ManageVirtualLibrariesDialogState {
    fn default() -> Self {
        Self {
            open: false,
            searches: Vec::new(),
            new_name: String::new(),
            new_query: String::new(),
            delete_name: String::new(),
            needs_refresh: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetailAction {
    None,
    BeginEdit,
    Save,
    Cancel,
    SetCover,
    RemoveCover,
    GenerateCover,
    OpenReader,
}

#[derive(Debug, Clone)]
struct Toast {
    message: String,
    level: ToastLevel,
    created_at: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToastLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone)]
struct JobEntry {
    id: u64,
    name: String,
    status: JobStatus,
    progress: f32,
    created_at: f64,
    updated_at: f64,
}

#[derive(Debug, Clone)]
struct LibraryStatsSummary {
    formats: Vec<(String, usize)>,
    languages: Vec<(String, usize)>,
    tags: Vec<(String, usize)>,
    authors: Vec<(String, usize)>,
    series: Vec<(String, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JobStatus {
    Queued,
    Running,
    Paused,
    Completed,
    Cancelled,
    Failed,
}

impl JobStatus {
    fn label(&self) -> &'static str {
        match self {
            JobStatus::Queued => "Queued",
            JobStatus::Running => "Running",
            JobStatus::Paused => "Paused",
            JobStatus::Completed => "Completed",
            JobStatus::Cancelled => "Cancelled",
            JobStatus::Failed => "Failed",
        }
    }
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
            series_index: details
                .series
                .as_ref()
                .map(|series| series.index)
                .unwrap_or(1.0),
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

fn is_archive_path(path: &Path, formats: &caliberate_core::config::FormatsConfig) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .map(|ext| formats.archive_formats.iter().any(|fmt| fmt == &ext))
        .unwrap_or(false)
}

fn is_supported_path(
    path: &Path,
    formats: &caliberate_core::config::FormatsConfig,
    include_archives: bool,
) -> bool {
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    let ext = ext.to_lowercase();
    if formats.supported.iter().any(|fmt| fmt == &ext) {
        return true;
    }
    include_archives && formats.archive_formats.iter().any(|fmt| fmt == &ext)
}

fn should_delete_asset(asset: &AssetRow, delete_reference_files: bool) -> bool {
    if asset.storage_mode.eq_ignore_ascii_case("reference") {
        delete_reference_files
    } else {
        true
    }
}

fn asset_format(asset: &AssetRow, fallback: &str) -> Option<String> {
    let source_ext = asset
        .source_path
        .as_deref()
        .and_then(|path| Path::new(path).extension().and_then(|ext| ext.to_str()))
        .map(|ext| ext.to_lowercase());
    let stored_ext = Path::new(&asset.stored_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase());
    let ext = source_ext.or(stored_ext);
    match ext.as_deref() {
        Some("zst") | None => Some(fallback.to_string()),
        Some(value) => Some(value.to_string()),
    }
}

fn build_output_name(title: &str, book_id: i64, format: &str) -> String {
    let safe = sanitize_filename(title);
    format!("{safe}_{book_id}.{format}")
}

fn build_output_path(output_dir: &Path, title: &str, book_id: i64, format: &str) -> PathBuf {
    output_dir.join(build_output_name(title, book_id, format))
}

fn sanitize_filename(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "untitled".to_string();
    }
    trimmed
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn output_dir_or_default(input: &str, fallback: &Path) -> PathBuf {
    if input.trim().is_empty() {
        fallback.to_path_buf()
    } else {
        PathBuf::from(input.trim())
    }
}

fn ensure_dir(path: &Path) -> CoreResult<()> {
    fs::create_dir_all(path).map_err(|err| CoreError::Io("create output dir".to_string(), err))
}

fn resolve_asset_input_path(
    asset: &AssetRow,
    tmp_dir: &Path,
) -> CoreResult<(PathBuf, Option<PathBuf>)> {
    let stored = PathBuf::from(&asset.stored_path);
    if asset.is_compressed {
        fs::create_dir_all(tmp_dir)
            .map_err(|err| CoreError::Io("create temp dir".to_string(), err))?;
        let stem = stored
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("asset");
        let temp_path = tmp_dir.join(format!("decompressed-{}-{}", asset.id, stem));
        decompress_file(&stored, &temp_path)?;
        Ok((temp_path.clone(), Some(temp_path)))
    } else {
        Ok((stored, None))
    }
}

fn choose_asset(assets: &[AssetRow]) -> Option<&AssetRow> {
    assets
        .iter()
        .find(|asset| !asset.is_compressed)
        .or_else(|| assets.first())
}

fn now_timestamp() -> CoreResult<String> {
    let format = time::format_description::parse("[year]-[month]-[day]T[hour]:[minute]:[second]Z")
        .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
    OffsetDateTime::now_utc()
        .format(&format)
        .map_err(|err| CoreError::ConfigValidate(err.to_string()))
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

fn record_search_history(history: &mut Vec<String>, max: usize, query: &str) {
    let query = query.trim();
    if query.is_empty() || max == 0 {
        return;
    }
    if let Some(pos) = history
        .iter()
        .position(|item| item.eq_ignore_ascii_case(query))
    {
        history.remove(pos);
    }
    history.insert(0, query.to_string());
    history.truncate(max);
}

fn field_contains(haystack: &str, needle_lower: &str) -> bool {
    if needle_lower.trim().is_empty() {
        return true;
    }
    haystack.to_lowercase().contains(needle_lower)
}

fn hierarchical_category_label(category: BrowserCategory, name: &str) -> String {
    let delimiter = match category {
        BrowserCategory::Tags => Some('/'),
        BrowserCategory::Series => Some(':'),
        _ => None,
    };
    if let Some(delim) = delimiter {
        let depth = name.split(delim).count().saturating_sub(1);
        if depth > 0 {
            let indent = "  ".repeat(depth.min(6));
            return format!("{indent}{name}");
        }
    }
    name.to_string()
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

fn compute_library_stats(rows: &[BookRow]) -> LibraryStatsSummary {
    let mut formats = HashMap::<String, usize>::new();
    let mut languages = HashMap::<String, usize>::new();
    let mut tags = HashMap::<String, usize>::new();
    let mut authors = HashMap::<String, usize>::new();
    let mut series = HashMap::<String, usize>::new();
    for row in rows {
        *formats.entry(row.format.clone()).or_insert(0) += 1;
        for item in split_csv_field(&row.languages) {
            *languages.entry(item).or_insert(0) += 1;
        }
        for item in split_csv_field(&row.tags) {
            *tags.entry(item).or_insert(0) += 1;
        }
        for item in split_csv_field(&row.authors) {
            *authors.entry(item).or_insert(0) += 1;
        }
        if !row.series.trim().is_empty() {
            *series.entry(row.series.clone()).or_insert(0) += 1;
        }
    }
    LibraryStatsSummary {
        formats: sort_count_map(formats),
        languages: sort_count_map(languages),
        tags: sort_count_map(tags),
        authors: sort_count_map(authors),
        series: sort_count_map(series),
    }
}

fn sort_count_map(map: HashMap<String, usize>) -> Vec<(String, usize)> {
    let mut items = map.into_iter().collect::<Vec<_>>();
    items.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    items
}

fn split_csv_field(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn export_stats_csv(cache_dir: &Path, stats: &LibraryStatsSummary) -> CoreResult<PathBuf> {
    fs::create_dir_all(cache_dir)
        .map_err(|err| CoreError::Io("create cache dir".to_string(), err))?;
    let output = cache_dir.join("library-stats.csv");
    let mut lines = vec!["kind,name,count".to_string()];
    append_stat_lines(&mut lines, "format", &stats.formats);
    append_stat_lines(&mut lines, "language", &stats.languages);
    append_stat_lines(&mut lines, "tag", &stats.tags);
    append_stat_lines(&mut lines, "author", &stats.authors);
    append_stat_lines(&mut lines, "series", &stats.series);
    fs::write(&output, lines.join("\n"))
        .map_err(|err| CoreError::Io("write stats csv".to_string(), err))?;
    Ok(output)
}

fn append_stat_lines(lines: &mut Vec<String>, kind: &str, items: &[(String, usize)]) {
    for (name, count) in items {
        lines.push(format!("{kind},{},{}", escape_csv_cell(name), count));
    }
}

fn escape_csv_cell(value: &str) -> String {
    if value.contains(',') || value.contains('"') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn format_badge_text(format: &str) -> String {
    format!("[{}]", format.trim().to_uppercase())
}

fn language_badge_text(languages: &str) -> String {
    let parts = split_csv_field(languages);
    if parts.is_empty() {
        return String::new();
    }
    parts
        .iter()
        .map(|lang| format!("[{}]", lang.trim().to_uppercase()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn encode_filter_entry(filter: &BrowserFilter) -> String {
    let category = match filter.category {
        BrowserCategory::Authors => "authors",
        BrowserCategory::Tags => "tags",
        BrowserCategory::Series => "series",
        BrowserCategory::Publishers => "publishers",
        BrowserCategory::Ratings => "ratings",
        BrowserCategory::Languages => "languages",
    };
    let mode = match filter.mode {
        BrowserFilterMode::Include => "include",
        BrowserFilterMode::Exclude => "exclude",
    };
    format!("{category}|{mode}|{}", filter.value.replace('|', "\\|"))
}

fn decode_filter_entry(value: &str) -> Option<BrowserFilter> {
    let mut parts = value.splitn(3, '|');
    let category = parts.next()?;
    let mode = parts.next()?;
    let raw = parts.next()?.replace("\\|", "|");
    let category = match category {
        "authors" => BrowserCategory::Authors,
        "tags" => BrowserCategory::Tags,
        "series" => BrowserCategory::Series,
        "publishers" => BrowserCategory::Publishers,
        "ratings" => BrowserCategory::Ratings,
        "languages" => BrowserCategory::Languages,
        _ => return None,
    };
    let mode = match mode {
        "include" => BrowserFilterMode::Include,
        "exclude" => BrowserFilterMode::Exclude,
        _ => return None,
    };
    Some(BrowserFilter {
        category,
        value: raw,
        mode,
    })
}

fn encode_virtual_library_filters(
    source: &HashMap<String, Vec<BrowserFilter>>,
) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut out = std::collections::BTreeMap::new();
    for (name, filters) in source {
        let entries = filters.iter().map(encode_filter_entry).collect::<Vec<_>>();
        out.insert(name.clone(), entries);
    }
    out
}

fn decode_virtual_library_filters(
    source: &std::collections::BTreeMap<String, Vec<String>>,
) -> HashMap<String, Vec<BrowserFilter>> {
    let mut out = HashMap::new();
    for (name, filters) in source {
        let parsed = filters
            .iter()
            .filter_map(|entry| decode_filter_entry(entry))
            .collect::<Vec<_>>();
        out.insert(name.clone(), parsed);
    }
    out
}

fn parse_view_mode(value: &str) -> ViewMode {
    match value {
        "grid" => ViewMode::Grid,
        _ => ViewMode::Table,
    }
}

fn parse_view_density(value: &str) -> ViewDensity {
    match value {
        "compact" => ViewDensity::Compact,
        _ => ViewDensity::Comfortable,
    }
}

fn column_width_control(ui: &mut egui::Ui, label: &str, value: &mut f32, layout_dirty: &mut bool) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).range(60.0..=720.0).speed(1.0));
    });
    *layout_dirty = true;
}

fn column_with_width(width: f32) -> Column {
    Column::initial(width).resizable(true)
}

fn compare_row(mode: SortMode, a: &BookRow, b: &BookRow) -> std::cmp::Ordering {
    match mode {
        SortMode::Title => a.title.cmp(&b.title),
        SortMode::Authors => a.authors.cmp(&b.authors),
        SortMode::Series => a.series.cmp(&b.series),
        SortMode::Tags => a.tags.cmp(&b.tags),
        SortMode::Formats => a.format.cmp(&b.format),
        SortMode::Rating => a.rating.cmp(&b.rating),
        SortMode::Publisher => a.publisher.cmp(&b.publisher),
        SortMode::Languages => a.languages.cmp(&b.languages),
        SortMode::Id => a.id.cmp(&b.id),
    }
}

fn render_cover_thumbnail(
    ui: &mut egui::Ui,
    texture: Option<&egui::TextureHandle>,
    has_cover: bool,
    size: f32,
) {
    let size = egui::vec2(size, size * 1.3);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    if let Some(texture) = texture {
        ui.painter().image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        return;
    }
    let color = if has_cover {
        egui::Color32::from_rgb(80, 140, 80)
    } else {
        egui::Color32::from_rgb(80, 80, 80)
    };
    ui.painter().rect_filled(rect, 2.0, color);
    let label = if has_cover { "Cover" } else { "No Cover" };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::TextStyle::Small.resolve(ui.style()),
        egui::Color32::WHITE,
    );
}

fn render_cover_preview(
    ui: &mut egui::Ui,
    texture: Option<&egui::TextureHandle>,
    has_cover: bool,
    size: f32,
) {
    let size = egui::vec2(size, size * 1.4);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    if let Some(texture) = texture {
        ui.painter().image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        return;
    }
    let color = if has_cover {
        egui::Color32::from_rgb(90, 150, 90)
    } else {
        egui::Color32::from_rgb(90, 90, 90)
    };
    ui.painter().rect_filled(rect, 4.0, color);
    let label = if has_cover {
        "Cover Preview"
    } else {
        "No Cover"
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::TextStyle::Heading.resolve(ui.style()),
        egui::Color32::WHITE,
    );
}

fn render_markdown(ui: &mut egui::Ui, text: &str) {
    let mut job = egui::text::LayoutJob::default();
    let mut bold = false;
    let mut italic = false;
    let mut code = false;
    let mut heading_level: Option<u32> = None;
    let parser = Parser::new(text);
    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading_level = Some(level as u32);
            }
            Event::End(TagEnd::Heading(_)) => {
                heading_level = None;
                job.append(
                    "\n",
                    0.0,
                    base_text_format(ui, bold, italic, code, heading_level),
                );
            }
            Event::Start(Tag::Emphasis) => italic = true,
            Event::End(TagEnd::Emphasis) => italic = false,
            Event::Start(Tag::Strong) => bold = true,
            Event::End(TagEnd::Strong) => bold = false,
            Event::Start(Tag::CodeBlock(_)) => code = true,
            Event::End(TagEnd::CodeBlock) => {
                code = false;
                job.append(
                    "\n",
                    0.0,
                    base_text_format(ui, bold, italic, code, heading_level),
                );
            }
            Event::Start(Tag::Item) => {
                job.append(
                    "• ",
                    0.0,
                    base_text_format(ui, bold, italic, code, heading_level),
                );
            }
            Event::Text(value) => {
                job.append(
                    value.as_ref(),
                    0.0,
                    base_text_format(ui, bold, italic, code, heading_level),
                );
            }
            Event::SoftBreak | Event::HardBreak => {
                job.append(
                    "\n",
                    0.0,
                    base_text_format(ui, bold, italic, code, heading_level),
                );
            }
            _ => {}
        }
    }
    ui.label(job);
}

fn base_text_format(
    ui: &egui::Ui,
    bold: bool,
    italic: bool,
    code: bool,
    heading_level: Option<u32>,
) -> egui::text::TextFormat {
    let mut size = ui.text_style_height(&egui::TextStyle::Body);
    if let Some(level) = heading_level {
        size += (4.0_f32).max(2.0 * (3.0 - level.min(3) as f32));
    }
    if code {
        size *= 0.95;
    }
    let mut format = egui::text::TextFormat {
        font_id: egui::FontId::proportional(size),
        color: ui.visuals().text_color(),
        ..Default::default()
    };
    if bold {
        format.font_id = egui::FontId::proportional(size + 1.0);
    }
    if italic {
        format.italics = true;
    }
    if code {
        format.font_id = egui::FontId::monospace(size);
    }
    format
}

fn load_texture_from_path(ctx: &egui::Context, path: &Path) -> CoreResult<egui::TextureHandle> {
    if !path.exists() {
        return Err(CoreError::ConfigValidate(format!(
            "cover file missing: {}",
            path.display()
        )));
    }
    let image = image::open(path).map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
    let color_image = image_to_color_image(&image);
    Ok(ctx.load_texture(
        format!("cover-{}", path.display()),
        color_image,
        egui::TextureOptions::LINEAR,
    ))
}

fn image_to_color_image(image: &DynamicImage) -> egui::ColorImage {
    let rgba = image.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw())
}

fn render_text_with_highlight(ui: &mut egui::Ui, text: &str, query: &str, size: f32) {
    let mut job = egui::text::LayoutJob::default();
    if query.trim().is_empty() {
        job.append(
            text,
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::proportional(size),
                ..Default::default()
            },
        );
        ui.label(job);
        return;
    }
    let query_lower = query.to_lowercase();
    let mut remaining = text;
    while let Some(pos) = remaining.to_lowercase().find(&query_lower) {
        let (prefix, rest) = remaining.split_at(pos);
        if !prefix.is_empty() {
            job.append(
                prefix,
                0.0,
                egui::TextFormat {
                    font_id: egui::FontId::proportional(size),
                    ..Default::default()
                },
            );
        }
        let (match_text, tail) = rest.split_at(query.len().min(rest.len()));
        if !match_text.is_empty() {
            job.append(
                match_text,
                0.0,
                egui::TextFormat {
                    font_id: egui::FontId::proportional(size),
                    color: egui::Color32::from_rgb(220, 180, 60),
                    ..Default::default()
                },
            );
        }
        remaining = tail;
    }
    if !remaining.is_empty() {
        job.append(
            remaining,
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::proportional(size),
                ..Default::default()
            },
        );
    }
    ui.label(job);
}

fn render_html_fallback(ui: &mut egui::Ui, text: &str) {
    let stripped = strip_html_tags(text);
    ui.label(stripped);
}

fn strip_html_tags(text: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ => {
                if !in_tag {
                    out.push(ch);
                }
            }
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_lowercase().as_str(), "png" | "jpg" | "jpeg"))
        .unwrap_or(false)
}

fn paginate_text(text: &str, page_chars: usize) -> Vec<String> {
    if page_chars == 0 {
        return vec![text.to_string()];
    }
    let mut pages = Vec::new();
    let mut buffer = String::new();
    for ch in text.chars() {
        buffer.push(ch);
        if buffer.chars().count() >= page_chars {
            pages.push(buffer);
            buffer = String::new();
        }
    }
    if !buffer.is_empty() {
        pages.push(buffer);
    }
    if pages.is_empty() {
        pages.push(String::new());
    }
    pages
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
