//! GUI views and models.

use arboard::Clipboard;
use caliberate_assets::compression::decompress_file;
use caliberate_assets::storage::{AssetStore, LocalAssetStore, StorageMode};
use caliberate_conversion::pipeline::convert_file;
use caliberate_conversion::settings::ConversionSettings;
use caliberate_core::config::{ControlPlane, GuiConfig, IngestMode, MetadataDownloadConfig};
use caliberate_core::error::{CoreError, CoreResult};
use caliberate_db::cache::MetadataCache;
use caliberate_db::database::{
    AssetRow, BookRecord, CategoryCount, CustomColumn, Database, IdentifierEntry, NoteRecord,
    SeriesEntry,
};
use caliberate_device::detection::{DeviceInfo, detect_devices};
use caliberate_device::sync::{cleanup_device_orphans, list_device_entries, send_to_device};
use caliberate_library::ingest::{IngestOutcome, IngestRequest, Ingestor};
use caliberate_metadata::online::{
    DownloadedMetadata, MetadataQuery, ProviderConfig, fetch_cover, fetch_metadata,
};
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use image::{DynamicImage, ImageFormat};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::collections::{BTreeSet, HashMap, HashSet};
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
    pub date_added: String,
    pub date_modified: String,
    pub pubdate: String,
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
    Shelf,
}

impl ViewMode {
    fn preset_scope_key(self) -> &'static str {
        match self {
            ViewMode::Table => "table",
            ViewMode::Grid => "grid",
            ViewMode::Shelf => "shelf",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColumnPresetScope {
    CurrentView,
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupMode {
    None,
    Series,
    Authors,
    Tags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchScope {
    All,
    Title,
    Authors,
    Tags,
    Series,
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
    DateAdded,
    DateModified,
    PubDate,
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
            SortMode::DateAdded => "Added",
            SortMode::DateModified => "Modified",
            SortMode::PubDate => "Pubdate",
            SortMode::Id => "ID",
        }
    }
}

impl SortMode {
    fn key(self) -> &'static str {
        match self {
            SortMode::Title => "title",
            SortMode::Authors => "authors",
            SortMode::Series => "series",
            SortMode::Tags => "tags",
            SortMode::Formats => "formats",
            SortMode::Rating => "rating",
            SortMode::Publisher => "publisher",
            SortMode::Languages => "languages",
            SortMode::DateAdded => "date_added",
            SortMode::DateModified => "date_modified",
            SortMode::PubDate => "pubdate",
            SortMode::Id => "id",
        }
    }
}

fn parse_sort_mode(value: &str) -> Option<SortMode> {
    match value {
        "title" => Some(SortMode::Title),
        "authors" => Some(SortMode::Authors),
        "series" => Some(SortMode::Series),
        "tags" => Some(SortMode::Tags),
        "formats" => Some(SortMode::Formats),
        "rating" => Some(SortMode::Rating),
        "publisher" => Some(SortMode::Publisher),
        "languages" => Some(SortMode::Languages),
        "date_added" => Some(SortMode::DateAdded),
        "date_modified" => Some(SortMode::DateModified),
        "pubdate" => Some(SortMode::PubDate),
        "id" => Some(SortMode::Id),
        _ => None,
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
    date_added: bool,
    date_modified: bool,
    pubdate: bool,
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
    date_added: f32,
    date_modified: f32,
    pubdate: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ColumnKey {
    Title,
    Cover,
    Authors,
    Series,
    Tags,
    Formats,
    Rating,
    Publisher,
    Languages,
    DateAdded,
    DateModified,
    PubDate,
}

impl ColumnKey {
    fn label(self) -> &'static str {
        match self {
            ColumnKey::Title => "Title",
            ColumnKey::Cover => "Cover",
            ColumnKey::Authors => "Authors",
            ColumnKey::Series => "Series",
            ColumnKey::Tags => "Tags",
            ColumnKey::Formats => "Formats",
            ColumnKey::Rating => "Rating",
            ColumnKey::Publisher => "Publisher",
            ColumnKey::Languages => "Languages",
            ColumnKey::DateAdded => "Added",
            ColumnKey::DateModified => "Modified",
            ColumnKey::PubDate => "Pubdate",
        }
    }

    fn key(self) -> &'static str {
        match self {
            ColumnKey::Title => "title",
            ColumnKey::Cover => "cover",
            ColumnKey::Authors => "authors",
            ColumnKey::Series => "series",
            ColumnKey::Tags => "tags",
            ColumnKey::Formats => "formats",
            ColumnKey::Rating => "rating",
            ColumnKey::Publisher => "publisher",
            ColumnKey::Languages => "languages",
            ColumnKey::DateAdded => "date_added",
            ColumnKey::DateModified => "date_modified",
            ColumnKey::PubDate => "pubdate",
        }
    }

    fn sort_mode(self) -> Option<SortMode> {
        match self {
            ColumnKey::Title => Some(SortMode::Title),
            ColumnKey::Authors => Some(SortMode::Authors),
            ColumnKey::Series => Some(SortMode::Series),
            ColumnKey::Tags => Some(SortMode::Tags),
            ColumnKey::Formats => Some(SortMode::Formats),
            ColumnKey::Rating => Some(SortMode::Rating),
            ColumnKey::Publisher => Some(SortMode::Publisher),
            ColumnKey::Languages => Some(SortMode::Languages),
            ColumnKey::DateAdded => Some(SortMode::DateAdded),
            ColumnKey::DateModified => Some(SortMode::DateModified),
            ColumnKey::PubDate => Some(SortMode::PubDate),
            ColumnKey::Cover => None,
        }
    }
}

fn parse_column_key(value: &str) -> Option<ColumnKey> {
    match value {
        "title" => Some(ColumnKey::Title),
        "cover" => Some(ColumnKey::Cover),
        "authors" => Some(ColumnKey::Authors),
        "series" => Some(ColumnKey::Series),
        "tags" => Some(ColumnKey::Tags),
        "formats" => Some(ColumnKey::Formats),
        "rating" => Some(ColumnKey::Rating),
        "publisher" => Some(ColumnKey::Publisher),
        "languages" => Some(ColumnKey::Languages),
        "date_added" => Some(ColumnKey::DateAdded),
        "date_modified" => Some(ColumnKey::DateModified),
        "pubdate" => Some(ColumnKey::PubDate),
        _ => None,
    }
}

fn default_column_order() -> Vec<ColumnKey> {
    vec![
        ColumnKey::Title,
        ColumnKey::Cover,
        ColumnKey::Authors,
        ColumnKey::Series,
        ColumnKey::Tags,
        ColumnKey::Formats,
        ColumnKey::Rating,
        ColumnKey::Publisher,
        ColumnKey::Languages,
        ColumnKey::DateAdded,
        ColumnKey::DateModified,
        ColumnKey::PubDate,
    ]
}

#[derive(Debug, Clone)]
struct SortPreset {
    primary: SortMode,
    secondary: Option<SortMode>,
    direction: SortDirection,
}

#[derive(Debug, Clone)]
struct ColumnPreset {
    order: Vec<ColumnKey>,
    visibility: ColumnVisibility,
    widths: ColumnWidths,
}

#[derive(Debug, Clone)]
enum TableDisplayRow {
    GroupHeader(String),
    BookRow(usize),
}

#[derive(Debug, Clone, Default)]
struct InlineEditState {
    book_id: Option<i64>,
    title: String,
    authors: String,
    tags: String,
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
            date_added: gui.show_date_added,
            date_modified: gui.show_date_modified,
            pubdate: gui.show_pubdate,
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
        gui.show_date_added = self.date_added;
        gui.show_date_modified = self.date_modified;
        gui.show_pubdate = self.pubdate;
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
            date_added: gui.width_date_added,
            date_modified: gui.width_date_modified,
            pubdate: gui.width_pubdate,
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
        gui.width_date_added = self.date_added;
        gui.width_date_modified = self.date_modified;
        gui.width_pubdate = self.pubdate;
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
    sort_presets: HashMap<String, SortPreset>,
    sort_preset_name: String,
    active_sort_preset: Option<String>,
    column_presets: HashMap<String, ColumnPreset>,
    column_preset_name: String,
    column_preset_scope: ColumnPresetScope,
    active_column_preset: Option<String>,
    search_query: String,
    search_scope: SearchScope,
    search_history: Vec<String>,
    search_history_max: usize,
    search_commit_requested: bool,
    status: String,
    last_error: Option<String>,
    needs_refresh: bool,
    search_focus: bool,
    view_mode: ViewMode,
    view_density: ViewDensity,
    browser_visible: bool,
    browser_side: PaneSide,
    details_visible: bool,
    details_side: PaneSide,
    jobs_visible: bool,
    left_pane_width: f32,
    right_pane_width: f32,
    group_mode: GroupMode,
    shelf_columns: usize,
    quick_details_panel: bool,
    columns: ColumnVisibility,
    column_widths: ColumnWidths,
    column_order: Vec<ColumnKey>,
    column_search: String,
    layout_dirty: bool,
    config_dirty: bool,
    pending_save: bool,
    open_logs_requested: bool,
    log_dir: PathBuf,
    tmp_dir: PathBuf,
    conversion_job_history_path: PathBuf,
    conversion_job_logs_dir: PathBuf,
    max_job_history: usize,
    cover_thumb_size: f32,
    cover_preview_size: f32,
    show_format_badges: bool,
    show_language_badges: bool,
    conditional_missing_cover: bool,
    conditional_low_rating: bool,
    low_rating_threshold: i64,
    color_missing_cover: String,
    color_low_rating: String,
    cover_dir: PathBuf,
    cover_cache_dir: PathBuf,
    cover_max_bytes: u64,
    last_cover_thumb_size: f32,
    last_cover_preview_size: f32,
    table_row_height: f32,
    toast_duration_secs: f64,
    toast_max: usize,
    stats_top_n: usize,
    toasts: Vec<Toast>,
    jobs: Vec<JobEntry>,
    next_job_id: u64,
    last_tick: f64,
    comment_preview: bool,
    comment_preview_html: bool,
    comment_render_markdown: bool,
    comment_render_overrides: HashMap<i64, bool>,
    identifier_io_buffer: String,
    cover_history: Vec<String>,
    cover_favorites: BTreeSet<String>,
    cover_restore_history: Vec<String>,
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
    news_only_filter: bool,
    add_books: AddBooksDialogState,
    remove_books: RemoveBooksDialogState,
    bulk_edit: BulkEditDialogState,
    convert_books: ConvertBooksDialogState,
    save_to_disk: SaveToDiskDialogState,
    device_sync: DeviceSyncDialogState,
    device_manager: DeviceManagerDialogState,
    device_file_delete: DeviceFileDeleteDialogState,
    fetch_from_device: FetchFromDeviceDialogState,
    news_manager: NewsDialogState,
    manage_tags: ManageTagsDialogState,
    manage_series: ManageSeriesDialogState,
    manage_custom_columns: ManageCustomColumnsDialogState,
    manage_virtual_libraries: ManageVirtualLibrariesDialogState,
    plugins: PluginManagerDialogState,
    metadata_download_config: MetadataDownloadConfig,
    metadata_download: MetadataDownloadDialogState,
    edit_custom_fields: Vec<CustomEditField>,
    inline_edit: InlineEditState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewDensity {
    Compact,
    Comfortable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneSide {
    Left,
    Right,
}

impl PaneSide {
    fn as_config(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

fn parse_pane_side(value: &str) -> PaneSide {
    match value {
        "right" => PaneSide::Right,
        _ => PaneSide::Left,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ShellPaneLayout {
    pub browser_visible: bool,
    pub browser_side: PaneSide,
    pub details_visible: bool,
    pub details_side: PaneSide,
    pub jobs_visible: bool,
    pub left_width: f32,
    pub right_width: f32,
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
            sort_presets: decode_sort_presets(&config.gui.sort_presets),
            sort_preset_name: String::new(),
            active_sort_preset: config.gui.active_sort_preset.clone(),
            column_presets: decode_column_presets(&config.gui.column_presets),
            column_preset_name: String::new(),
            column_preset_scope: ColumnPresetScope::CurrentView,
            active_column_preset: config.gui.active_column_preset.clone(),
            search_query: String::new(),
            search_scope: SearchScope::All,
            search_history: Vec::new(),
            search_history_max: config.gui.search_history_max,
            search_commit_requested: false,
            status: "Ready".to_string(),
            last_error: None,
            needs_refresh: true,
            search_focus: false,
            view_mode: parse_view_mode(&config.gui.list_view_mode),
            view_density: parse_view_density(&config.gui.view_density),
            browser_visible: config.gui.pane_browser_visible,
            browser_side: parse_pane_side(&config.gui.pane_browser_side),
            details_visible: config.gui.pane_details_visible,
            details_side: parse_pane_side(&config.gui.pane_details_side),
            jobs_visible: config.gui.pane_jobs_visible,
            left_pane_width: config.gui.pane_left_width,
            right_pane_width: config.gui.pane_right_width,
            group_mode: parse_group_mode(&config.gui.group_mode),
            shelf_columns: config.gui.shelf_columns,
            quick_details_panel: config.gui.quick_details_panel,
            columns: ColumnVisibility::from_config(&config.gui),
            column_widths: ColumnWidths::from_config(&config.gui),
            column_order: decode_column_order(&config.gui.column_order),
            column_search: String::new(),
            layout_dirty: false,
            config_dirty: false,
            pending_save: false,
            open_logs_requested: false,
            log_dir: config.paths.log_dir.clone(),
            tmp_dir: config.paths.tmp_dir.clone(),
            conversion_job_history_path: config.conversion.job_history_path.clone(),
            conversion_job_logs_dir: config.conversion.job_logs_dir.clone(),
            max_job_history: config.conversion.max_job_history,
            cover_thumb_size: config.gui.cover_thumb_size,
            cover_preview_size: config.gui.cover_preview_size,
            show_format_badges: config.gui.show_format_badges,
            show_language_badges: config.gui.show_language_badges,
            conditional_missing_cover: config.gui.conditional_missing_cover,
            conditional_low_rating: config.gui.conditional_low_rating,
            low_rating_threshold: config.gui.low_rating_threshold,
            color_missing_cover: config.gui.color_missing_cover.clone(),
            color_low_rating: config.gui.color_low_rating.clone(),
            cover_dir: config.gui.cover_dir.clone(),
            cover_cache_dir: config.gui.cover_cache_dir.clone(),
            cover_max_bytes: config.gui.cover_max_bytes,
            last_cover_thumb_size: config.gui.cover_thumb_size,
            last_cover_preview_size: config.gui.cover_preview_size,
            table_row_height: config.gui.table_row_height,
            toast_duration_secs: config.gui.toast_duration_secs,
            toast_max: config.gui.toast_max,
            stats_top_n: config.gui.stats_top_n,
            toasts: Vec::new(),
            jobs: Vec::new(),
            next_job_id: 1,
            last_tick: 0.0,
            comment_preview: false,
            comment_preview_html: false,
            comment_render_markdown: true,
            comment_render_overrides: HashMap::new(),
            identifier_io_buffer: String::new(),
            cover_history: Vec::new(),
            cover_favorites: BTreeSet::new(),
            cover_restore_history: Vec::new(),
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
            news_only_filter: false,
            add_books: AddBooksDialogState::default(),
            remove_books: RemoveBooksDialogState::default(),
            bulk_edit: BulkEditDialogState::default(),
            convert_books: ConvertBooksDialogState::default(),
            save_to_disk: SaveToDiskDialogState::default(),
            device_sync: DeviceSyncDialogState::default(),
            device_manager: DeviceManagerDialogState::default(),
            device_file_delete: DeviceFileDeleteDialogState::default(),
            fetch_from_device: FetchFromDeviceDialogState::default(),
            news_manager: NewsDialogState::default(),
            manage_tags: ManageTagsDialogState::default(),
            manage_series: ManageSeriesDialogState::default(),
            manage_custom_columns: ManageCustomColumnsDialogState::default(),
            manage_virtual_libraries: ManageVirtualLibrariesDialogState::default(),
            plugins: PluginManagerDialogState::default(),
            metadata_download_config: config.metadata_download.clone(),
            metadata_download: MetadataDownloadDialogState::default_from_config(
                &config.metadata_download,
            ),
            edit_custom_fields: Vec::new(),
            inline_edit: InlineEditState::default(),
        };
        view.convert_books.apply_defaults(config);
        view.save_to_disk.apply_defaults(config);
        view.device_manager.apply_defaults(config);
        view.news_manager.apply_defaults(config);
        let _ = view.refresh_news_sources(config);
        let _ = view.refresh_news_downloads(config);
        let _ = view.load_news_history(config);
        let _ = view.load_job_history();
        if let Some(active) = &view.active_virtual_library {
            view.browser_filters = view
                .virtual_library_filters
                .get(active)
                .cloned()
                .unwrap_or_default();
        }
        if let Some(name) = view.active_sort_preset.clone() {
            view.apply_sort_preset(&name);
        }
        if let Some(name) = view.active_column_preset.clone() {
            view.apply_column_preset(&name);
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

    pub fn apply_global_search(&mut self, query: &str, scope: &str) {
        self.search_query = query.to_string();
        self.search_scope = match scope {
            "title" => SearchScope::Title,
            "authors" => SearchScope::Authors,
            "tags" => SearchScope::Tags,
            "series" => SearchScope::Series,
            _ => SearchScope::All,
        };
        self.search_commit_requested = true;
        self.needs_refresh = true;
    }

    pub fn clear_search_query(&mut self) {
        self.search_query.clear();
        self.needs_refresh = true;
    }

    pub fn filtered_count(&self) -> usize {
        self.books.len()
    }

    pub fn active_jobs_count(&self) -> usize {
        self.jobs
            .iter()
            .filter(|job| {
                matches!(
                    job.status,
                    JobStatus::Queued | JobStatus::Running | JobStatus::Paused
                )
            })
            .count()
    }

    pub fn set_shell_layout(&mut self, layout: ShellPaneLayout) {
        self.browser_visible = layout.browser_visible;
        self.browser_side = layout.browser_side;
        self.details_visible = layout.details_visible;
        self.details_side = layout.details_side;
        self.jobs_visible = layout.jobs_visible;
        self.left_pane_width = layout.left_width.clamp(320.0, 2400.0);
        self.right_pane_width = layout.right_width.clamp(280.0, 2000.0);
    }

    pub fn shell_layout(&self) -> ShellPaneLayout {
        ShellPaneLayout {
            browser_visible: self.browser_visible,
            browser_side: self.browser_side,
            details_visible: self.details_visible,
            details_side: self.details_side,
            jobs_visible: self.jobs_visible,
            left_width: self.left_pane_width,
            right_width: self.right_pane_width,
        }
    }

    pub fn recent_notifications(&self, limit: usize) -> Vec<String> {
        self.toasts
            .iter()
            .rev()
            .take(limit)
            .map(|toast| toast.message.clone())
            .collect()
    }

    pub fn open_add_books(&mut self, config: &ControlPlane) {
        self.add_books.apply_defaults(config);
        self.add_books.open = true;
    }

    pub fn ingest_paths_now(
        &mut self,
        config: &ControlPlane,
        paths: &[std::path::PathBuf],
    ) -> CoreResult<()> {
        if paths.is_empty() {
            return Ok(());
        }
        info!(
            component = "gui_library",
            ingest_paths = paths.len(),
            "running immediate ingest for dropped files"
        );
        self.add_books.apply_defaults(config);
        self.add_books.files_input = paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        self.run_add_books(config)
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

    pub fn open_device_manager(&mut self, config: &ControlPlane) {
        self.device_manager.apply_defaults(config);
        let _ = self.refresh_device_files(config);
        self.device_manager.open = true;
    }

    pub fn open_news_manager(&mut self, config: &ControlPlane) {
        self.news_manager.apply_defaults(config);
        let _ = self.refresh_news_sources(config);
        let _ = self.refresh_news_downloads(config);
        let _ = self.load_news_history(config);
        self.news_manager.open = true;
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

    pub fn open_manage_plugins(&mut self) {
        self.plugins.open = true;
        if self.plugins.selected.is_none() {
            self.plugins.selected = self.plugins.plugins.first().map(|entry| entry.id.clone());
        }
    }

    pub fn open_download_metadata(&mut self) {
        self.metadata_download.open = true;
        self.metadata_download.cover_only = false;
        self.metadata_download.progress = 0.0;
        self.metadata_download.failed = false;
        self.metadata_download.results.clear();
        self.metadata_download.queue_rows.clear();
        self.metadata_download.last_error = None;
        self.metadata_download.selected_book_id = self.selected_ids.first().copied();
        self.metadata_download.merge_tags = self.metadata_download_config.merge_tags_default;
        self.metadata_download.merge_identifiers =
            self.metadata_download_config.merge_identifiers_default;
        self.metadata_download.overwrite_title =
            self.metadata_download_config.overwrite_title_default;
        self.metadata_download.overwrite_authors =
            self.metadata_download_config.overwrite_authors_default;
        self.metadata_download.overwrite_publisher =
            self.metadata_download_config.overwrite_publisher_default;
        self.metadata_download.overwrite_language =
            self.metadata_download_config.overwrite_language_default;
        self.metadata_download.overwrite_pubdate =
            self.metadata_download_config.overwrite_pubdate_default;
        self.metadata_download.overwrite_comment =
            self.metadata_download_config.overwrite_comment_default;
        if let Some(source) = first_enabled_source(&self.metadata_download_config) {
            self.metadata_download.source = source;
        }
    }

    pub fn open_download_cover(&mut self) {
        self.metadata_download.open = true;
        self.metadata_download.cover_only = true;
        self.metadata_download.progress = 0.0;
        self.metadata_download.failed = false;
        self.metadata_download.results.clear();
        self.metadata_download.queue_rows.clear();
        self.metadata_download.last_error = None;
        self.metadata_download.selected_book_id = self.selected_ids.first().copied();
        self.metadata_download.merge_tags = self.metadata_download_config.merge_tags_default;
        self.metadata_download.merge_identifiers =
            self.metadata_download_config.merge_identifiers_default;
        self.metadata_download.overwrite_title =
            self.metadata_download_config.overwrite_title_default;
        self.metadata_download.overwrite_authors =
            self.metadata_download_config.overwrite_authors_default;
        self.metadata_download.overwrite_publisher =
            self.metadata_download_config.overwrite_publisher_default;
        self.metadata_download.overwrite_language =
            self.metadata_download_config.overwrite_language_default;
        self.metadata_download.overwrite_pubdate =
            self.metadata_download_config.overwrite_pubdate_default;
        self.metadata_download.overwrite_comment =
            self.metadata_download_config.overwrite_comment_default;
        if let Some(source) = first_enabled_source(&self.metadata_download_config) {
            self.metadata_download.source = source;
        }
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
        if let Some(details) = self.details.as_ref() {
            let book_id = details.book.id;
            self.show_edit_dialog = true;
            self.edit_mode = true;
            self.edit = EditState::from_details(details);
            if let Err(err) = self.load_edit_custom_fields(book_id) {
                self.set_error(err);
            }
            if let Err(err) = self.load_publish_slots(book_id) {
                self.set_error(err);
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, config: &mut ControlPlane, config_path: &Path) {
        let now = ui.ctx().input(|i| i.time);
        self.sync_cover_config(config);
        self.tick_jobs(now);
        self.prune_toasts(now);
        let available = ui.available_rect_before_wrap();
        let left_width = self
            .left_pane_width
            .clamp(320.0, (available.width() - 120.0).max(320.0));
        if self.browser_visible && self.browser_side == PaneSide::Right {
            let browser_panel = egui::Panel::right("browser_panel")
                .resizable(true)
                .default_size(self.right_pane_width.clamp(280.0, 1200.0))
                .show_inside(ui, |ui| {
                    ui.heading("Browser");
                    ui.separator();
                    self.browser_controls(ui);
                });
            self.right_pane_width = browser_panel.response.rect.width();
        }
        if self.details_visible && self.details_side == PaneSide::Right {
            let details_panel = egui::Panel::right("details_panel")
                .resizable(true)
                .default_size(self.right_pane_width.clamp(280.0, 1200.0))
                .show_inside(ui, |ui| {
                    ui.heading("Details");
                    ui.separator();
                    self.details_view(ui, config);
                });
            self.right_pane_width = details_panel.response.rect.width();
        }

        let list_panel = egui::Panel::left("library_list")
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
                self.management_controls(ui, config);
                if self.browser_visible && self.browser_side == PaneSide::Left {
                    ui.separator();
                    self.browser_controls(ui);
                }
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
                    ViewMode::Shelf => self.shelf_view(ui, config),
                }
                if self.quick_details_panel {
                    ui.separator();
                    self.quick_details_preview(ui);
                }
                if self.details_visible && self.details_side == PaneSide::Left {
                    ui.separator();
                    self.details_view(ui, config);
                }
                ui.separator();
                self.library_stats_panel(ui, &config.paths.cache_dir);
                ui.separator();
                self.status_bar(ui);
            });
        self.left_pane_width = list_panel.response.rect.width();

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if !self.details_visible {
                ui.centered_and_justified(|ui| {
                    ui.label("Details panel is hidden");
                });
            } else if self.details_side == PaneSide::Right {
                ui.centered_and_justified(|ui| {
                    ui.label("Details docked in right pane");
                });
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("Details docked in left pane");
                });
            }
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
        self.device_manager_dialog(ui, config);
        self.fetch_from_device_dialog(ui, config);
        self.device_file_delete_dialog(ui, config);
        self.news_dialog(ui, config);
        self.manage_tags_dialog(ui);
        self.manage_series_dialog(ui);
        self.manage_custom_columns_dialog(ui);
        self.manage_virtual_libraries_dialog(ui);
        self.plugins_dialog(ui);
        self.metadata_download_dialog(ui);
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

        if self.jobs_visible {
            self.render_jobs(ui);
        }
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
            egui::ComboBox::from_id_salt("search_scope")
                .selected_text(match self.search_scope {
                    SearchScope::All => "All",
                    SearchScope::Title => "Title",
                    SearchScope::Authors => "Authors",
                    SearchScope::Tags => "Tags",
                    SearchScope::Series => "Series",
                })
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(&mut self.search_scope, SearchScope::All, "All")
                        .clicked()
                    {
                        self.needs_refresh = true;
                    }
                    if ui
                        .selectable_value(&mut self.search_scope, SearchScope::Title, "Title")
                        .clicked()
                    {
                        self.needs_refresh = true;
                    }
                    if ui
                        .selectable_value(&mut self.search_scope, SearchScope::Authors, "Authors")
                        .clicked()
                    {
                        self.needs_refresh = true;
                    }
                    if ui
                        .selectable_value(&mut self.search_scope, SearchScope::Tags, "Tags")
                        .clicked()
                    {
                        self.needs_refresh = true;
                    }
                    if ui
                        .selectable_value(&mut self.search_scope, SearchScope::Series, "Series")
                        .clicked()
                    {
                        self.needs_refresh = true;
                    }
                });
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
                    ui.selectable_value(&mut self.sort_mode, SortMode::DateAdded, "Added");
                    ui.selectable_value(&mut self.sort_mode, SortMode::DateModified, "Modified");
                    ui.selectable_value(&mut self.sort_mode, SortMode::PubDate, "Pubdate");
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
                        Some(SortMode::DateAdded),
                        "Secondary: Added",
                    );
                    ui.selectable_value(
                        &mut self.secondary_sort,
                        Some(SortMode::DateModified),
                        "Secondary: Modified",
                    );
                    ui.selectable_value(
                        &mut self.secondary_sort,
                        Some(SortMode::PubDate),
                        "Secondary: Pubdate",
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
        ui.horizontal(|ui| {
            let selected = self
                .active_sort_preset
                .as_ref()
                .map(|name| name.as_str())
                .unwrap_or("Sort preset: none");
            egui::ComboBox::from_id_salt("sort_preset")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(self.active_sort_preset.is_none(), "Sort preset: none")
                        .clicked()
                    {
                        self.active_sort_preset = None;
                        self.config_dirty = true;
                    }
                    let mut names = self.sort_presets.keys().cloned().collect::<Vec<_>>();
                    names.sort();
                    for name in names {
                        if ui
                            .selectable_label(
                                self.active_sort_preset.as_deref() == Some(name.as_str()),
                                name.as_str(),
                            )
                            .clicked()
                        {
                            self.apply_sort_preset(&name);
                            self.active_sort_preset = Some(name);
                            self.config_dirty = true;
                        }
                    }
                });
            ui.label("Name");
            ui.text_edit_singleline(&mut self.sort_preset_name);
            if ui.button("Save preset").clicked() {
                self.save_sort_preset();
            }
            if ui
                .add_enabled(
                    self.active_sort_preset.is_some(),
                    egui::Button::new("Delete preset"),
                )
                .clicked()
            {
                self.delete_active_sort_preset();
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
            if ui
                .checkbox(&mut self.news_only_filter, "News only")
                .changed()
            {
                self.apply_filters();
            }
        });
    }

    fn filter_summary_controls(&mut self, ui: &mut egui::Ui) {
        let has_filters = !self.search_query.trim().is_empty()
            || self.format_filter.is_some()
            || !self.browser_filters.is_empty()
            || self.news_only_filter;
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
            if ui
                .checkbox(&mut self.news_only_filter, "News only")
                .changed()
            {
                self.apply_filters();
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
        self.news_only_filter = false;
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

    fn apply_stats_drilldown(&mut self, category: BrowserCategory, value: &str) {
        self.browser_filters
            .retain(|filter| filter.category != category);
        self.browser_filters.push(BrowserFilter {
            category,
            value: value.to_string(),
            mode: BrowserFilterMode::Include,
        });
        self.persist_active_virtual_filters();
        self.apply_filters();
        self.config_dirty = true;
        self.push_toast("Applied stats drilldown filter", ToastLevel::Info);
        info!(
            component = "gui",
            category = category.label(),
            value = value,
            "stats drilldown filter applied"
        );
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
                let stats = match compute_library_stats(&self.all_books, &self.db) {
                    Ok(stats) => stats,
                    Err(err) => {
                        self.set_error(err);
                        ui.label("Stats unavailable.");
                        return;
                    }
                };
                ui.label(format!("Formats: {}", stats.formats.len()));
                ui.label(format!("Languages: {}", stats.languages.len()));
                ui.label(format!("Tags: {}", stats.tags.len()));
                ui.label(format!("Authors: {}", stats.authors.len()));
                ui.label(format!("Series: {}", stats.series.len()));
                ui.separator();
                ui.label("Formats distribution");
                for (format, count) in stats.formats.iter().take(self.stats_top_n) {
                    ui.label(format!("{format}: {count}"));
                }
                ui.separator();
                ui.label("Format storage size (MiB)");
                let top_size = stats
                    .format_sizes
                    .first()
                    .map(|(_, size)| *size)
                    .unwrap_or(1);
                for (format, size) in stats.format_sizes.iter().take(self.stats_top_n) {
                    let frac = (*size as f32) / (top_size as f32);
                    ui.horizontal(|ui| {
                        ui.label(format!("{format}: {}", format_bytes(*size)));
                        ui.add(egui::ProgressBar::new(frac).desired_width(120.0));
                    });
                }
                ui.separator();
                ui.label("Top authors");
                for (author, count) in stats.authors.iter().take(self.stats_top_n) {
                    if ui.button(format!("{author} ({count})")).clicked() {
                        self.apply_stats_drilldown(BrowserCategory::Authors, author);
                    }
                }
                ui.label("Top series");
                for (series, count) in stats.series.iter().take(self.stats_top_n) {
                    if ui.button(format!("{series} ({count})")).clicked() {
                        self.apply_stats_drilldown(BrowserCategory::Series, series);
                    }
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
                    ViewMode::Shelf => "Shelf",
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
                    if ui
                        .selectable_value(&mut self.view_mode, ViewMode::Shelf, "Shelf")
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
            if self.view_mode == ViewMode::Shelf {
                let mut columns = self.shelf_columns as u32;
                if ui
                    .add(egui::Slider::new(&mut columns, 1..=8).text("Shelf columns"))
                    .changed()
                {
                    self.shelf_columns = columns as usize;
                    self.layout_dirty = true;
                    self.config_dirty = true;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Group by");
            egui::ComboBox::from_id_salt("group_mode")
                .selected_text(match self.group_mode {
                    GroupMode::None => "None",
                    GroupMode::Series => "Series",
                    GroupMode::Authors => "Authors",
                    GroupMode::Tags => "Tags",
                })
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(&mut self.group_mode, GroupMode::None, "None")
                        .clicked()
                    {
                        self.layout_dirty = true;
                        self.config_dirty = true;
                    }
                    if ui
                        .selectable_value(&mut self.group_mode, GroupMode::Series, "Series")
                        .clicked()
                    {
                        self.layout_dirty = true;
                        self.config_dirty = true;
                    }
                    if ui
                        .selectable_value(&mut self.group_mode, GroupMode::Authors, "Authors")
                        .clicked()
                    {
                        self.layout_dirty = true;
                        self.config_dirty = true;
                    }
                    if ui
                        .selectable_value(&mut self.group_mode, GroupMode::Tags, "Tags")
                        .clicked()
                    {
                        self.layout_dirty = true;
                        self.config_dirty = true;
                    }
                });
            if ui
                .checkbox(&mut self.conditional_missing_cover, "Mark missing cover")
                .changed()
            {
                self.layout_dirty = true;
                self.config_dirty = true;
            }
            if ui
                .checkbox(&mut self.conditional_low_rating, "Mark low rating")
                .changed()
            {
                self.layout_dirty = true;
                self.config_dirty = true;
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
                ui.horizontal(|ui| {
                    ui.label("Search");
                    ui.text_edit_singleline(&mut self.column_search);
                    if ui.button("Reset order").clicked() {
                        self.column_order = default_column_order();
                        self.layout_dirty = true;
                        info!(component = "gui", "column order reset");
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Scope");
                    egui::ComboBox::from_id_salt("column_preset_scope")
                        .selected_text(match self.column_preset_scope {
                            ColumnPresetScope::CurrentView => "Current view",
                            ColumnPresetScope::Global => "All views",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.column_preset_scope,
                                ColumnPresetScope::CurrentView,
                                "Current view",
                            );
                            ui.selectable_value(
                                &mut self.column_preset_scope,
                                ColumnPresetScope::Global,
                                "All views",
                            );
                        });
                    let selected = self
                        .active_column_preset
                        .as_ref()
                        .map(|name| name.as_str())
                        .unwrap_or("Column preset: none");
                    egui::ComboBox::from_id_salt("column_preset")
                        .selected_text(selected)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(
                                    self.active_column_preset.is_none(),
                                    "Column preset: none",
                                )
                                .clicked()
                            {
                                self.active_column_preset = None;
                                self.config_dirty = true;
                            }
                            let names = self.visible_column_preset_names();
                            for name in names {
                                if ui
                                    .selectable_label(
                                        self.active_column_preset.as_deref() == Some(name.as_str()),
                                        name.as_str(),
                                    )
                                    .clicked()
                                {
                                    self.apply_column_preset(&name);
                                    self.active_column_preset = Some(name);
                                    self.config_dirty = true;
                                }
                            }
                        });
                    ui.label("Name");
                    ui.text_edit_singleline(&mut self.column_preset_name);
                    if ui.button("Save preset").clicked() {
                        self.save_column_preset();
                    }
                    if ui
                        .add_enabled(
                            self.active_column_preset.is_some(),
                            egui::Button::new("Delete preset"),
                        )
                        .clicked()
                    {
                        self.delete_active_column_preset();
                    }
                });
                let filter = self.column_search.trim().to_lowercase();
                let mut idx = 0;
                while idx < self.column_order.len() {
                    let key = self.column_order[idx];
                    if !filter.is_empty() && !key.label().to_lowercase().contains(&filter) {
                        idx += 1;
                        continue;
                    }
                    ui.horizontal(|ui| {
                        let mut visible = self.column_visible(key);
                        if ui.checkbox(&mut visible, key.label()).changed() {
                            self.set_column_visible(key, visible);
                            self.layout_dirty = true;
                        }
                        if ui.small_button("↑").clicked() && idx > 0 {
                            self.column_order.swap(idx, idx - 1);
                            self.layout_dirty = true;
                            info!(
                                component = "gui",
                                column = key.key(),
                                from = idx,
                                to = idx - 1,
                                "column moved"
                            );
                        }
                        if ui.small_button("↓").clicked() && idx + 1 < self.column_order.len() {
                            self.column_order.swap(idx, idx + 1);
                            self.layout_dirty = true;
                            info!(
                                component = "gui",
                                column = key.key(),
                                from = idx,
                                to = idx + 1,
                                "column moved"
                            );
                        }
                    });
                    idx += 1;
                }
                ui.separator();
                ui.label("Column widths");
                for key in &self.column_order.clone() {
                    if !filter.is_empty() && !key.label().to_lowercase().contains(&filter) {
                        continue;
                    }
                    if let Some(width) = self.column_width_mut(*key) {
                        if column_width_control(ui, key.label(), width) {
                            self.layout_dirty = true;
                        }
                    }
                }
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
                if ui.button("News…").clicked() {
                    self.open_news_manager(config);
                }
            });
    }

    fn management_controls(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
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
                if ui.button("Plugins…").clicked() {
                    self.open_manage_plugins();
                }
                if ui.button("Devices…").clicked() {
                    self.open_device_manager(config);
                }
            });
    }

    fn table_view(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        let density_factor = match self.view_density {
            ViewDensity::Compact => 0.85,
            ViewDensity::Comfortable => 1.0,
        };
        let row_height = (self.table_row_height * density_factor).max(30.0);
        let visible_columns = self.visible_column_order();
        let display_rows = self.table_display_rows();
        let mut table = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0);
        for key in &visible_columns {
            if let Some(width) = self.column_width(*key) {
                table = table.column(column_with_width(width));
            }
        }

        table
            .header(row_height, |mut header| {
                for key in &visible_columns {
                    header.col(|ui| {
                        if let Some(mode) = key.sort_mode() {
                            self.sort_header(ui, key.label(), mode);
                        } else {
                            ui.label(key.label());
                        }
                    });
                }
            })
            .body(|body| {
                body.rows(row_height, display_rows.len(), |mut row| {
                    let row_index = row.index();
                    let entry = display_rows[row_index].clone();
                    match entry {
                        TableDisplayRow::GroupHeader(group) => {
                            for (idx, key) in visible_columns.iter().enumerate() {
                                row.col(|ui: &mut egui::Ui| {
                                    if idx == 0 || matches!(key, ColumnKey::Title) {
                                        ui.label(egui::RichText::new(group.clone()).strong());
                                    } else {
                                        ui.label("");
                                    }
                                });
                            }
                        }
                        TableDisplayRow::BookRow(book_idx) => {
                            let book = self.books[book_idx].clone();
                            let selected = self.selected_ids.contains(&book.id);
                            let mut row_clicked = false;
                            let mut modifiers = egui::Modifiers::default();
                            for key in &visible_columns {
                                row.col(|ui: &mut egui::Ui| {
                                    self.render_table_cell(
                                        ui,
                                        config,
                                        &book,
                                        *key,
                                        selected,
                                        &mut row_clicked,
                                        &mut modifiers,
                                    );
                                });
                            }
                            if row_clicked {
                                self.handle_selection(book_idx, modifiers);
                            }
                        }
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

    fn shelf_view(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        let books = self.books.clone();
        let columns = self.shelf_columns.max(1);
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut index = 0usize;
            while index < books.len() {
                ui.horizontal(|ui| {
                    for _ in 0..columns {
                        if index >= books.len() {
                            break;
                        }
                        let book = books[index].clone();
                        let selected = self.selected_ids.contains(&book.id);
                        ui.vertical(|ui| {
                            let texture =
                                self.cover_thumb_texture(ui.ctx(), book.id, book.has_cover);
                            render_cover_thumbnail(
                                ui,
                                texture.as_ref(),
                                book.has_cover,
                                self.cover_thumb_size.max(72.0),
                            );
                            if ui.selectable_label(selected, book.title.clone()).clicked() {
                                self.select_book(book.id);
                            }
                            ui.horizontal(|ui| {
                                if ui.small_button("E").clicked() {
                                    self.select_book(book.id);
                                    self.begin_edit();
                                }
                                if ui.small_button("R").clicked() {
                                    self.selected_ids = vec![book.id];
                                    self.open_remove_books(config);
                                }
                                if ui.small_button("C").clicked() {
                                    self.selected_ids = vec![book.id];
                                    self.open_convert_books(config);
                                }
                            });
                        });
                        index += 1;
                    }
                });
                ui.separator();
            }
        });
    }

    fn table_display_rows(&self) -> Vec<TableDisplayRow> {
        if self.group_mode == GroupMode::None {
            return (0..self.books.len())
                .map(TableDisplayRow::BookRow)
                .collect();
        }
        let mut rows = Vec::new();
        let mut current_group = String::new();
        for (idx, book) in self.books.iter().enumerate() {
            let group = self.book_group_label(book);
            if group != current_group {
                current_group = group.clone();
                rows.push(TableDisplayRow::GroupHeader(group));
            }
            rows.push(TableDisplayRow::BookRow(idx));
        }
        rows
    }

    fn book_group_label(&self, book: &BookRow) -> String {
        match self.group_mode {
            GroupMode::None => "All books".to_string(),
            GroupMode::Series => {
                if book.series.trim().is_empty() {
                    "Series: (none)".to_string()
                } else {
                    format!("Series: {}", book.series)
                }
            }
            GroupMode::Authors => {
                let first = split_csv_field(&book.authors)
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| "(none)".to_string());
                format!("Author: {first}")
            }
            GroupMode::Tags => {
                let first = split_csv_field(&book.tags)
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| "(none)".to_string());
                format!("Tag: {first}")
            }
        }
    }

    fn visible_column_order(&self) -> Vec<ColumnKey> {
        self.column_order
            .iter()
            .copied()
            .filter(|key| self.column_visible(*key))
            .collect()
    }

    fn column_visible(&self, key: ColumnKey) -> bool {
        match key {
            ColumnKey::Title => self.columns.title,
            ColumnKey::Cover => self.columns.cover,
            ColumnKey::Authors => self.columns.authors,
            ColumnKey::Series => self.columns.series,
            ColumnKey::Tags => self.columns.tags,
            ColumnKey::Formats => self.columns.formats,
            ColumnKey::Rating => self.columns.rating,
            ColumnKey::Publisher => self.columns.publisher,
            ColumnKey::Languages => self.columns.languages,
            ColumnKey::DateAdded => self.columns.date_added,
            ColumnKey::DateModified => self.columns.date_modified,
            ColumnKey::PubDate => self.columns.pubdate,
        }
    }

    fn set_column_visible(&mut self, key: ColumnKey, visible: bool) {
        match key {
            ColumnKey::Title => self.columns.title = visible,
            ColumnKey::Cover => self.columns.cover = visible,
            ColumnKey::Authors => self.columns.authors = visible,
            ColumnKey::Series => self.columns.series = visible,
            ColumnKey::Tags => self.columns.tags = visible,
            ColumnKey::Formats => self.columns.formats = visible,
            ColumnKey::Rating => self.columns.rating = visible,
            ColumnKey::Publisher => self.columns.publisher = visible,
            ColumnKey::Languages => self.columns.languages = visible,
            ColumnKey::DateAdded => self.columns.date_added = visible,
            ColumnKey::DateModified => self.columns.date_modified = visible,
            ColumnKey::PubDate => self.columns.pubdate = visible,
        }
    }

    fn column_width(&self, key: ColumnKey) -> Option<f32> {
        let width = match key {
            ColumnKey::Title => self.column_widths.title,
            ColumnKey::Cover => self.column_widths.cover,
            ColumnKey::Authors => self.column_widths.authors,
            ColumnKey::Series => self.column_widths.series,
            ColumnKey::Tags => self.column_widths.tags,
            ColumnKey::Formats => self.column_widths.formats,
            ColumnKey::Rating => self.column_widths.rating,
            ColumnKey::Publisher => self.column_widths.publisher,
            ColumnKey::Languages => self.column_widths.languages,
            ColumnKey::DateAdded => self.column_widths.date_added,
            ColumnKey::DateModified => self.column_widths.date_modified,
            ColumnKey::PubDate => self.column_widths.pubdate,
        };
        Some(width)
    }

    fn column_width_mut(&mut self, key: ColumnKey) -> Option<&mut f32> {
        let width = match key {
            ColumnKey::Title => &mut self.column_widths.title,
            ColumnKey::Cover => &mut self.column_widths.cover,
            ColumnKey::Authors => &mut self.column_widths.authors,
            ColumnKey::Series => &mut self.column_widths.series,
            ColumnKey::Tags => &mut self.column_widths.tags,
            ColumnKey::Formats => &mut self.column_widths.formats,
            ColumnKey::Rating => &mut self.column_widths.rating,
            ColumnKey::Publisher => &mut self.column_widths.publisher,
            ColumnKey::Languages => &mut self.column_widths.languages,
            ColumnKey::DateAdded => &mut self.column_widths.date_added,
            ColumnKey::DateModified => &mut self.column_widths.date_modified,
            ColumnKey::PubDate => &mut self.column_widths.pubdate,
        };
        Some(width)
    }

    fn render_table_cell(
        &mut self,
        ui: &mut egui::Ui,
        config: &ControlPlane,
        book: &BookRow,
        key: ColumnKey,
        selected: bool,
        row_clicked: &mut bool,
        modifiers: &mut egui::Modifiers,
    ) {
        let row_color = self.row_text_color(book);
        match key {
            ColumnKey::Title => {
                if self.inline_edit.book_id == Some(book.id) {
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.inline_edit.title);
                        if ui.small_button("Save").clicked() {
                            if let Err(err) = self.save_inline_edit() {
                                self.set_error(err);
                            }
                        }
                        if ui.small_button("Cancel").clicked() {
                            self.cancel_inline_edit();
                        }
                    });
                    return;
                }
                ui.horizontal(|ui| {
                    let response = ui.selectable_label(selected, row_text(&book.title, row_color));
                    if response.clicked() {
                        *row_clicked = true;
                        *modifiers = response.ctx.input(|i| i.modifiers);
                    }
                    response.context_menu(|ui| {
                        self.row_context_menu(ui, config, book);
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
                    if ui.small_button("I").on_hover_text("Inline edit").clicked() {
                        self.begin_inline_edit(book);
                    }
                });
            }
            ColumnKey::Cover => {
                let texture = self.cover_thumb_texture(ui.ctx(), book.id, book.has_cover);
                render_cover_thumbnail(ui, texture.as_ref(), book.has_cover, self.cover_thumb_size);
            }
            ColumnKey::Authors => {
                if self.inline_edit.book_id == Some(book.id) {
                    ui.text_edit_singleline(&mut self.inline_edit.authors);
                } else {
                    self.selectable_cell(
                        ui,
                        selected,
                        &book.authors,
                        row_clicked,
                        modifiers,
                        row_color,
                    )
                }
            }
            ColumnKey::Series => self.selectable_cell(
                ui,
                selected,
                &book.series,
                row_clicked,
                modifiers,
                row_color,
            ),
            ColumnKey::Tags => {
                if self.inline_edit.book_id == Some(book.id) {
                    ui.text_edit_singleline(&mut self.inline_edit.tags);
                } else {
                    self.selectable_cell(
                        ui,
                        selected,
                        &book.tags,
                        row_clicked,
                        modifiers,
                        row_color,
                    )
                }
            }
            ColumnKey::Formats => {
                let text = if self.show_format_badges {
                    format_badge_text(&book.format)
                } else {
                    book.format.clone()
                };
                self.selectable_cell(ui, selected, &text, row_clicked, modifiers, row_color);
            }
            ColumnKey::Rating => self.selectable_cell(
                ui,
                selected,
                &book.rating,
                row_clicked,
                modifiers,
                row_color,
            ),
            ColumnKey::Publisher => self.selectable_cell(
                ui,
                selected,
                &book.publisher,
                row_clicked,
                modifiers,
                row_color,
            ),
            ColumnKey::Languages => {
                let text = if self.show_language_badges {
                    language_badge_text(&book.languages)
                } else {
                    book.languages.clone()
                };
                self.selectable_cell(ui, selected, &text, row_clicked, modifiers, row_color);
            }
            ColumnKey::DateAdded => self.selectable_cell(
                ui,
                selected,
                &format_date_cell(&book.date_added),
                row_clicked,
                modifiers,
                row_color,
            ),
            ColumnKey::DateModified => self.selectable_cell(
                ui,
                selected,
                &format_date_cell(&book.date_modified),
                row_clicked,
                modifiers,
                row_color,
            ),
            ColumnKey::PubDate => self.selectable_cell(
                ui,
                selected,
                &format_date_cell(&book.pubdate),
                row_clicked,
                modifiers,
                row_color,
            ),
        }
    }

    fn selectable_cell(
        &self,
        ui: &mut egui::Ui,
        selected: bool,
        text: &str,
        row_clicked: &mut bool,
        modifiers: &mut egui::Modifiers,
        color: Option<egui::Color32>,
    ) {
        let label = row_text(text, color);
        let response =
            ui.selectable_label(selected, highlight_rich_text(label, &self.search_query));
        if response.clicked() {
            *row_clicked = true;
            *modifiers = response.ctx.input(|i| i.modifiers);
        }
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
                    if ui.button("Paste cover").clicked() {
                        action = DetailAction::PasteCoverClipboard;
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
                egui::CollapsingHeader::new("Cover browser")
                    .default_open(false)
                    .show(ui, |ui| {
                        if self.cover_history.is_empty() {
                            ui.label("No cover history.");
                        } else {
                            for entry in self.cover_history.clone() {
                                ui.horizontal(|ui| {
                                    let is_favorite = self.cover_favorites.contains(&entry);
                                    if ui
                                        .small_button(if is_favorite { "★" } else { "☆" })
                                        .clicked()
                                    {
                                        if is_favorite {
                                            self.cover_favorites.remove(&entry);
                                        } else {
                                            self.cover_favorites.insert(entry.clone());
                                        }
                                    }
                                    if ui.button(entry.clone()).clicked() {
                                        self.cover_state.cover_path_input = entry.clone();
                                    }
                                });
                            }
                        }
                    });
                egui::CollapsingHeader::new("Cover favorites")
                    .default_open(false)
                    .show(ui, |ui| {
                        if self.cover_favorites.is_empty() {
                            ui.label("No favorites.");
                        } else {
                            for entry in self.cover_favorites.clone() {
                                if ui.button(entry.clone()).clicked() {
                                    self.cover_state.cover_path_input = entry;
                                }
                            }
                        }
                    });
                egui::CollapsingHeader::new("Removed cover history")
                    .default_open(false)
                    .show(ui, |ui| {
                        if self.cover_restore_history.is_empty() {
                            ui.label("No removed covers.");
                        } else {
                            for entry in self.cover_restore_history.clone() {
                                ui.horizontal(|ui| {
                                    ui.label(entry.clone());
                                    if ui.button("Restore path").clicked() {
                                        self.cover_state.cover_path_input = entry;
                                    }
                                });
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
            DetailAction::PasteCoverClipboard => {
                if let Some(details) = &details_snapshot {
                    if let Err(err) = self.apply_cover_from_clipboard(details.book.id) {
                        self.set_error(err);
                    } else {
                        let _ = self.load_details(details.book.id);
                        self.push_toast("Cover updated from clipboard", ToastLevel::Info);
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
        let baseline = self
            .details
            .as_ref()
            .map(EditState::from_details)
            .unwrap_or_default();
        let mut request_save = false;
        let mut request_cancel = false;
        let mut request_undo = false;
        let mut request_generate_uuid = false;
        let mut request_copy_identifiers = false;
        let mut request_normalize = false;
        let mut request_resolve = false;
        egui::Window::new("Edit Metadata")
            .open(&mut open)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                request_save = ui
                    .ctx()
                    .input(|i| i.modifiers.command && i.key_pressed(egui::Key::S));
                request_cancel = ui.ctx().input(|i| i.key_pressed(egui::Key::Escape));
                request_undo = ui
                    .ctx()
                    .input(|i| i.modifiers.command && i.key_pressed(egui::Key::R));
                request_generate_uuid = ui
                    .ctx()
                    .input(|i| i.modifiers.command && i.key_pressed(egui::Key::G));
                request_copy_identifiers = ui
                    .ctx()
                    .input(|i| i.modifiers.command && i.key_pressed(egui::Key::I));
                request_normalize = ui.ctx().input(|i| {
                    i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::N)
                });
                request_resolve = ui.ctx().input(|i| {
                    i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::F)
                });
                ui.small(
                    "Shortcuts: Ctrl/Cmd+S save, Esc cancel, Ctrl/Cmd+R undo, Ctrl/Cmd+G UUID, Ctrl/Cmd+I copy IDs, Ctrl/Cmd+Shift+N normalize, Ctrl/Cmd+Shift+F auto-fix",
                );
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Title")
                        .on_hover_text("Primary display title for the book.");
                    ui.text_edit_singleline(&mut self.edit.title);
                    if ui.small_button("Reset").clicked() {
                        self.edit.title = baseline.title.clone();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Authors")
                        .on_hover_text("Comma-separated author names.");
                    ui.text_edit_singleline(&mut self.edit.authors);
                    if ui.small_button("Reset").clicked() {
                        self.edit.authors = baseline.authors.clone();
                    }
                });
                if let Some(message) = duplicate_csv_hint("authors", &self.edit.authors) {
                    ui.colored_label(egui::Color32::from_rgb(170, 90, 20), message);
                }
                ui.horizontal(|ui| {
                    ui.label("Author sort");
                    ui.text_edit_singleline(&mut self.edit.author_sort);
                    if ui.small_button("Reset").clicked() {
                        self.edit.author_sort = baseline.author_sort.clone();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Tags");
                    ui.text_edit_singleline(&mut self.edit.tags);
                    if ui.small_button("Reset").clicked() {
                        self.edit.tags = baseline.tags.clone();
                    }
                });
                self.tag_autocomplete(ui);
                ui.label("Series");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.edit.series_name);
                    if ui.small_button("Reset").clicked() {
                        self.edit.series_name = baseline.series_name.clone();
                        self.edit.series_index = baseline.series_index;
                    }
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
                ui.horizontal(|ui| {
                    ui.label("Title sort")
                        .on_hover_text("Sort key used for title ordering.");
                    ui.text_edit_singleline(&mut self.edit.series_sort);
                    if ui.small_button("Reset").clicked() {
                        self.edit.series_sort = baseline.series_sort.clone();
                    }
                    if ui.small_button("Derive").clicked() {
                        self.edit.series_sort = derive_title_sort(&self.edit.title);
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Normalize authors").clicked() {
                        self.edit.authors = normalize_csv_list(&self.edit.authors, false);
                    }
                    if ui.button("Normalize tags").clicked() {
                        self.edit.tags = normalize_csv_list(&self.edit.tags, false);
                    }
                    if ui.button("Normalize languages").clicked() {
                        self.edit.languages = normalize_csv_list(&self.edit.languages, true);
                    }
                });
                ui.label("Identifiers (one per line, type:value)");
                ui.text_edit_multiline(&mut self.edit.identifiers);
                if let Some(message) = identifier_conflict_hint(&self.edit.identifiers) {
                    ui.colored_label(egui::Color32::from_rgb(170, 90, 20), message);
                }
                ui.horizontal(|ui| {
                    if ui.button("Add ISBN").clicked() && !self.edit.isbn.trim().is_empty() {
                        self.edit
                            .identifiers
                            .push_str(&format!("\nisbn:{}", self.edit.isbn.trim()));
                        self.edit.identifiers = cleanup_identifier_lines(&self.edit.identifiers);
                    }
                    if ui.button("Add ASIN").clicked() {
                        self.edit.identifiers.push_str("\nasin:");
                    }
                    if ui.button("Add DOI").clicked() {
                        self.edit.identifiers.push_str("\ndoi:");
                    }
                    if ui.button("Dedupe").clicked() {
                        self.edit.identifiers = cleanup_identifier_lines(&self.edit.identifiers);
                    }
                    if ui.button("Normalize IDs").clicked() {
                        self.edit.identifiers = normalize_identifier_lines(&self.edit.identifiers);
                    }
                    if ui.button("Copy IDs").clicked() {
                        ui.ctx().copy_text(self.edit.identifiers.clone());
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Identifier import/export buffer");
                    ui.text_edit_singleline(&mut self.identifier_io_buffer);
                    if ui.button("Import").clicked() {
                        if !self.identifier_io_buffer.trim().is_empty() {
                            self.edit.identifiers = self.identifier_io_buffer.clone();
                        }
                    }
                    if ui.button("Export").clicked() {
                        self.identifier_io_buffer = self.edit.identifiers.clone();
                    }
                });
                identifier_validation_badges(ui, &self.edit.identifiers);
                ui.label("ISBN");
                ui.text_edit_singleline(&mut self.edit.isbn);
                ui.horizontal(|ui| {
                    if ui.button("Open ISBN").clicked() && !self.edit.isbn.trim().is_empty() {
                        let url = format!("https://isbnsearch.org/isbn/{}", self.edit.isbn.trim());
                        if let Err(err) = open_url(&url) {
                            self.set_error(err);
                        }
                    }
                    if ui.button("Open ASIN").clicked() {
                        if let Some(asin) = find_identifier_value(&self.edit.identifiers, "asin") {
                            let url = format!("https://www.amazon.com/dp/{asin}");
                            if let Err(err) = open_url(&url) {
                                self.set_error(err);
                            }
                        }
                    }
                    if ui.button("Open DOI").clicked() {
                        if let Some(doi) = find_identifier_value(&self.edit.identifiers, "doi") {
                            let url = format!("https://doi.org/{doi}");
                            if let Err(err) = open_url(&url) {
                                self.set_error(err);
                            }
                        }
                    }
                });
                ui.label("Publisher");
                ui.text_edit_singleline(&mut self.edit.publisher);
                ui.horizontal(|ui| {
                    ui.label("Imprint");
                    ui.text_edit_singleline(&mut self.edit.imprint);
                    if ui.small_button("Reset").clicked() {
                        self.edit.imprint = baseline.imprint.clone();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Edition");
                    ui.text_edit_singleline(&mut self.edit.edition);
                    if ui.small_button("Reset").clicked() {
                        self.edit.edition = baseline.edition.clone();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Rights");
                    ui.text_edit_singleline(&mut self.edit.rights);
                    if ui.small_button("Reset").clicked() {
                        self.edit.rights = baseline.rights.clone();
                    }
                });
                ui.label("Languages (comma separated)");
                ui.text_edit_singleline(&mut self.edit.languages);
                self.language_autocomplete(ui);
                if let Some(message) = language_hint(&self.edit.languages) {
                    ui.colored_label(egui::Color32::from_rgb(170, 90, 20), message);
                }
                ui.horizontal(|ui| {
                    ui.label("Timestamp");
                    ui.text_edit_singleline(&mut self.edit.timestamp);
                    if ui.small_button("Reset").clicked() {
                        self.edit.timestamp = baseline.timestamp.clone();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Publication date");
                    ui.text_edit_singleline(&mut self.edit.pubdate);
                    if ui.small_button("Reset").clicked() {
                        self.edit.pubdate = baseline.pubdate.clone();
                        self.edit.pubdate_year = baseline.pubdate_year;
                        self.edit.pubdate_month = baseline.pubdate_month;
                        self.edit.pubdate_day = baseline.pubdate_day;
                    }
                    if ui.small_button("Today").clicked() {
                        self.edit.pubdate = current_date_ymd();
                        if let Some((y, m, d)) = parse_date_parts(&self.edit.pubdate) {
                            self.edit.pubdate_year = y;
                            self.edit.pubdate_month = m;
                            self.edit.pubdate_day = d;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Published Y/M/D")
                        .on_hover_text("Helper values for publication date composition.");
                    ui.add(egui::DragValue::new(&mut self.edit.pubdate_year).range(0..=9999));
                    ui.add(egui::DragValue::new(&mut self.edit.pubdate_month).range(1..=12));
                    ui.add(egui::DragValue::new(&mut self.edit.pubdate_day).range(1..=31));
                    if ui.small_button("Apply").clicked() {
                        self.edit.pubdate = format!(
                            "{:04}-{:02}-{:02}",
                            self.edit.pubdate_year, self.edit.pubdate_month, self.edit.pubdate_day
                        );
                    }
                    if ui.small_button("Sync from text").clicked() {
                        if let Some((y, m, d)) = parse_date_parts(&self.edit.pubdate) {
                            self.edit.pubdate_year = y;
                            self.edit.pubdate_month = m;
                            self.edit.pubdate_day = d;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Last modified");
                    ui.text_edit_singleline(&mut self.edit.last_modified);
                    if ui.small_button("Reset").clicked() {
                        self.edit.last_modified = baseline.last_modified.clone();
                    }
                    if ui.small_button("Now").clicked() {
                        if let Ok(now) = now_timestamp() {
                            self.edit.last_modified = now;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("UUID");
                    ui.text_edit_singleline(&mut self.edit.uuid);
                    if ui.small_button("Reset").clicked() {
                        self.edit.uuid = baseline.uuid.clone();
                    }
                    if ui.small_button("Generate").clicked() {
                        self.edit.uuid = uuid::Uuid::new_v4().to_string();
                    }
                    if ui.small_button("Copy").clicked() {
                        ui.ctx().copy_text(self.edit.uuid.clone());
                    }
                });
                ui.label("Rating");
                rating_stars(ui, &mut self.edit.rating);
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Rating value: {}",
                        format_half_star_rating(self.edit.rating)
                    ));
                    ui.add(egui::Slider::new(&mut self.edit.rating, 0..=10).text("half-stars"));
                    if ui.small_button("Reset").clicked() {
                        self.edit.rating = baseline.rating;
                    }
                });
                ui.label("Comment");
                ui.horizontal(|ui| {
                    if ui.button("Bold").clicked() {
                        self.edit.comment.push_str(" **bold**");
                    }
                    if ui.button("Italic").clicked() {
                        self.edit.comment.push_str(" *italic*");
                    }
                    if ui.button("Heading").clicked() {
                        self.edit.comment.push_str("\n# Heading\n");
                    }
                    if ui.button("Link").clicked() {
                        self.edit.comment.push_str("[text](https://example.com)");
                    }
                });
                ui.text_edit_multiline(&mut self.edit.comment);
                ui.checkbox(&mut self.comment_preview, "Preview comment");
                ui.checkbox(&mut self.comment_preview_html, "Preview as HTML fallback");
                if self.comment_preview {
                    ui.separator();
                    ui.label("Preview");
                    if self.comment_preview_html {
                        render_html_fallback(ui, &self.edit.comment);
                    } else {
                        render_markdown(ui, &self.edit.comment);
                    }
                }
                egui::CollapsingHeader::new("Custom metadata fields")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Filter");
                            ui.text_edit_singleline(&mut self.manage_custom_columns.value_filter);
                            if ui.small_button("Clear filter").clicked() {
                                self.manage_custom_columns.value_filter.clear();
                            }
                        });
                        if self.edit_custom_fields.is_empty() {
                            ui.label("No custom columns defined.");
                        } else {
                            let filter = self.manage_custom_columns.value_filter.trim().to_lowercase();
                            let mut rendered = 0usize;
                            for field in &mut self.edit_custom_fields {
                                if !filter.is_empty()
                                    && !field.label.to_lowercase().contains(&filter)
                                    && !field.name.to_lowercase().contains(&filter)
                                    && !field.datatype.to_lowercase().contains(&filter)
                                {
                                    continue;
                                }
                                rendered += 1;
                                ui.horizontal(|ui| {
                                    ui.label(format!("{} ({})", field.name, field.datatype));
                                    custom_field_editor_widget(ui, field);
                                    if ui.small_button("Clear").clicked() {
                                        field.value.clear();
                                    }
                                });
                            }
                            if rendered == 0 {
                                ui.label("No matching custom fields for current filter.");
                            }
                        }
                    });
                egui::CollapsingHeader::new("Validation summary")
                    .default_open(false)
                    .show(ui, |ui| {
                        let issues = collect_edit_validation_issues(&self.edit);
                        ui.horizontal(|ui| {
                            if ui.button("Normalize fields").clicked() {
                                self.edit.authors = normalize_csv_list(&self.edit.authors, false);
                                self.edit.tags = normalize_csv_list(&self.edit.tags, false);
                                self.edit.languages = normalize_csv_list(&self.edit.languages, true);
                                self.edit.identifiers = normalize_identifier_lines(&self.edit.identifiers);
                                self.edit.publisher = self.edit.publisher.trim().to_string();
                                self.edit.imprint = self.edit.imprint.trim().to_string();
                                self.edit.edition = self.edit.edition.trim().to_string();
                                self.edit.rights = self.edit.rights.trim().to_string();
                            }
                            if ui.button("Auto-fix conflicts").clicked() {
                                self.edit.identifiers = dedupe_identifier_lines(&self.edit.identifiers);
                                if !is_loose_date_or_datetime(self.edit.pubdate.trim()) {
                                    self.edit.pubdate = current_date_ymd();
                                }
                                if !is_loose_date_or_datetime(self.edit.timestamp.trim()) {
                                    self.edit.timestamp = current_date_ymd();
                                }
                                if !is_loose_date_or_datetime(self.edit.last_modified.trim()) {
                                    if let Ok(now) = now_timestamp() {
                                        self.edit.last_modified = now;
                                    } else {
                                        self.edit.last_modified = current_date_ymd();
                                    }
                                }
                                if uuid::Uuid::parse_str(self.edit.uuid.trim()).is_err() {
                                    self.edit.uuid = uuid::Uuid::new_v4().to_string();
                                }
                            }
                        });
                        for issue in &issues {
                            ui.colored_label(egui::Color32::from_rgb(180, 40, 40), issue);
                        }
                        if issues.is_empty() {
                            ui.colored_label(egui::Color32::from_rgb(40, 140, 60), "No issues");
                        }
                    });
                egui::CollapsingHeader::new("Diff view (before/after)")
                    .default_open(false)
                    .show(ui, |ui| {
                        for (name, before, after) in edit_diff_rows(&baseline, &self.edit) {
                            ui.horizontal(|ui| {
                                ui.label(name);
                                ui.monospace(format!("{before} -> {after}"));
                            });
                        }
                    });
                ui.label(format!("UUID: {}", self.edit.uuid));
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Undo all").clicked() {
                        self.edit = baseline.clone();
                        if let Some(book_id) = self.details.as_ref().map(|details| details.book.id) {
                            if let Err(err) = self.load_edit_custom_fields(book_id) {
                                self.set_error(err);
                            }
                            if let Err(err) = self.load_publish_slots(book_id) {
                                self.set_error(err);
                            }
                        }
                    }
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
        if request_generate_uuid {
            self.edit.uuid = uuid::Uuid::new_v4().to_string();
        }
        if request_copy_identifiers {
            ui.ctx().copy_text(self.edit.identifiers.clone());
        }
        if request_normalize {
            self.edit.authors = normalize_csv_list(&self.edit.authors, false);
            self.edit.tags = normalize_csv_list(&self.edit.tags, false);
            self.edit.languages = normalize_csv_list(&self.edit.languages, true);
            self.edit.identifiers = normalize_identifier_lines(&self.edit.identifiers);
            self.edit.publisher = self.edit.publisher.trim().to_string();
            self.edit.imprint = self.edit.imprint.trim().to_string();
            self.edit.edition = self.edit.edition.trim().to_string();
            self.edit.rights = self.edit.rights.trim().to_string();
        }
        if request_resolve {
            self.edit.identifiers = dedupe_identifier_lines(&self.edit.identifiers);
            if !is_loose_date_or_datetime(self.edit.pubdate.trim()) {
                self.edit.pubdate = current_date_ymd();
            }
            if !is_loose_date_or_datetime(self.edit.timestamp.trim()) {
                self.edit.timestamp = current_date_ymd();
            }
            if !is_loose_date_or_datetime(self.edit.last_modified.trim()) {
                if let Ok(now) = now_timestamp() {
                    self.edit.last_modified = now;
                }
            }
            if uuid::Uuid::parse_str(self.edit.uuid.trim()).is_err() {
                self.edit.uuid = uuid::Uuid::new_v4().to_string();
            }
        }
        if request_undo {
            self.edit = baseline.clone();
            if let Some(book_id) = self.details.as_ref().map(|details| details.book.id) {
                if let Err(err) = self.load_edit_custom_fields(book_id) {
                    self.set_error(err);
                }
                if let Err(err) = self.load_publish_slots(book_id) {
                    self.set_error(err);
                }
            }
        }
        if request_cancel {
            self.cancel_edit();
            open = false;
        }
        if request_save {
            self.pending_save = true;
            open = false;
        }
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
        let mut save_preset = false;
        let mut load_preset: Option<String> = None;
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
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Input profile");
                    egui::ComboBox::from_id_salt("convert_input_profile")
                        .selected_text(self.convert_books.input_profile.as_str())
                        .show_ui(ui, |ui| {
                            for profile in &config.conversion.input_profiles {
                                ui.selectable_value(
                                    &mut self.convert_books.input_profile,
                                    profile.clone(),
                                    profile,
                                );
                            }
                        });
                    ui.label("Output profile");
                    egui::ComboBox::from_id_salt("convert_output_profile")
                        .selected_text(self.convert_books.output_profile.as_str())
                        .show_ui(ui, |ui| {
                            for profile in &config.conversion.output_profiles {
                                ui.selectable_value(
                                    &mut self.convert_books.output_profile,
                                    profile.clone(),
                                    profile,
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
                    ui.checkbox(
                        &mut self.convert_books.heuristic_enable,
                        "Enable heuristics",
                    );
                    ui.checkbox(
                        &mut self.convert_books.heuristic_unwrap_lines,
                        "Unwrap hard line breaks",
                    );
                    ui.checkbox(
                        &mut self.convert_books.heuristic_delete_blank_lines,
                        "Delete blank lines",
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Margins");
                    ui.add(
                        egui::DragValue::new(&mut self.convert_books.page_margin_left)
                            .speed(0.5)
                            .range(0.0..=40.0)
                            .prefix("L "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.convert_books.page_margin_right)
                            .speed(0.5)
                            .range(0.0..=40.0)
                            .prefix("R "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.convert_books.page_margin_top)
                            .speed(0.5)
                            .range(0.0..=40.0)
                            .prefix("T "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.convert_books.page_margin_bottom)
                            .speed(0.5)
                            .range(0.0..=40.0)
                            .prefix("B "),
                    );
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.convert_books.embed_fonts, "Embed fonts");
                    ui.checkbox(&mut self.convert_books.subset_fonts, "Subset fonts");
                    ui.label("Cover policy");
                    egui::ComboBox::from_id_salt("convert_cover_policy")
                        .selected_text(self.convert_books.cover_policy.as_str())
                        .show_ui(ui, |ui| {
                            for option in ["keep", "replace", "generate"] {
                                ui.selectable_value(
                                    &mut self.convert_books.cover_policy,
                                    option.to_string(),
                                    option,
                                );
                            }
                        });
                });
                egui::CollapsingHeader::new("Per-format options")
                    .default_open(true)
                    .show(ui, |ui| {
                        render_format_options(ui, "EPUB", &self.convert_books.output_format);
                        render_format_options(ui, "MOBI", &self.convert_books.output_format);
                        render_format_options(ui, "PDF", &self.convert_books.output_format);
                        render_format_options(ui, "AZW3", &self.convert_books.output_format);
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Preset name");
                    ui.text_edit_singleline(&mut self.convert_books.preset_name);
                    if ui.button("Save preset").clicked() {
                        save_preset = true;
                    }
                    let mut selected = String::new();
                    egui::ComboBox::from_id_salt("convert_load_preset")
                        .selected_text("Load preset")
                        .show_ui(ui, |ui| {
                            for key in self.convert_books.presets.keys() {
                                if ui.selectable_label(false, key).clicked() {
                                    selected = key.clone();
                                }
                            }
                        });
                    if !selected.is_empty() {
                        load_preset = Some(selected);
                    }
                });
                ui.checkbox(
                    &mut self.convert_books.warn_unsupported_options,
                    "Warn on unsupported options",
                );
                let warnings = conversion_warnings(&self.convert_books);
                if self.convert_books.warn_unsupported_options && !warnings.is_empty() {
                    ui.group(|ui| {
                        ui.strong("Warnings");
                        for warning in warnings {
                            ui.colored_label(egui::Color32::from_rgb(200, 130, 20), warning);
                        }
                    });
                }
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
        if save_preset {
            let preset_name = self.convert_books.preset_name.clone();
            self.convert_books.save_preset(&preset_name);
            self.push_toast("Saved conversion preset", ToastLevel::Info);
        }
        if let Some(name) = load_preset {
            if self.convert_books.load_preset(&name) {
                self.push_toast(&format!("Loaded preset: {name}"), ToastLevel::Info);
            }
        }
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
        let mut save_preset = false;
        let mut load_preset: Option<String> = None;
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
                ui.horizontal(|ui| {
                    ui.label("Path template");
                    ui.text_edit_singleline(&mut self.save_to_disk.path_template);
                });
                ui.horizontal(|ui| {
                    ui.label("Conflict policy");
                    egui::ComboBox::from_id_salt("save_to_disk_conflict_policy")
                        .selected_text(self.save_to_disk.conflict_policy.as_str())
                        .show_ui(ui, |ui| {
                            for policy in ["rename", "skip", "overwrite"] {
                                ui.selectable_value(
                                    &mut self.save_to_disk.conflict_policy,
                                    policy.to_string(),
                                    policy,
                                );
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("Preset name");
                    ui.text_edit_singleline(&mut self.save_to_disk.preset_name);
                    if ui.button("Save preset").clicked() {
                        save_preset = true;
                    }
                    let mut selected = String::new();
                    egui::ComboBox::from_id_salt("save_to_disk_load_preset")
                        .selected_text("Load preset")
                        .show_ui(ui, |ui| {
                            for key in self.save_to_disk.presets.keys() {
                                if ui.selectable_label(false, key).clicked() {
                                    selected = key.clone();
                                }
                            }
                        });
                    if !selected.is_empty() {
                        load_preset = Some(selected);
                    }
                });
                ui.collapsing("Export preview", |ui| {
                    let preview = self.build_export_preview(config, 10);
                    if preview.is_empty() {
                        ui.label("No selected files to preview.");
                    } else {
                        for line in preview {
                            ui.monospace(line);
                        }
                    }
                });
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
        if save_preset {
            let preset_name = self.save_to_disk.preset_name.clone();
            self.save_to_disk.save_preset(&preset_name);
            self.push_toast("Saved export preset", ToastLevel::Info);
        }
        if let Some(name) = load_preset {
            if self.save_to_disk.load_preset(&name) {
                self.push_toast(&format!("Loaded export preset: {name}"), ToastLevel::Info);
            }
        }
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
                if ui
                    .checkbox(
                        &mut self.device_sync.auto_convert,
                        "Auto convert before send",
                    )
                    .changed()
                {
                    self.config_dirty = true;
                }
                if ui
                    .checkbox(&mut self.device_sync.overwrite, "Overwrite existing files")
                    .changed()
                {
                    self.config_dirty = true;
                }
                if ui
                    .checkbox(&mut self.device_sync.sync_metadata, "Sync metadata")
                    .changed()
                {
                    self.config_dirty = true;
                }
                if ui
                    .checkbox(&mut self.device_sync.sync_cover, "Sync cover")
                    .changed()
                {
                    self.config_dirty = true;
                }
                ui.collapsing("Send queue", |ui| {
                    if self.device_sync.queue.is_empty() {
                        ui.label("Queue is empty.");
                    } else {
                        for row in &self.device_sync.queue {
                            ui.label(format!("{} — {}", row.item, row.status));
                        }
                    }
                });
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

    fn device_manager_dialog(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        if !self.device_manager.open {
            return;
        }
        let mut open = self.device_manager.open;
        egui::Window::new("Device manager")
            .open(&mut open)
            .resizable(true)
            .default_size(egui::vec2(900.0, 520.0))
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Refresh devices").clicked() {
                        self.device_manager.apply_defaults(config);
                        let _ = self.refresh_device_files(config);
                    }
                    ui.label("Driver");
                    egui::ComboBox::from_id_salt("device_driver_backend")
                        .selected_text(self.device_manager.driver_backend.as_str())
                        .show_ui(ui, |ui| {
                            for backend in ["auto", "usb", "mtp"] {
                                if ui
                                    .selectable_value(
                                        &mut self.device_manager.driver_backend,
                                        backend.to_string(),
                                        backend,
                                    )
                                    .changed()
                                {
                                    self.config_dirty = true;
                                }
                            }
                        });
                    ui.label("Timeout (ms)");
                    if ui
                        .add(
                            egui::DragValue::new(&mut self.device_manager.connection_timeout_ms)
                                .speed(100.0)
                                .range(100..=60_000),
                        )
                        .changed()
                    {
                        self.config_dirty = true;
                    }
                });
                if let Some(err) = &self.device_manager.last_scan_error {
                    ui.colored_label(egui::Color32::from_rgb(200, 60, 60), err);
                }
                ui.separator();
                ui.columns(2, |columns| {
                    columns[0].heading("Connected devices");
                    let mut selected_idx: Option<usize> = None;
                    for (idx, device) in self.device_manager.devices.iter().enumerate() {
                        if columns[0]
                            .selectable_label(
                                self.device_manager.selected_device == Some(idx),
                                device.name.clone(),
                            )
                            .clicked()
                        {
                            selected_idx = Some(idx);
                        }
                    }
                    if let Some(idx) = selected_idx {
                        self.device_manager.selected_device = Some(idx);
                        let _ = self.refresh_device_files(config);
                    }
                    columns[1].heading("Device view");
                    if let Some(device) = self.active_managed_device() {
                        columns[1].label(format!("Name: {}", device.name));
                        columns[1].label(format!("Mount: {}", device.mount_path.display()));
                        columns[1].label(format!("Library: {}", device.library_path.display()));
                        let (count, bytes) = self.device_storage_stats(&device);
                        columns[1].label(format!("Files: {count}"));
                        columns[1].label(format!("Used: {}", format_bytes(bytes)));
                        columns[1].separator();
                        columns[1].label("Collections (on-device shelves)");
                        for collection in &self.device_manager.collections {
                            columns[1].label(format!("• {collection}"));
                        }
                        columns[1].separator();
                        columns[1].label("Filters");
                        columns[1].text_edit_singleline(&mut self.device_manager.file_filter);
                        egui::ScrollArea::vertical().max_height(160.0).show(
                            &mut columns[1],
                            |ui| {
                                for file in self.filtered_device_files() {
                                    let label = file
                                        .file_name()
                                        .and_then(|name| name.to_str())
                                        .unwrap_or_default()
                                        .to_string();
                                    if ui
                                        .selectable_label(
                                            self.device_manager.selected_file.as_ref()
                                                == Some(&file),
                                            label,
                                        )
                                        .clicked()
                                    {
                                        self.device_manager.selected_file = Some(file.clone());
                                    }
                                }
                            },
                        );
                        columns[1].horizontal(|ui| {
                            if ui.button("Fetch into library").clicked() {
                                self.fetch_from_device.open = true;
                                self.fetch_from_device.file_path =
                                    self.device_manager.selected_file.clone();
                                self.fetch_from_device.mode = IngestMode::Copy;
                            }
                            if ui.button("Delete from device").clicked() {
                                self.device_file_delete.open = true;
                                self.device_file_delete.path =
                                    self.device_manager.selected_file.clone();
                            }
                            if ui.button("Cleanup orphans").clicked() {
                                if let Ok(removed) = cleanup_device_orphans(&device, &[]) {
                                    self.push_toast(
                                        &format!("Removed {removed} device files"),
                                        ToastLevel::Warn,
                                    );
                                    let _ = self.refresh_device_files(config);
                                }
                            }
                        });
                        columns[1].separator();
                        ui_collapsing_troubleshooting(&mut columns[1], &device);
                    } else {
                        columns[1].label("No device selected.");
                    }
                });
            });
        self.device_manager.open = open;
    }

    fn fetch_from_device_dialog(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        if !self.fetch_from_device.open {
            return;
        }
        let mut open = self.fetch_from_device.open;
        let mut confirmed = false;
        egui::Window::new("Fetch from device")
            .open(&mut open)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                if let Some(path) = &self.fetch_from_device.file_path {
                    ui.label(path.display().to_string());
                } else {
                    ui.label("No file selected.");
                }
                ui.horizontal(|ui| {
                    ui.label("Ingest mode");
                    ui.selectable_value(&mut self.fetch_from_device.mode, IngestMode::Copy, "Copy");
                    ui.selectable_value(
                        &mut self.fetch_from_device.mode,
                        IngestMode::Reference,
                        "Reference",
                    );
                });
                if ui.button("Import").clicked() {
                    confirmed = true;
                }
            });
        if confirmed {
            if let Some(path) = self.fetch_from_device.file_path.clone() {
                match self.run_fetch_from_device(config, &path, self.fetch_from_device.mode) {
                    Ok(()) => {
                        self.push_toast("Imported from device", ToastLevel::Info);
                        open = false;
                    }
                    Err(err) => self.set_error(err),
                }
            }
        }
        self.fetch_from_device.open = open;
    }

    fn device_file_delete_dialog(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        if !self.device_file_delete.open {
            return;
        }
        let mut open = self.device_file_delete.open;
        let mut confirmed = false;
        egui::Window::new("Delete device file")
            .open(&mut open)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                if let Some(path) = &self.device_file_delete.path {
                    ui.label(format!("Delete {} ?", path.display()));
                } else {
                    ui.label("No file selected.");
                }
                if ui.button("Delete").clicked() {
                    confirmed = true;
                }
            });
        if confirmed {
            if let Some(path) = self.device_file_delete.path.clone() {
                match fs::remove_file(&path) {
                    Ok(()) => {
                        self.push_toast("Deleted device file", ToastLevel::Warn);
                        let _ = self.refresh_device_files(config);
                        open = false;
                    }
                    Err(err) => {
                        self.set_error(CoreError::Io("delete device file".to_string(), err))
                    }
                }
            }
        }
        self.device_file_delete.open = open;
    }

    fn news_dialog(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        if !self.news_manager.open {
            return;
        }
        let mut open = self.news_manager.open;
        let mut download_now = false;
        let mut import_recipe = false;
        let mut retry_selected = false;
        let mut open_reader_selected = false;
        egui::Window::new("News")
            .open(&mut open)
            .resizable(true)
            .default_size(egui::vec2(980.0, 560.0))
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Refresh sources").clicked() {
                        let _ = self.refresh_news_sources(config);
                        let _ = self.refresh_news_downloads(config);
                    }
                    if ui.button("Download").clicked() {
                        download_now = true;
                    }
                    if ui.button("Retry selected").clicked() {
                        retry_selected = true;
                    }
                    if ui.button("Open selected in reader").clicked() {
                        open_reader_selected = true;
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Filter");
                    ui.text_edit_singleline(&mut self.news_manager.source_filter);
                    ui.label("Recipe import path");
                    ui.text_edit_singleline(&mut self.news_manager.recipe_import_path);
                    if ui.button("Import recipe").clicked() {
                        import_recipe = true;
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .checkbox(
                            &mut self.news_manager.auto_delete,
                            "Auto-delete old downloads",
                        )
                        .changed()
                    {
                        self.config_dirty = true;
                    }
                    ui.label("Retention days");
                    if ui
                        .add(
                            egui::DragValue::new(&mut self.news_manager.retention_days)
                                .speed(1.0)
                                .range(1..=3650),
                        )
                        .changed()
                    {
                        self.config_dirty = true;
                    }
                });
                ui.separator();
                let source_filter = self.news_manager.source_filter.to_lowercase();
                ui.columns(3, |columns| {
                    columns[0].heading("Sources");
                    for (idx, source) in
                        self.news_manager
                            .sources
                            .iter_mut()
                            .enumerate()
                            .filter(|(_, source)| {
                                source_filter.is_empty()
                                    || source.name.to_lowercase().contains(&source_filter)
                            })
                    {
                        columns[0].horizontal(|ui| {
                            if ui
                                .selectable_label(
                                    self.news_manager.selected_source == Some(idx),
                                    source.name.clone(),
                                )
                                .clicked()
                            {
                                self.news_manager.selected_source = Some(idx);
                            }
                            if ui.checkbox(&mut source.enabled, "").changed() {
                                self.config_dirty = true;
                            }
                        });
                        columns[0].small(format!(
                            "Schedule: {}  Status: {}",
                            source.schedule, source.status
                        ));
                        columns[0].separator();
                    }
                    columns[1].heading("Downloads");
                    egui::ScrollArea::vertical()
                        .max_height(320.0)
                        .show(&mut columns[1], |ui| {
                            for (idx, row) in self.news_manager.downloads.iter().enumerate() {
                                let label = format!(
                                    "{} [{}]",
                                    row.path
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("item"),
                                    row.status
                                );
                                if ui
                                    .selectable_label(
                                        self.news_manager.selected_download == Some(idx),
                                        label,
                                    )
                                    .clicked()
                                {
                                    self.news_manager.selected_download = Some(idx);
                                }
                                ui.small(format!("source: {}", row.source));
                            }
                        });
                    columns[2].heading("History + logs");
                    egui::ScrollArea::vertical()
                        .max_height(320.0)
                        .show(&mut columns[2], |ui| {
                            for line in &self.news_manager.history_lines {
                                ui.monospace(line);
                            }
                        });
                });
            });
        if import_recipe {
            match self.import_news_recipe(config) {
                Ok(()) => {
                    self.push_toast("Recipe imported", ToastLevel::Info);
                    let _ = self.refresh_news_sources(config);
                }
                Err(err) => self.set_error(err),
            }
        }
        if download_now {
            if let Err(err) = self.run_news_download(config) {
                self.set_error(err);
            } else {
                let _ = self.refresh_news_downloads(config);
                let _ = self.load_news_history(config);
            }
        }
        if retry_selected {
            if let Err(err) = self.retry_news_download(config) {
                self.set_error(err);
            } else {
                let _ = self.refresh_news_downloads(config);
            }
        }
        if open_reader_selected {
            if let Err(err) = self.open_selected_news_in_reader() {
                self.set_error(err);
            }
        }
        self.news_manager.open = open;
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
        let mut merge = false;
        let mut delete = false;
        let mut bulk_assign = false;
        let mut bulk_remove = false;
        egui::Window::new("Manage tags")
            .open(&mut open)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                ui.label("Tags");
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        let grouped = group_hierarchical_categories(&self.manage_tags.tags, '/');
                        for (root, entries) in grouped {
                            ui.collapsing(root, |ui| {
                                for entry in entries {
                                    ui.label(format!("{} ({})", entry.name, entry.count));
                                }
                            });
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
                ui.label("Merge tags");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.manage_tags.merge_from);
                    ui.label("→");
                    ui.text_edit_singleline(&mut self.manage_tags.merge_to);
                });
                if ui.button("Merge").clicked() {
                    merge = true;
                }
                ui.separator();
                ui.label("Delete tag");
                ui.text_edit_singleline(&mut self.manage_tags.delete_name);
                if ui.button("Delete").clicked() {
                    delete = true;
                }
                ui.separator();
                ui.label("Bulk assign/remove on selected books");
                ui.text_edit_singleline(&mut self.manage_tags.bulk_tag);
                ui.horizontal(|ui| {
                    if ui.button("Assign to selected").clicked() {
                        bulk_assign = true;
                    }
                    if ui.button("Remove from selected").clicked() {
                        bulk_remove = true;
                    }
                });
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
        if merge {
            if let Err(err) = self
                .db
                .rename_tag(&self.manage_tags.merge_from, &self.manage_tags.merge_to)
            {
                self.set_error(err);
            } else {
                self.manage_tags.needs_refresh = true;
                self.needs_refresh = true;
            }
        }
        if bulk_assign {
            let tag = self.manage_tags.bulk_tag.clone();
            if let Err(err) = self.bulk_assign_tag(tag.trim()) {
                self.set_error(err);
            }
        }
        if bulk_remove {
            let tag = self.manage_tags.bulk_tag.clone();
            if let Err(err) = self.bulk_remove_tag(tag.trim()) {
                self.set_error(err);
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
        let mut merge = false;
        let mut delete = false;
        let mut renumber = false;
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
                ui.label("Merge series");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.manage_series.merge_from);
                    ui.label("→");
                    ui.text_edit_singleline(&mut self.manage_series.merge_to);
                });
                if ui.button("Merge").clicked() {
                    merge = true;
                }
                ui.separator();
                ui.label("Delete series");
                ui.text_edit_singleline(&mut self.manage_series.delete_name);
                if ui.button("Delete").clicked() {
                    delete = true;
                }
                ui.separator();
                ui.label("Renumber selected books in series");
                ui.horizontal(|ui| {
                    ui.label("Series");
                    ui.text_edit_singleline(&mut self.manage_series.renumber_name);
                });
                ui.horizontal(|ui| {
                    ui.label("Start");
                    ui.add(egui::DragValue::new(&mut self.manage_series.renumber_start).speed(0.1));
                    ui.label("Step");
                    ui.add(egui::DragValue::new(&mut self.manage_series.renumber_step).speed(0.1));
                });
                if ui.button("Renumber selected").clicked() {
                    renumber = true;
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
        if merge {
            if let Err(err) = self
                .db
                .rename_series(&self.manage_series.merge_from, &self.manage_series.merge_to)
            {
                self.set_error(err);
            } else {
                self.manage_series.needs_refresh = true;
                self.needs_refresh = true;
            }
        }
        if renumber {
            let series_name = self.manage_series.renumber_name.clone();
            let start = self.manage_series.renumber_start;
            let step = self.manage_series.renumber_step;
            if let Err(err) = self.renumber_selected_series(series_name.trim(), start, step) {
                self.set_error(err);
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
        let mut save = false;
        let mut export = false;
        let mut import = false;
        egui::Window::new("Manage custom columns")
            .open(&mut open)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                ui.label("Custom columns");
                egui::ScrollArea::vertical()
                    .max_height(160.0)
                    .show(ui, |ui| {
                        for column in self.manage_custom_columns.columns.clone() {
                            let selected =
                                self.manage_custom_columns.edit_label.trim() == column.label;
                            let label =
                                format!("{} ({}, {})", column.label, column.name, column.datatype);
                            if ui.selectable_label(selected, label).clicked() {
                                self.select_custom_column_for_edit(&column.label);
                            }
                        }
                    });
                ui.separator();
                ui.label("Create column");
                ui.label("Label");
                ui.text_edit_singleline(&mut self.manage_custom_columns.new_label);
                ui.label("Name");
                ui.text_edit_singleline(&mut self.manage_custom_columns.new_name);
                egui::ComboBox::from_id_salt("custom_column_datatype")
                    .selected_text(self.manage_custom_columns.new_datatype.as_str())
                    .show_ui(ui, |ui| {
                        for datatype in ["text", "int", "float", "bool", "date", "series"] {
                            ui.selectable_value(
                                &mut self.manage_custom_columns.new_datatype,
                                datatype.to_string(),
                                datatype,
                            );
                        }
                    });
                ui.label("Display JSON");
                ui.text_edit_singleline(&mut self.manage_custom_columns.new_display);
                if ui.button("Create").clicked() {
                    create = true;
                }
                ui.separator();
                ui.label("Edit selected column");
                if self.manage_custom_columns.edit_label.trim().is_empty() {
                    if let Some(label) = self.selected_column_label() {
                        self.select_custom_column_for_edit(&label);
                    }
                }
                ui.horizontal(|ui| {
                    ui.label("Label");
                    ui.monospace(self.manage_custom_columns.edit_label.clone());
                });
                ui.label("Name");
                ui.text_edit_singleline(&mut self.manage_custom_columns.edit_name);
                egui::ComboBox::from_id_salt("custom_column_edit_datatype")
                    .selected_text(self.manage_custom_columns.edit_datatype.as_str())
                    .show_ui(ui, |ui| {
                        for datatype in ["text", "int", "float", "bool", "date", "series"] {
                            ui.selectable_value(
                                &mut self.manage_custom_columns.edit_datatype,
                                datatype.to_string(),
                                datatype,
                            );
                        }
                    });
                ui.label("Display JSON");
                ui.text_edit_singleline(&mut self.manage_custom_columns.edit_display);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.manage_custom_columns.edit_editable, "Editable");
                    ui.checkbox(
                        &mut self.manage_custom_columns.edit_is_multiple,
                        "Multiple values",
                    );
                    ui.checkbox(
                        &mut self.manage_custom_columns.edit_normalized,
                        "Normalized",
                    );
                });
                if ui.button("Save edits").clicked() {
                    save = true;
                }
                ui.separator();
                ui.label("Delete column (label)");
                ui.text_edit_singleline(&mut self.manage_custom_columns.delete_label);
                if ui.button("Delete").clicked() {
                    delete = true;
                }
                ui.separator();
                ui.label("Import/Export custom columns");
                ui.text_edit_singleline(&mut self.manage_custom_columns.import_path);
                if ui.button("Import columns").clicked() {
                    import = true;
                }
                ui.text_edit_singleline(&mut self.manage_custom_columns.export_path);
                if ui.button("Export columns").clicked() {
                    export = true;
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
        if save {
            if let Err(err) = self.save_custom_column_edits() {
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
        if export {
            if let Err(err) =
                self.export_custom_columns(Path::new(self.manage_custom_columns.export_path.trim()))
            {
                self.set_error(err);
            }
        }
        if import {
            if let Err(err) =
                self.import_custom_columns(Path::new(self.manage_custom_columns.import_path.trim()))
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
        let mut build_query = false;
        let mut export = false;
        let mut import = false;
        let mut assign = false;
        let mut unassign = false;
        egui::Window::new("Manage virtual libraries")
            .open(&mut open)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                ui.label("Saved searches");
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        let grouped = self.build_saved_search_groups();
                        for (folder, entries) in grouped {
                            ui.collapsing(folder, |ui| {
                                for (name, query) in entries {
                                    ui.label(format!("{name}: {query}"));
                                }
                            });
                        }
                    });
                ui.separator();
                ui.label("Add saved search");
                ui.text_edit_singleline(&mut self.manage_virtual_libraries.new_name);
                ui.text_edit_singleline(&mut self.manage_virtual_libraries.new_folder);
                ui.text_edit_singleline(&mut self.manage_virtual_libraries.new_query);
                if ui.button("Add").clicked() {
                    add = true;
                }
                ui.horizontal(|ui| {
                    ui.label("Builder");
                    egui::ComboBox::from_id_salt("vl_query_field")
                        .selected_text(self.manage_virtual_libraries.builder_field.as_str())
                        .show_ui(ui, |ui| {
                            for field in [
                                "title",
                                "authors",
                                "tags",
                                "series",
                                "publisher",
                                "languages",
                            ] {
                                ui.selectable_value(
                                    &mut self.manage_virtual_libraries.builder_field,
                                    field.to_string(),
                                    field,
                                );
                            }
                        });
                    egui::ComboBox::from_id_salt("vl_query_op")
                        .selected_text(self.manage_virtual_libraries.builder_op.as_str())
                        .show_ui(ui, |ui| {
                            for op in ["contains", "is", "not"] {
                                ui.selectable_value(
                                    &mut self.manage_virtual_libraries.builder_op,
                                    op.to_string(),
                                    op,
                                );
                            }
                        });
                    ui.text_edit_singleline(&mut self.manage_virtual_libraries.builder_value);
                    if ui.button("Append").clicked() {
                        build_query = true;
                    }
                });
                ui.separator();
                ui.label("Remove saved search");
                ui.text_edit_singleline(&mut self.manage_virtual_libraries.delete_name);
                if ui.button("Remove").clicked() {
                    delete = true;
                }
                ui.separator();
                ui.label("Import/Export saved searches");
                ui.text_edit_singleline(&mut self.manage_virtual_libraries.import_path);
                if ui.button("Import").clicked() {
                    import = true;
                }
                ui.text_edit_singleline(&mut self.manage_virtual_libraries.export_path);
                if ui.button("Export").clicked() {
                    export = true;
                }
                ui.separator();
                ui.label("Assign/Unassign virtual library tag on selected books");
                ui.text_edit_singleline(&mut self.manage_virtual_libraries.assign_name);
                if ui.button("Assign").clicked() {
                    assign = true;
                }
                ui.text_edit_singleline(&mut self.manage_virtual_libraries.unassign_name);
                if ui.button("Unassign").clicked() {
                    unassign = true;
                }
            });
        if add {
            let folder = self.manage_virtual_libraries.new_folder.trim();
            let name = self.manage_virtual_libraries.new_name.trim();
            let full_name = if folder.is_empty() {
                name.to_string()
            } else {
                format!("{folder}/{name}")
            };
            if let Err(err) = self
                .db
                .add_saved_search(&full_name, &self.manage_virtual_libraries.new_query)
            {
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
        if build_query {
            self.append_query_builder_clause();
        }
        if export {
            if let Err(err) = self
                .export_saved_searches(Path::new(self.manage_virtual_libraries.export_path.trim()))
            {
                self.set_error(err);
            }
        }
        if import {
            if let Err(err) = self
                .import_saved_searches(Path::new(self.manage_virtual_libraries.import_path.trim()))
            {
                self.set_error(err);
            } else {
                self.manage_virtual_libraries.needs_refresh = true;
            }
        }
        if assign {
            let name = self.manage_virtual_libraries.assign_name.clone();
            if let Err(err) = self.assign_virtual_library_tag(name.trim()) {
                self.set_error(err);
            }
        }
        if unassign {
            let name = self.manage_virtual_libraries.unassign_name.clone();
            if let Err(err) = self.unassign_virtual_library_tag(name.trim()) {
                self.set_error(err);
            }
        }
        self.manage_virtual_libraries.open = open;
    }

    fn plugins_dialog(&mut self, ui: &mut egui::Ui) {
        if !self.plugins.open {
            return;
        }
        let mut open = self.plugins.open;
        let mut check_updates = false;
        let mut install = false;
        let mut remove = false;
        egui::Window::new("Plugins")
            .open(&mut open)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Filter");
                    ui.text_edit_singleline(&mut self.plugins.search);
                    if ui.button("Check updates").clicked() {
                        check_updates = true;
                    }
                });
                ui.separator();
                let filter = self.plugins.search.trim().to_lowercase();
                egui::ScrollArea::vertical()
                    .max_height(180.0)
                    .show(ui, |ui| {
                        for plugin in &mut self.plugins.plugins {
                            if !filter.is_empty()
                                && !plugin.name.to_lowercase().contains(&filter)
                                && !plugin.id.to_lowercase().contains(&filter)
                            {
                                continue;
                            }
                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(
                                        self.plugins.selected.as_deref()
                                            == Some(plugin.id.as_str()),
                                        format!("{} ({})", plugin.name, plugin.id),
                                    )
                                    .clicked()
                                {
                                    self.plugins.selected = Some(plugin.id.clone());
                                    self.plugins.remove_id = plugin.id.clone();
                                }
                                ui.checkbox(&mut plugin.enabled, "enabled");
                            });
                        }
                    });
                if let Some(selected) = self.plugins.selected.clone() {
                    let installed_ids = self
                        .plugins
                        .plugins
                        .iter()
                        .map(|entry| entry.id.clone())
                        .collect::<HashSet<_>>();
                    if let Some(plugin) = self.plugins.plugins.iter_mut().find(|p| p.id == selected)
                    {
                        ui.separator();
                        ui.label(format!("Version: {}", plugin.version));
                        ui.label(format!("Latest: {}", plugin.latest_version));
                        ui.label(format!("Author: {}", plugin.author));
                        ui.label(format!("Description: {}", plugin.description));
                        ui.label(format!("Status: {}", plugin.status));
                        if let Some(err) = &plugin.error {
                            ui.colored_label(egui::Color32::from_rgb(180, 40, 40), err);
                        }
                        if !plugin.dependencies.is_empty() {
                            let missing = plugin
                                .dependencies
                                .iter()
                                .filter(|dep| !installed_ids.contains(*dep))
                                .cloned()
                                .collect::<Vec<_>>();
                            ui.label(format!("Dependencies: {}", plugin.dependencies.join(", ")));
                            if !missing.is_empty() {
                                ui.colored_label(
                                    egui::Color32::from_rgb(180, 80, 20),
                                    format!("Conflict: missing {}", missing.join(", ")),
                                );
                            }
                        }
                        ui.separator();
                        ui.label("Plugin settings");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut plugin.setting_key);
                            ui.text_edit_singleline(&mut plugin.setting_value);
                            if ui.button("Apply setting").clicked() {
                                plugin.logs.push(format!(
                                    "set {}={}",
                                    plugin.setting_key, plugin.setting_value
                                ));
                                plugin.status = "settings updated".to_string();
                            }
                        });
                        egui::CollapsingHeader::new("Plugin logs")
                            .default_open(false)
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical()
                                    .max_height(100.0)
                                    .show(ui, |ui| {
                                        for line in &plugin.logs {
                                            ui.monospace(line);
                                        }
                                    });
                            });
                    }
                }
                ui.separator();
                ui.label("Install plugin");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.plugins.install_id);
                    ui.text_edit_singleline(&mut self.plugins.install_name);
                    ui.text_edit_singleline(&mut self.plugins.install_version);
                });
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.plugins.install_author);
                    ui.text_edit_singleline(&mut self.plugins.install_description);
                });
                if ui.button("Install").clicked() {
                    install = true;
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Remove id");
                    ui.text_edit_singleline(&mut self.plugins.remove_id);
                    if ui.button("Remove").clicked() {
                        remove = true;
                    }
                });
                if !self.plugins.status_message.is_empty() {
                    ui.label(self.plugins.status_message.clone());
                }
            });
        if check_updates {
            for plugin in &mut self.plugins.plugins {
                if plugin.latest_version > plugin.version {
                    plugin.status = "update available".to_string();
                    plugin.logs.push(format!(
                        "update available: {} -> {}",
                        plugin.version, plugin.latest_version
                    ));
                } else {
                    plugin.status = "up to date".to_string();
                    plugin.logs.push("already up to date".to_string());
                }
            }
            self.plugins.status_message = "Update check completed".to_string();
            info!(component = "gui", "checked plugin updates");
        }
        if install {
            let id = self.plugins.install_id.trim().to_string();
            let name = self.plugins.install_name.trim().to_string();
            if !id.is_empty() && !name.is_empty() {
                self.plugins.plugins.push(PluginEntry {
                    id: id.clone(),
                    name,
                    version: self.plugins.install_version.trim().to_string(),
                    latest_version: self.plugins.install_version.trim().to_string(),
                    author: self.plugins.install_author.trim().to_string(),
                    description: self.plugins.install_description.trim().to_string(),
                    enabled: true,
                    dependencies: Vec::new(),
                    status: "installed".to_string(),
                    error: None,
                    setting_key: "enabled".to_string(),
                    setting_value: "true".to_string(),
                    logs: vec!["plugin installed".to_string()],
                });
                self.plugins.selected = Some(id.clone());
                self.plugins.remove_id = id;
                self.plugins.status_message = "Plugin installed".to_string();
                info!(component = "gui", "installed plugin");
            }
        }
        if remove {
            let remove_id = self.plugins.remove_id.trim().to_string();
            let before = self.plugins.plugins.len();
            self.plugins.plugins.retain(|entry| entry.id != remove_id);
            if self.plugins.plugins.len() != before {
                self.plugins.status_message = "Plugin removed".to_string();
                self.plugins.selected = self.plugins.plugins.first().map(|entry| entry.id.clone());
                info!(component = "gui", "removed plugin");
            } else {
                self.plugins.status_message = "Plugin id not found".to_string();
            }
        }
        self.plugins.open = open;
    }

    fn metadata_download_dialog(&mut self, ui: &mut egui::Ui) {
        if !self.metadata_download.open {
            return;
        }
        let mut open = self.metadata_download.open;
        let mut close_requested = false;
        let enabled_sources =
            active_sources_for_dialog(&self.metadata_download_config, &self.metadata_download);
        if enabled_sources.is_empty() {
            self.metadata_download.last_error =
                Some("No metadata providers enabled in config.".to_string());
        } else if !enabled_sources.contains(&self.metadata_download.source) {
            self.metadata_download.source = enabled_sources[0].clone();
        }
        egui::Window::new(if self.metadata_download.cover_only {
            "Download Cover"
        } else {
            "Download Metadata"
        })
        .open(&mut open)
        .resizable(true)
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                ui.label("Source");
                egui::ComboBox::from_id_salt("metadata_download_source")
                    .selected_text(self.metadata_download.source.clone())
                    .show_ui(ui, |ui| {
                        for source in &enabled_sources {
                            ui.selectable_value(
                                &mut self.metadata_download.source,
                                source.clone(),
                                source,
                            );
                        }
                    });
                ui.checkbox(
                    &mut self.metadata_download.merge_mode,
                    "Merge instead of replace",
                );
            });
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut self.metadata_download.source_openlibrary,
                    "OpenLibrary",
                );
                ui.checkbox(&mut self.metadata_download.source_google, "GoogleBooks");
                ui.checkbox(&mut self.metadata_download.source_amazon, "Amazon");
                ui.checkbox(&mut self.metadata_download.source_isbndb, "ISBNdb");
            });
            egui::CollapsingHeader::new("Merge rules")
                .default_open(false)
                .show(ui, |ui| {
                    ui.checkbox(&mut self.metadata_download.merge_tags, "Merge tags");
                    ui.checkbox(
                        &mut self.metadata_download.merge_identifiers,
                        "Merge identifiers",
                    );
                    ui.checkbox(
                        &mut self.metadata_download.overwrite_title,
                        "Overwrite title",
                    );
                    ui.checkbox(
                        &mut self.metadata_download.overwrite_authors,
                        "Overwrite authors",
                    );
                    ui.checkbox(
                        &mut self.metadata_download.overwrite_publisher,
                        "Overwrite publisher",
                    );
                    ui.checkbox(
                        &mut self.metadata_download.overwrite_language,
                        "Overwrite language",
                    );
                    ui.checkbox(
                        &mut self.metadata_download.overwrite_pubdate,
                        "Overwrite publication date",
                    );
                    ui.checkbox(
                        &mut self.metadata_download.overwrite_comment,
                        "Overwrite comments",
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Preset: Conservative").clicked() {
                            self.metadata_download.merge_mode = true;
                            self.metadata_download.merge_tags = true;
                            self.metadata_download.merge_identifiers = true;
                            self.metadata_download.overwrite_title = false;
                            self.metadata_download.overwrite_authors = false;
                            self.metadata_download.overwrite_publisher = false;
                            self.metadata_download.overwrite_language = false;
                            self.metadata_download.overwrite_pubdate = false;
                            self.metadata_download.overwrite_comment = false;
                        }
                        if ui.button("Preset: Balanced").clicked() {
                            self.metadata_download.merge_mode = true;
                            self.metadata_download.merge_tags =
                                self.metadata_download_config.merge_tags_default;
                            self.metadata_download.merge_identifiers =
                                self.metadata_download_config.merge_identifiers_default;
                            self.metadata_download.overwrite_title =
                                self.metadata_download_config.overwrite_title_default;
                            self.metadata_download.overwrite_authors =
                                self.metadata_download_config.overwrite_authors_default;
                            self.metadata_download.overwrite_publisher =
                                self.metadata_download_config.overwrite_publisher_default;
                            self.metadata_download.overwrite_language =
                                self.metadata_download_config.overwrite_language_default;
                            self.metadata_download.overwrite_pubdate =
                                self.metadata_download_config.overwrite_pubdate_default;
                            self.metadata_download.overwrite_comment =
                                self.metadata_download_config.overwrite_comment_default;
                        }
                        if ui.button("Preset: Replace").clicked() {
                            self.metadata_download.merge_mode = false;
                            self.metadata_download.merge_tags = false;
                            self.metadata_download.merge_identifiers = false;
                            self.metadata_download.overwrite_title = true;
                            self.metadata_download.overwrite_authors = true;
                            self.metadata_download.overwrite_publisher = true;
                            self.metadata_download.overwrite_language = true;
                            self.metadata_download.overwrite_pubdate = true;
                            self.metadata_download.overwrite_comment = true;
                        }
                    });
                });
            if !self.metadata_download.cover_only {
                ui.horizontal(|ui| {
                    if ui.button("Queue selected").clicked() {
                        self.queue_selected_for_download();
                    }
                    if ui.button("Run queue").clicked() {
                        self.run_metadata_download_queue();
                    }
                    if ui.button("Clear queue").clicked() {
                        self.metadata_download.queue_rows.clear();
                        self.metadata_download.progress = 0.0;
                    }
                });
                ui.separator();
                ui.label("Queue");
                if self.metadata_download.queue_rows.is_empty() {
                    ui.label("No queued books.");
                } else {
                    for row in &self.metadata_download.queue_rows {
                        ui.horizontal(|ui| {
                            ui.label(format!("#{}", row.book_id));
                            ui.label(row.title.clone());
                            ui.monospace(row.status.label());
                            if let Some(err) = &row.error {
                                ui.colored_label(egui::Color32::from_rgb(190, 0, 0), err);
                            }
                            if ui.small_button("Select").clicked() {
                                self.metadata_download.selected_book_id = Some(row.book_id);
                            }
                        });
                    }
                }
            }
            ui.add(
                egui::ProgressBar::new(self.metadata_download.progress)
                    .show_percentage()
                    .text("Queue progress"),
            );
            if self.metadata_download.failed {
                ui.colored_label(egui::Color32::from_rgb(190, 0, 0), "Last fetch failed");
            }
            if let Some(message) = &self.metadata_download.last_error {
                ui.colored_label(egui::Color32::from_rgb(190, 0, 0), message);
            }
            ui.horizontal(|ui| {
                if ui.button("Retry failed").clicked() {
                    self.retry_failed_metadata_rows();
                }
            });
            ui.separator();
            ui.label("Results comparison");
            for result in &self.metadata_download.results {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(
                            self.metadata_download.selected_book_id == Some(result.book_id),
                            format!("#{} {}", result.book_id, result.provider),
                        )
                        .clicked()
                    {
                        self.metadata_download.selected_book_id = Some(result.book_id);
                    }
                    ui.label(result.preview_line());
                });
            }
            ui.separator();
            ui.label("Cover chooser");
            ui.horizontal_wrapped(|ui| {
                let mut count = 0usize;
                for result in &self.metadata_download.results {
                    if let Some(cover_url) = &result.metadata.cover_url {
                        count += 1;
                        if ui
                            .button(format!("{} ({})", count, result.provider))
                            .clicked()
                        {
                            self.metadata_download.selected_cover = count;
                            self.metadata_download.selected_cover_url = Some(cover_url.clone());
                            self.metadata_download.selected_book_id = Some(result.book_id);
                        }
                    }
                }
                if count == 0 {
                    ui.label("No downloaded covers.");
                }
            });
            ui.label(format!(
                "Selected cover: {}",
                self.metadata_download.selected_cover
            ));
            ui.horizontal(|ui| {
                if ui.button("Apply metadata").clicked() {
                    self.apply_metadata_download_result();
                }
                if ui.button("Apply cover").clicked() {
                    self.apply_downloaded_cover();
                }
                if ui.button("Close").clicked() {
                    close_requested = true;
                }
            });
        });
        if close_requested {
            open = false;
        }
        self.metadata_download.open = open;
    }

    fn queue_selected_for_download(&mut self) {
        let selected = self.selected_ids.clone();
        if selected.is_empty() {
            self.push_toast(
                "Select at least one book to queue metadata download",
                ToastLevel::Warn,
            );
            return;
        }
        for book_id in selected
            .iter()
            .take(self.metadata_download_config.queue_batch_size)
        {
            if self
                .metadata_download
                .queue_rows
                .iter()
                .any(|row| row.book_id == *book_id)
            {
                continue;
            }
            let title = self
                .all_books
                .iter()
                .find(|row| row.id == *book_id)
                .map(|row| row.title.clone())
                .unwrap_or_else(|| "Unknown".to_string());
            self.metadata_download.queue_rows.push(MetadataQueueRow {
                book_id: *book_id,
                title,
                status: MetadataQueueStatus::Pending,
                error: None,
            });
        }
        self.metadata_download.selected_book_id = selected.first().copied();
        self.metadata_download.progress = 0.0;
        self.push_toast(
            "Queued selected books for metadata download",
            ToastLevel::Info,
        );
    }

    fn run_metadata_download_queue(&mut self) {
        if self.metadata_download.queue_rows.is_empty() {
            self.push_toast("Queue is empty", ToastLevel::Warn);
            return;
        }
        let total = self.metadata_download.queue_rows.len() as f32;
        let mut completed = 0f32;
        self.metadata_download.results.clear();
        self.metadata_download.failed = false;
        self.metadata_download.last_error = None;
        for idx in 0..self.metadata_download.queue_rows.len() {
            let book_id = self.metadata_download.queue_rows[idx].book_id;
            self.metadata_download.queue_rows[idx].status = MetadataQueueStatus::Running;
            match self.fetch_metadata_for_book(book_id) {
                Ok(result) => {
                    self.metadata_download.queue_rows[idx].status = MetadataQueueStatus::Success;
                    self.metadata_download.queue_rows[idx].error = None;
                    self.metadata_download.results.push(result);
                }
                Err(err) => {
                    self.metadata_download.queue_rows[idx].status = MetadataQueueStatus::Failed;
                    self.metadata_download.queue_rows[idx].error = Some(err.to_string());
                    self.metadata_download.failed = true;
                    self.metadata_download.last_error = Some(err.to_string());
                    warn!(
                        component = "gui",
                        book_id,
                        error = %err,
                        "metadata download failed"
                    );
                }
            }
            completed += 1.0;
            self.metadata_download.progress = completed / total;
        }
        if self.metadata_download.results.is_empty() {
            self.push_toast("No metadata results downloaded", ToastLevel::Warn);
        } else {
            self.push_toast("Metadata download queue completed", ToastLevel::Info);
        }
    }

    fn retry_failed_metadata_rows(&mut self) {
        let mut retried = 0usize;
        for idx in 0..self.metadata_download.queue_rows.len() {
            if self.metadata_download.queue_rows[idx].status != MetadataQueueStatus::Failed {
                continue;
            }
            let book_id = self.metadata_download.queue_rows[idx].book_id;
            self.metadata_download.queue_rows[idx].status = MetadataQueueStatus::Running;
            match self.fetch_metadata_for_book(book_id) {
                Ok(result) => {
                    self.metadata_download.queue_rows[idx].status = MetadataQueueStatus::Success;
                    self.metadata_download.queue_rows[idx].error = None;
                    self.metadata_download
                        .results
                        .retain(|entry| entry.book_id != book_id);
                    self.metadata_download.results.push(result);
                }
                Err(err) => {
                    self.metadata_download.queue_rows[idx].status = MetadataQueueStatus::Failed;
                    self.metadata_download.queue_rows[idx].error = Some(err.to_string());
                    self.metadata_download.last_error = Some(err.to_string());
                }
            }
            retried += 1;
        }
        if retried > 0 {
            self.push_toast("Retried failed metadata queue rows", ToastLevel::Info);
        }
        self.metadata_download.failed = self
            .metadata_download
            .queue_rows
            .iter()
            .any(|row| row.status == MetadataQueueStatus::Failed);
    }

    fn fetch_metadata_for_book(&mut self, book_id: i64) -> CoreResult<MetadataDownloadResult> {
        let details = self.load_details_for_download(book_id)?;
        let source = if self.metadata_download.source.trim().is_empty() {
            first_enabled_source(&self.metadata_download_config)
                .ok_or_else(|| CoreError::ConfigValidate("no provider configured".to_string()))?
        } else {
            self.metadata_download.source.clone()
        };
        let active_sources =
            active_sources_for_dialog(&self.metadata_download_config, &self.metadata_download);
        if !active_sources.iter().any(|entry| entry == &source) {
            return Err(CoreError::ConfigValidate(format!(
                "source '{source}' is disabled in dialog or config"
            )));
        }
        let provider_config = to_provider_config(&self.metadata_download_config);
        let query = MetadataQuery {
            title: details.book.title.clone(),
            authors: details.authors.clone(),
            isbn: first_identifier_from_details(&details, "isbn"),
        };
        let metadata = fetch_metadata(&provider_config, &query, &source)?;
        let provider_name = metadata.provider.clone();
        Ok(MetadataDownloadResult {
            book_id,
            provider: provider_name,
            metadata,
        })
    }

    fn load_details_for_download(&mut self, book_id: i64) -> CoreResult<BookDetails> {
        if self.details.as_ref().map(|details| details.book.id) == Some(book_id) {
            if let Some(details) = &self.details {
                return Ok(details.clone());
            }
        }
        self.load_details(book_id)?;
        self.details
            .clone()
            .ok_or_else(|| CoreError::ConfigValidate("book details missing after load".to_string()))
    }

    fn apply_metadata_download_result(&mut self) {
        let Some(book_id) = self.metadata_download_target_book() else {
            self.push_toast(
                "Select a book before applying downloaded metadata",
                ToastLevel::Warn,
            );
            return;
        };
        let Some(result) = self
            .metadata_download
            .results
            .iter()
            .find(|entry| entry.book_id == book_id)
            .cloned()
        else {
            self.push_toast(
                "No downloaded metadata result for selected book",
                ToastLevel::Warn,
            );
            return;
        };
        if let Err(err) = self.apply_downloaded_metadata_to_book(book_id, &result.metadata) {
            self.set_error(err);
        } else {
            self.push_toast("Applied downloaded metadata", ToastLevel::Info);
        }
    }

    fn apply_downloaded_cover(&mut self) {
        let Some(book_id) = self.metadata_download_target_book() else {
            self.push_toast(
                "Select a book before applying downloaded cover",
                ToastLevel::Warn,
            );
            return;
        };
        let cover_url = self
            .metadata_download
            .selected_cover_url
            .clone()
            .or_else(|| {
                self.metadata_download
                    .results
                    .iter()
                    .find(|entry| entry.book_id == book_id)
                    .and_then(|entry| entry.metadata.cover_url.clone())
            });
        let Some(cover_url) = cover_url else {
            self.push_toast(
                "No downloaded cover available for selected book",
                ToastLevel::Warn,
            );
            return;
        };
        let provider_config = to_provider_config(&self.metadata_download_config);
        match fetch_cover(&provider_config, &cover_url) {
            Ok(cover) => {
                if let Err(err) = self.apply_cover_from_bytes(book_id, &cover.bytes) {
                    self.set_error(err);
                } else {
                    self.push_toast("Applied downloaded cover", ToastLevel::Info);
                    let _ = self.load_details(book_id);
                }
            }
            Err(err) => self.set_error(err),
        }
    }

    fn metadata_download_target_book(&self) -> Option<i64> {
        self.metadata_download
            .selected_book_id
            .or_else(|| self.selected_ids.first().copied())
    }

    fn apply_downloaded_metadata_to_book(
        &mut self,
        book_id: i64,
        metadata: &DownloadedMetadata,
    ) -> CoreResult<()> {
        if self.details.as_ref().map(|d| d.book.id) != Some(book_id) {
            let _ = self.load_details(book_id)?;
        }
        let Some(details) = &self.details else {
            return Err(CoreError::ConfigValidate(
                "book details unavailable for metadata apply".to_string(),
            ));
        };
        self.edit = EditState::from_details(details);
        if let Some(title) = &metadata.title {
            if self.metadata_download.overwrite_title
                || self.edit.title.trim().is_empty()
                || !self.metadata_download.merge_mode
            {
                self.edit.title = title.clone();
            }
        }
        if !metadata.authors.is_empty()
            && (self.metadata_download.overwrite_authors
                || self.edit.authors.trim().is_empty()
                || !self.metadata_download.merge_mode)
        {
            self.edit.authors = metadata.authors.join(", ");
        }
        if let Some(publisher) = &metadata.publisher {
            if self.metadata_download.overwrite_publisher || self.edit.publisher.trim().is_empty() {
                self.edit.publisher = publisher.clone();
            }
        } else if !self.metadata_download.merge_mode && self.metadata_download.overwrite_publisher {
            self.edit.publisher.clear();
        }
        if let Some(language) = &metadata.language {
            if self.metadata_download.overwrite_language || self.edit.languages.trim().is_empty() {
                self.edit.languages = language.clone();
            }
        } else if !self.metadata_download.merge_mode && self.metadata_download.overwrite_language {
            self.edit.languages.clear();
        }
        if let Some(pubdate) = &metadata.pubdate {
            if self.metadata_download.overwrite_pubdate || self.edit.pubdate.trim().is_empty() {
                self.edit.pubdate = pubdate.clone();
            }
        } else if !self.metadata_download.merge_mode && self.metadata_download.overwrite_pubdate {
            self.edit.pubdate.clear();
        }
        if let Some(description) = &metadata.description {
            if self.metadata_download.overwrite_comment || self.edit.comment.trim().is_empty() {
                self.edit.comment = description.clone();
            }
        } else if !self.metadata_download.merge_mode && self.metadata_download.overwrite_comment {
            self.edit.comment.clear();
        }
        merge_tags_into_edit(
            &mut self.edit,
            &metadata.tags,
            self.metadata_download.merge_mode,
            self.metadata_download.merge_tags,
        );
        merge_identifiers_into_edit(
            &mut self.edit,
            &metadata.identifiers,
            self.metadata_download.merge_mode,
            self.metadata_download.merge_identifiers,
        );
        self.save_edit()?;
        self.load_details(book_id)?;
        Ok(())
    }

    fn apply_cover_from_bytes(&mut self, book_id: i64, bytes: &[u8]) -> CoreResult<()> {
        let image = image::load_from_memory(bytes)
            .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
        self.ensure_cover_dirs()?;
        let cover_path = self.cover_path(book_id);
        image
            .save_with_format(&cover_path, ImageFormat::Png)
            .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
        self.generate_cover_thumb_from_image(book_id, &image)?;
        self.db.update_book_has_cover(book_id, true)?;
        self.clear_cover_cache(book_id);
        self.record_cover_history(cover_path.display().to_string());
        Ok(())
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
                    egui::ComboBox::from_id_salt("reader_search_scope")
                        .selected_text(match self.reader.search_scope {
                            ReaderSearchScope::CurrentBook => "This book",
                            ReaderSearchScope::Library => "Library",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.reader.search_scope,
                                ReaderSearchScope::CurrentBook,
                                "This book",
                            );
                            ui.selectable_value(
                                &mut self.reader.search_scope,
                                ReaderSearchScope::Library,
                                "Library",
                            );
                        });
                    if ui.button("Find").clicked() {
                        match self.reader.search_scope {
                            ReaderSearchScope::CurrentBook => self.reader.find_next(),
                            ReaderSearchScope::Library => {
                                let query = self.reader.search_query.trim().to_lowercase();
                                self.reader.search_results = self
                                    .all_books
                                    .iter()
                                    .filter(|book| {
                                        book.title.to_lowercase().contains(&query)
                                            || book.authors.to_lowercase().contains(&query)
                                            || book.tags.to_lowercase().contains(&query)
                                    })
                                    .map(|book| ReaderSearchResult {
                                        label: format!(
                                            "{} — {}",
                                            book.title,
                                            if book.authors.is_empty() {
                                                "Unknown".to_string()
                                            } else {
                                                book.authors.clone()
                                            }
                                        ),
                                        page: None,
                                        book_id: Some(book.id),
                                    })
                                    .collect();
                            }
                        }
                    }
                    if ui.button("Clear").clicked() {
                        self.reader.search_query.clear();
                        self.reader.search_results.clear();
                        self.reader.search_result_cursor = None;
                    }
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.reader.search_highlighting, "Highlight matches");
                    ui.label(format!("Results: {}", self.reader.search_results.len()));
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
                    if ui.button("Prev chapter").clicked() {
                        self.reader.prev_chapter();
                    }
                    if ui.button("Next chapter").clicked() {
                        self.reader.next_chapter();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Go to page");
                    ui.add(
                        egui::DragValue::new(&mut self.reader.go_to_page_input).range(1..=50000),
                    );
                    if ui.button("Go").clicked() {
                        self.reader.go_to_page(self.reader.go_to_page_input);
                    }
                    ui.label("or %");
                    ui.add(
                        egui::DragValue::new(&mut self.reader.go_to_percent_input)
                            .speed(0.5)
                            .range(0.0..=100.0),
                    );
                    if ui.button("Jump %").clicked() {
                        self.reader.go_to_percent(self.reader.go_to_percent_input);
                    }
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
                ui.horizontal(|ui| {
                    ui.label("Font family");
                    egui::ComboBox::from_id_salt("reader_font_family")
                        .selected_text(match self.reader.font_family {
                            ReaderFontFamily::Sans => "Sans",
                            ReaderFontFamily::Serif => "Serif",
                            ReaderFontFamily::Monospace => "Monospace",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.reader.font_family,
                                ReaderFontFamily::Sans,
                                "Sans",
                            );
                            ui.selectable_value(
                                &mut self.reader.font_family,
                                ReaderFontFamily::Serif,
                                "Serif",
                            );
                            ui.selectable_value(
                                &mut self.reader.font_family,
                                ReaderFontFamily::Monospace,
                                "Monospace",
                            );
                        });
                    ui.checkbox(&mut self.reader.justify_text, "Justify");
                    ui.checkbox(&mut self.reader.hyphenation, "Hyphenation");
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.reader.continuous_scroll, "Continuous scroll");
                    ui.label("Fit");
                    egui::ComboBox::from_id_salt("reader_fit_mode")
                        .selected_text(match self.reader.fit_mode {
                            ReaderFitMode::FitWidth => "Width",
                            ReaderFitMode::FitPage => "Page",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.reader.fit_mode,
                                ReaderFitMode::FitWidth,
                                "Fit width",
                            );
                            ui.selectable_value(
                                &mut self.reader.fit_mode,
                                ReaderFitMode::FitPage,
                                "Fit page",
                            );
                        });
                    ui.label("Image zoom");
                    ui.add(
                        egui::DragValue::new(&mut self.reader.image_zoom)
                            .speed(0.1)
                            .range(0.5..=4.0),
                    );
                });
                let mut page_chars = self.reader.page_chars;
                ui.horizontal(|ui| {
                    ui.label("Page chars");
                    ui.add(egui::DragValue::new(&mut page_chars).range(600..=6000));
                });
                if matches!(self.reader.fit_mode, ReaderFitMode::FitPage) {
                    page_chars = page_chars.clamp(600, 1800);
                } else {
                    page_chars = page_chars.clamp(1200, 6000);
                }
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
                    ui.label("Preset");
                    egui::ComboBox::from_id_salt("reader_preset")
                        .selected_text(match self.reader.preset {
                            ReaderPreset::Balanced => "Balanced",
                            ReaderPreset::Focus => "Focus",
                            ReaderPreset::Dense => "Dense",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.reader.preset,
                                ReaderPreset::Balanced,
                                "Balanced",
                            );
                            ui.selectable_value(
                                &mut self.reader.preset,
                                ReaderPreset::Focus,
                                "Focus",
                            );
                            ui.selectable_value(
                                &mut self.reader.preset,
                                ReaderPreset::Dense,
                                "Dense",
                            );
                        });
                    if ui.button("Apply preset").clicked() {
                        self.reader.apply_preset();
                    }
                    ui.label("Margins");
                    ui.add(
                        egui::DragValue::new(&mut self.reader.margin_scale)
                            .speed(0.05)
                            .range(0.5..=2.0),
                    );
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
                    ui.add_space(12.0 * self.reader.margin_scale);
                    ui.set_width(ui.available_width());
                    ui.add_space(4.0);
                    self.reader.render(ui);
                    ui.add_space(4.0);
                    ui.add_space(12.0 * self.reader.margin_scale);
                });
                ui.separator();
                ui.collapsing("Search results", |ui| {
                    let results = self.reader.search_results.clone();
                    for (idx, result) in results.iter().enumerate() {
                        if ui
                            .selectable_label(
                                self.reader.search_result_cursor == Some(idx),
                                result.label.as_str(),
                            )
                            .clicked()
                        {
                            self.reader.search_result_cursor = Some(idx);
                            if let Some(page) = result.page {
                                self.reader.page = page;
                            } else if let Some(book_id) = result.book_id {
                                let _ = self.open_reader(book_id);
                            }
                        }
                    }
                });
                ui.collapsing("Table of contents", |ui| {
                    let toc = self.reader.toc.clone();
                    for item in toc {
                        if ui
                            .button(format!("{} (p{})", item.title, item.page + 1))
                            .clicked()
                        {
                            self.reader.page = item.page;
                        }
                    }
                });
                ui.collapsing("Bookmarks", |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Add bookmark").clicked() {
                            self.reader.add_bookmark();
                        }
                    });
                    let bookmarks = self.reader.bookmarks.clone();
                    for (idx, mark) in bookmarks.iter().enumerate() {
                        ui.horizontal(|ui| {
                            if ui.button(mark.title.as_str()).clicked() {
                                self.reader.page = mark.page;
                            }
                            if ui.small_button("Remove").clicked() {
                                self.reader.remove_bookmark(idx);
                            }
                        });
                    }
                });
                ui.collapsing("Highlights + Notes", |ui| {
                    ui.label("Text/quote");
                    ui.text_edit_singleline(&mut self.reader.selected_text);
                    ui.label("Color");
                    egui::ComboBox::from_id_salt("highlight_color")
                        .selected_text(match self.reader.highlight_color {
                            ReaderHighlightColor::Yellow => "Yellow",
                            ReaderHighlightColor::Green => "Green",
                            ReaderHighlightColor::Blue => "Blue",
                            ReaderHighlightColor::Pink => "Pink",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.reader.highlight_color,
                                ReaderHighlightColor::Yellow,
                                "Yellow",
                            );
                            ui.selectable_value(
                                &mut self.reader.highlight_color,
                                ReaderHighlightColor::Green,
                                "Green",
                            );
                            ui.selectable_value(
                                &mut self.reader.highlight_color,
                                ReaderHighlightColor::Blue,
                                "Blue",
                            );
                            ui.selectable_value(
                                &mut self.reader.highlight_color,
                                ReaderHighlightColor::Pink,
                                "Pink",
                            );
                        });
                    ui.label("Note");
                    ui.text_edit_singleline(&mut self.reader.highlight_note);
                    if ui.button("Add highlight").clicked() {
                        self.reader.add_annotation();
                    }
                    let annotations = self.reader.annotations.clone();
                    for (idx, ann) in annotations.iter().enumerate() {
                        ui.horizontal(|ui| {
                            if ui
                                .button(format!("p{} {}", ann.page + 1, ann.text))
                                .clicked()
                            {
                                self.reader.page = ann.page;
                            }
                            let mut note = ann.note.clone();
                            if ui.text_edit_singleline(&mut note).changed() {
                                if let Some(edit) = self.reader.annotations.get_mut(idx) {
                                    edit.note = note;
                                }
                            }
                        });
                    }
                });
                if let Some(book_id) = self.reader.book_id {
                    self.reader_progress.insert(book_id, self.reader.page);
                }
                ui.separator();
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
        self.record_cover_history(cover_path.display().to_string());
        Ok(())
    }

    fn apply_cover_from_clipboard(&mut self, book_id: i64) -> CoreResult<()> {
        let mut clipboard = Clipboard::new()
            .map_err(|err| CoreError::ConfigValidate(format!("open clipboard: {err}")))?;
        if let Ok(image_data) = clipboard.get_image() {
            let width = image_data.width as u32;
            let height = image_data.height as u32;
            let bytes = image_data.bytes.into_owned();
            let image = image::RgbaImage::from_raw(width, height, bytes).ok_or_else(|| {
                CoreError::ConfigValidate("invalid image payload from clipboard".to_string())
            })?;
            let dynamic = DynamicImage::ImageRgba8(image);
            self.ensure_cover_dirs()?;
            let cover_path = self.cover_path(book_id);
            dynamic
                .save_with_format(&cover_path, ImageFormat::Png)
                .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
            self.generate_cover_thumb_from_image(book_id, &dynamic)?;
            self.db.update_book_has_cover(book_id, true)?;
            self.clear_cover_cache(book_id);
            self.record_cover_history(cover_path.display().to_string());
            return Ok(());
        }
        if let Ok(text) = clipboard.get_text() {
            let path = PathBuf::from(text.trim());
            if path.is_file() && is_image_path(&path) {
                return self.apply_cover_from_path(book_id, &path);
            }
        }
        Err(CoreError::ConfigValidate(
            "clipboard does not contain a usable image or image file path".to_string(),
        ))
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
        self.record_cover_history(cover_path.display().to_string());
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
            self.cover_restore_history
                .push(cover_path.display().to_string());
            self.cover_restore_history.truncate(20);
        }
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

    fn record_cover_history(&mut self, path: String) {
        self.cover_history.retain(|entry| entry != &path);
        self.cover_history.insert(0, path);
        self.cover_history.truncate(40);
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
        info!(
            component = "gui",
            count = ids.len(),
            output_format = %self.convert_books.output_format,
            "starting convert books action"
        );
        let output_dir = output_dir_or_default(
            &self.convert_books.output_dir,
            &config.conversion.output_dir,
        );
        ensure_dir(&output_dir)?;
        let mut logs = vec![format!(
            "convert start format={} dir={}",
            self.convert_books.output_format,
            output_dir.display()
        )];
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
                .with_output_format(Some(self.convert_books.output_format.clone()))
                .with_profiles(
                    self.convert_books.input_profile.clone(),
                    self.convert_books.output_profile.clone(),
                )
                .with_heuristics(
                    self.convert_books.heuristic_enable,
                    self.convert_books.heuristic_unwrap_lines,
                    self.convert_books.heuristic_delete_blank_lines,
                )
                .with_page_setup(
                    self.convert_books.page_margin_left,
                    self.convert_books.page_margin_right,
                    self.convert_books.page_margin_top,
                    self.convert_books.page_margin_bottom,
                    self.convert_books.embed_fonts,
                    self.convert_books.subset_fonts,
                )
                .with_cover_policy(self.convert_books.cover_policy.clone());
            let _report = convert_file(&input_path, &output_path, &settings)?;
            logs.push(format!(
                "book_id={book_id} converted -> {}",
                output_path.display()
            ));
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
                        logs.push(format!(
                            "book_id={book_id} added converted asset to library"
                        ));
                    }
                    caliberate_assets::storage::StoreOutcome::Skipped(skip) => {
                        warn!(
                            component = "gui",
                            path = %skip.existing_path.display(),
                            "skipped storing converted asset"
                        );
                        logs.push(format!(
                            "book_id={book_id} skipped adding converted asset: {}",
                            skip.existing_path.display()
                        ));
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
        logs.push(format!("converted_count={converted}"));
        let details = vec![
            (
                "books_selected".to_string(),
                self.selected_ids.len().to_string(),
            ),
            ("books_converted".to_string(), converted.to_string()),
            (
                "output_format".to_string(),
                self.convert_books.output_format.clone(),
            ),
            (
                "input_profile".to_string(),
                self.convert_books.input_profile.clone(),
            ),
            (
                "output_profile".to_string(),
                self.convert_books.output_profile.clone(),
            ),
            ("output_dir".to_string(), output_dir.display().to_string()),
        ];
        let retry_action = JobRetryAction::Convert {
            ids: self.selected_ids.clone(),
            output_format: self.convert_books.output_format.clone(),
            output_dir: output_dir.clone(),
        };
        self.record_conversion_job(
            format!("Convert {} books", self.selected_ids.len()),
            JobStatus::Completed,
            Some(output_dir.clone()),
            details,
            logs,
            Some(retry_action),
        );
        self.needs_refresh = true;
        self.status = format!("Converted {converted} book(s)");
        let status = self.status.clone();
        self.push_toast(&status, ToastLevel::Info);
        info!(
            component = "gui",
            converted, "completed convert books action"
        );
        Ok(())
    }

    fn run_save_to_disk(&mut self, config: &ControlPlane) -> CoreResult<()> {
        let ids = self.selected_ids.clone();
        if ids.is_empty() {
            return Ok(());
        }
        info!(
            component = "gui",
            count = ids.len(),
            template = %self.save_to_disk.path_template,
            conflict_policy = %self.save_to_disk.conflict_policy,
            "starting save to disk action"
        );
        let output_dir =
            output_dir_or_default(&self.save_to_disk.output_dir, &config.conversion.output_dir);
        ensure_dir(&output_dir)?;
        let mut exported = 0;
        let mut skipped = 0;
        let mut logs = vec![format!(
            "save_to_disk start dir={} template={} policy={}",
            output_dir.display(),
            self.save_to_disk.path_template,
            self.save_to_disk.conflict_policy
        )];
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
                let authors = self.db.list_book_authors(book_id)?.join(", ");
                let base_dest = build_output_from_template(
                    &output_dir,
                    &self.save_to_disk.path_template,
                    &book.title,
                    book_id,
                    &format,
                    &authors,
                );
                let Some(dest) = resolve_export_conflict_path(
                    &base_dest,
                    self.save_to_disk.conflict_policy.as_str(),
                ) else {
                    skipped += 1;
                    logs.push(format!(
                        "book_id={book_id} skipped existing path {}",
                        base_dest.display()
                    ));
                    continue;
                };
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
                logs.push(format!(
                    "book_id={book_id} exported format={} -> {}",
                    format,
                    dest.display()
                ));
            }
        }
        logs.push(format!("exported_count={exported} skipped_count={skipped}"));
        let details = vec![
            (
                "books_selected".to_string(),
                self.selected_ids.len().to_string(),
            ),
            ("files_exported".to_string(), exported.to_string()),
            ("files_skipped".to_string(), skipped.to_string()),
            (
                "template".to_string(),
                self.save_to_disk.path_template.clone(),
            ),
            (
                "conflict_policy".to_string(),
                self.save_to_disk.conflict_policy.clone(),
            ),
            ("output_dir".to_string(), output_dir.display().to_string()),
        ];
        let retry_action = JobRetryAction::SaveToDisk {
            ids: self.selected_ids.clone(),
            output_dir: output_dir.clone(),
        };
        self.record_save_to_disk_job(
            format!("Save to disk {} books", self.selected_ids.len()),
            JobStatus::Completed,
            Some(output_dir.clone()),
            details,
            logs,
            Some(retry_action),
        );
        self.status = format!("Exported {exported} file(s)");
        let status = self.status.clone();
        self.push_toast(&status, ToastLevel::Info);
        info!(
            component = "gui",
            exported, skipped, "completed save to disk action"
        );
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
            if self.device_sync.auto_convert
                && !format.eq_ignore_ascii_case(config.conversion.default_output_format.as_str())
            {
                self.device_sync.queue.push(DeviceQueueRow {
                    item: format!("{} ({book_id})", book.title),
                    status: format!(
                        "auto-convert pending ({format} -> {})",
                        config.conversion.default_output_format
                    ),
                });
            }
            let _result = send_to_device(&input_path, &device, dest_name.as_deref())?;
            self.device_sync.queue.push(DeviceQueueRow {
                item: format!("{} ({book_id})", book.title),
                status: "completed".to_string(),
            });
            if let Some(temp_path) = temp_input {
                let _ = fs::remove_file(temp_path);
            }
            sent += 1;
        }
        self.status = format!("Sent {sent} file(s) to device {}", device.name);
        let status = self.status.clone();
        self.push_toast(&status, ToastLevel::Info);
        self.config_dirty = true;
        Ok(())
    }

    fn run_fetch_from_device(
        &mut self,
        config: &ControlPlane,
        path: &Path,
        mode: IngestMode,
    ) -> CoreResult<()> {
        let store = LocalAssetStore::from_config(config);
        let ingestor = Ingestor::new(std::sync::Arc::new(store), config.clone());
        match ingestor.ingest(IngestRequest {
            source_path: path,
            mode: Some(mode),
        })? {
            IngestOutcome::Ingested(result) => {
                let book_id = self.insert_ingested_book(&result)?;
                self.status = format!("Imported from device as book #{book_id}");
                self.needs_refresh = true;
                Ok(())
            }
            IngestOutcome::Skipped(skip) => Err(CoreError::ConfigValidate(format!(
                "device import skipped: {:?} ({})",
                skip.reason,
                skip.existing_path.display()
            ))),
        }
    }

    fn refresh_device_files(&mut self, _config: &ControlPlane) -> CoreResult<()> {
        self.device_manager.files.clear();
        self.device_manager.collections.clear();
        let Some(device) = self.active_managed_device() else {
            return Ok(());
        };
        let files = list_device_entries(&device)?;
        let mut collections: BTreeSet<String> = BTreeSet::new();
        for file in &files {
            let collection = file
                .parent()
                .and_then(|parent| parent.strip_prefix(&device.library_path).ok())
                .and_then(|path| path.components().next())
                .map(|component| component.as_os_str().to_string_lossy().to_string())
                .unwrap_or_else(|| "root".to_string());
            collections.insert(collection);
        }
        self.device_manager.files = files;
        self.device_manager.collections = collections.into_iter().collect();
        Ok(())
    }

    fn active_managed_device(&self) -> Option<DeviceInfo> {
        self.device_manager
            .selected_device
            .and_then(|idx| self.device_manager.devices.get(idx))
            .cloned()
    }

    fn filtered_device_files(&self) -> Vec<PathBuf> {
        let filter = self.device_manager.file_filter.to_lowercase();
        self.device_manager
            .files
            .iter()
            .filter(|path| {
                filter.is_empty()
                    || path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or_default()
                        .to_lowercase()
                        .contains(&filter)
            })
            .cloned()
            .collect()
    }

    fn device_storage_stats(&self, device: &DeviceInfo) -> (usize, u64) {
        let mut count = 0usize;
        let mut bytes = 0u64;
        if let Ok(entries) = list_device_entries(device) {
            count = entries.len();
            for entry in entries {
                if let Ok(meta) = fs::metadata(&entry) {
                    bytes += meta.len();
                }
            }
        }
        (count, bytes)
    }

    fn refresh_news_sources(&mut self, config: &ControlPlane) -> CoreResult<()> {
        let mut names: BTreeSet<String> = BTreeSet::new();
        if config.news.recipes_dir.exists() {
            for entry in fs::read_dir(&config.news.recipes_dir)
                .map_err(|err| CoreError::Io("read news recipes dir".to_string(), err))?
            {
                let entry = entry
                    .map_err(|err| CoreError::Io("read news recipe entry".to_string(), err))?;
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                    names.insert(stem.to_string());
                }
            }
        }
        for key in config.news.source_enabled.keys() {
            names.insert(key.clone());
        }
        let mut rows = Vec::new();
        for name in names {
            rows.push(NewsSourceRow {
                enabled: *config.news.source_enabled.get(&name).unwrap_or(&true),
                schedule: config
                    .news
                    .source_schedule
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| "manual".to_string()),
                status: "idle".to_string(),
                name,
            });
        }
        self.news_manager.sources = rows;
        Ok(())
    }

    fn refresh_news_downloads(&mut self, config: &ControlPlane) -> CoreResult<()> {
        self.news_manager.downloads.clear();
        if !config.news.downloads_dir.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(&config.news.downloads_dir)
            .map_err(|err| CoreError::Io("read news downloads dir".to_string(), err))?
        {
            let entry =
                entry.map_err(|err| CoreError::Io("read news download entry".to_string(), err))?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_string();
            let source = name.split('-').next().unwrap_or("news").replace('_', " ");
            self.news_manager.downloads.push(NewsDownloadRow {
                source,
                path,
                status: "downloaded".to_string(),
            });
        }
        self.news_manager
            .downloads
            .sort_by(|a, b| b.path.cmp(&a.path));
        Ok(())
    }

    fn append_news_history(&mut self, config: &ControlPlane, line: &str) -> CoreResult<()> {
        if let Some(parent) = config.news.history_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| CoreError::Io("create news history parent".to_string(), err))?;
        }
        let mut lines = if config.news.history_path.exists() {
            fs::read_to_string(&config.news.history_path)
                .map_err(|err| CoreError::Io("read news history".to_string(), err))?
                .lines()
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        lines.push(line.to_string());
        lines.truncate(500);
        fs::write(&config.news.history_path, lines.join("\n"))
            .map_err(|err| CoreError::Io("write news history".to_string(), err))?;
        Ok(())
    }

    fn load_news_history(&mut self, config: &ControlPlane) -> CoreResult<()> {
        self.news_manager.history_lines.clear();
        if !config.news.history_path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(&config.news.history_path)
            .map_err(|err| CoreError::Io("read news history".to_string(), err))?;
        self.news_manager.history_lines = content.lines().map(|line| line.to_string()).collect();
        Ok(())
    }

    fn import_news_recipe(&mut self, config: &ControlPlane) -> CoreResult<()> {
        let source = self.news_manager.recipe_import_path.trim();
        if source.is_empty() {
            return Err(CoreError::ConfigValidate(
                "recipe import path cannot be empty".to_string(),
            ));
        }
        let source_path = PathBuf::from(source);
        let file_name = source_path
            .file_name()
            .ok_or_else(|| CoreError::ConfigValidate("invalid recipe file name".to_string()))?;
        fs::create_dir_all(&config.news.recipes_dir)
            .map_err(|err| CoreError::Io("create recipes dir".to_string(), err))?;
        let dest = config.news.recipes_dir.join(file_name);
        fs::copy(&source_path, &dest)
            .map_err(|err| CoreError::Io("copy recipe".to_string(), err))?;
        self.append_news_history(
            config,
            &format!("{} imported recipe {}", now_timestamp()?, dest.display()),
        )?;
        Ok(())
    }

    fn run_news_download(&mut self, config: &ControlPlane) -> CoreResult<()> {
        fs::create_dir_all(&config.news.downloads_dir)
            .map_err(|err| CoreError::Io("create news downloads dir".to_string(), err))?;
        let store = LocalAssetStore::from_config(config);
        let ingestor = Ingestor::new(std::sync::Arc::new(store), config.clone());
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let mut downloaded = 0usize;
        let source_count = self.news_manager.sources.len().min(config.news.fetch_limit);
        for idx in 0..source_count {
            let (source_name, enabled) = {
                let source = &self.news_manager.sources[idx];
                (source.name.clone(), source.enabled)
            };
            if !enabled {
                self.news_manager.sources[idx].status = "disabled".to_string();
                continue;
            }
            let file_name = format!("{}-{now}.txt", source_name.replace(' ', "_"));
            let path = config.news.downloads_dir.join(file_name);
            let content = format!(
                "# {}\n\nFetched at {}\n\nThis is a generated news digest placeholder.",
                source_name,
                now_timestamp()?
            );
            fs::write(&path, content)
                .map_err(|err| CoreError::Io("write news digest".to_string(), err))?;
            match ingestor.ingest(IngestRequest {
                source_path: &path,
                mode: Some(IngestMode::Copy),
            })? {
                IngestOutcome::Ingested(result) => {
                    let book_id = self.insert_ingested_book(&result)?;
                    self.db
                        .add_book_tags(book_id, &["news".to_string(), source_name.clone()])?;
                }
                IngestOutcome::Skipped(_) => {}
            }
            downloaded += 1;
            self.news_manager.sources[idx].status = "downloaded".to_string();
            self.append_news_history(
                config,
                &format!("{} downloaded {}", now_timestamp()?, source_name),
            )?;
        }
        if self.news_manager.auto_delete {
            self.prune_news_downloads(config, self.news_manager.retention_days)?;
        }
        self.needs_refresh = true;
        self.status = format!("Downloaded {downloaded} news item(s)");
        Ok(())
    }

    fn retry_news_download(&mut self, config: &ControlPlane) -> CoreResult<()> {
        let Some(idx) = self.news_manager.selected_source else {
            return Err(CoreError::ConfigValidate("no source selected".to_string()));
        };
        if idx >= self.news_manager.sources.len() {
            return Err(CoreError::ConfigValidate(
                "invalid source selection".to_string(),
            ));
        }
        self.news_manager.sources[idx].status = "retrying".to_string();
        self.run_news_download(config)
    }

    fn open_selected_news_in_reader(&mut self) -> CoreResult<()> {
        let Some(idx) = self.news_manager.selected_download else {
            return Err(CoreError::ConfigValidate(
                "no downloaded news selected".to_string(),
            ));
        };
        let row = self
            .news_manager
            .downloads
            .get(idx)
            .ok_or_else(|| CoreError::ConfigValidate("invalid news selection".to_string()))?
            .clone();
        let raw = fs::read_to_string(&row.path)
            .map_err(|err| CoreError::Io("read selected news item".to_string(), err))?;
        let synthetic_book_id = -((idx as i64) + 1);
        self.reader
            .open_virtual_text(synthetic_book_id, &format!("News: {}", row.source), &raw);
        Ok(())
    }

    fn prune_news_downloads(
        &mut self,
        config: &ControlPlane,
        retention_days: u64,
    ) -> CoreResult<()> {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let keep_after = now - ((retention_days as i64) * 24 * 60 * 60);
        for row in self.news_manager.downloads.clone() {
            let Ok(meta) = fs::metadata(&row.path) else {
                continue;
            };
            let Ok(modified) = meta.modified() else {
                continue;
            };
            let age = modified
                .elapsed()
                .map(|elapsed| now - (elapsed.as_secs() as i64))
                .unwrap_or(now);
            if age < keep_after {
                if let Err(err) = fs::remove_file(&row.path) {
                    warn!(
                        component = "gui",
                        path = %row.path.display(),
                        error = %err,
                        "failed pruning news download"
                    );
                } else {
                    let _ = self.append_news_history(
                        config,
                        &format!("{} pruned {}", now_timestamp()?, row.path.display()),
                    );
                }
            }
        }
        self.refresh_news_downloads(config)?;
        Ok(())
    }

    fn build_export_preview(&self, config: &ControlPlane, max_rows: usize) -> Vec<String> {
        let output_dir =
            output_dir_or_default(&self.save_to_disk.output_dir, &config.conversion.output_dir);
        let mut rows = Vec::new();
        for book_id in self.selected_ids.iter().copied().take(max_rows) {
            let Some(book) = self.db.get_book(book_id).ok().flatten() else {
                continue;
            };
            let format = if book.format.trim().is_empty() {
                "unknown".to_string()
            } else {
                book.format.clone()
            };
            let authors = self
                .db
                .list_book_authors(book_id)
                .ok()
                .map(|items| items.join(", "))
                .unwrap_or_default();
            let dest = build_output_from_template(
                &output_dir,
                &self.save_to_disk.path_template,
                &book.title,
                book_id,
                &format,
                &authors,
            );
            rows.push(format!(
                "#{book_id} {} -> {}",
                book.title,
                dest.file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| dest.display().to_string())
            ));
        }
        rows
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
        let selected = self.manage_custom_columns.edit_label.clone();
        if !selected.trim().is_empty()
            && self
                .manage_custom_columns
                .columns
                .iter()
                .any(|column| column.label == selected)
        {
            self.select_custom_column_for_edit(&selected);
        } else if let Some(first_label) = self
            .manage_custom_columns
            .columns
            .first()
            .map(|column| column.label.clone())
        {
            self.select_custom_column_for_edit(&first_label);
        }
        self.manage_custom_columns.needs_refresh = false;
        Ok(())
    }

    fn refresh_manage_virtual_libraries(&mut self) -> CoreResult<()> {
        let searches = self.db.list_saved_searches()?;
        self.manage_virtual_libraries.searches = searches.into_iter().collect();
        let mut folders: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for (name, query) in &self.manage_virtual_libraries.searches {
            let (folder, leaf) = split_saved_search_name(name);
            folders
                .entry(folder.to_string())
                .or_default()
                .push((leaf.to_string(), query.clone()));
        }
        self.manage_virtual_libraries.folders = folders;
        self.manage_virtual_libraries.needs_refresh = false;
        Ok(())
    }

    fn build_saved_search_groups(&self) -> Vec<(String, Vec<(String, String)>)> {
        let mut grouped = self
            .manage_virtual_libraries
            .folders
            .iter()
            .map(|(folder, entries)| {
                let mut rows = entries.clone();
                rows.sort_by(|a, b| a.0.cmp(&b.0));
                (folder.clone(), rows)
            })
            .collect::<Vec<_>>();
        grouped.sort_by(|a, b| a.0.cmp(&b.0));
        grouped
    }

    fn append_query_builder_clause(&mut self) {
        let field = self.manage_virtual_libraries.builder_field.trim();
        let op = self.manage_virtual_libraries.builder_op.trim();
        let value = self.manage_virtual_libraries.builder_value.trim();
        if field.is_empty() || op.is_empty() || value.is_empty() {
            return;
        }
        let clause = match op {
            "is" => format!("{field}:\"{value}\""),
            "not" => format!("not {field}:\"{value}\""),
            _ => format!("{field}:{value}"),
        };
        if self.manage_virtual_libraries.new_query.trim().is_empty() {
            self.manage_virtual_libraries.new_query = clause;
        } else {
            self.manage_virtual_libraries.new_query =
                format!("{} and {clause}", self.manage_virtual_libraries.new_query);
        }
    }

    fn export_saved_searches(&self, path: &Path) -> CoreResult<()> {
        let output = path;
        if output.as_os_str().is_empty() {
            return Err(CoreError::ConfigValidate(
                "saved-search export path cannot be empty".to_string(),
            ));
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                CoreError::Io("create saved-search export parent".to_string(), err)
            })?;
        }
        let map = self.db.list_saved_searches()?;
        let json = serde_json::to_string_pretty(&map)
            .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
        fs::write(output, json)
            .map_err(|err| CoreError::Io("write saved-search export".to_string(), err))?;
        Ok(())
    }

    fn import_saved_searches(&self, path: &Path) -> CoreResult<()> {
        if path.as_os_str().is_empty() {
            return Err(CoreError::ConfigValidate(
                "saved-search import path cannot be empty".to_string(),
            ));
        }
        let content = fs::read_to_string(path)
            .map_err(|err| CoreError::Io("read saved-search import".to_string(), err))?;
        let map = serde_json::from_str::<HashMap<String, String>>(&content)
            .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
        for (name, query) in map {
            self.db.add_saved_search(&name, &query)?;
        }
        Ok(())
    }

    fn assign_virtual_library_tag(&mut self, name: &str) -> CoreResult<()> {
        if name.is_empty() {
            return Err(CoreError::ConfigValidate(
                "virtual library name cannot be empty".to_string(),
            ));
        }
        let tag = format!("vl:{name}");
        for book_id in self.selected_ids.clone() {
            self.db.add_book_tags(book_id, std::slice::from_ref(&tag))?;
        }
        self.needs_refresh = true;
        Ok(())
    }

    fn unassign_virtual_library_tag(&mut self, name: &str) -> CoreResult<()> {
        if name.is_empty() {
            return Err(CoreError::ConfigValidate(
                "virtual library name cannot be empty".to_string(),
            ));
        }
        let tag = format!("vl:{name}");
        for book_id in self.selected_ids.clone() {
            let mut tags = self.db.list_book_tags(book_id)?;
            tags.retain(|value| !value.eq_ignore_ascii_case(&tag));
            self.db.replace_book_tags(book_id, &tags)?;
        }
        self.needs_refresh = true;
        Ok(())
    }

    fn bulk_assign_tag(&mut self, tag: &str) -> CoreResult<()> {
        if tag.is_empty() {
            return Err(CoreError::ConfigValidate(
                "tag cannot be empty for assign".to_string(),
            ));
        }
        let tag_value = tag.to_string();
        for book_id in self.selected_ids.clone() {
            self.db
                .add_book_tags(book_id, std::slice::from_ref(&tag_value))?;
        }
        self.needs_refresh = true;
        Ok(())
    }

    fn bulk_remove_tag(&mut self, tag: &str) -> CoreResult<()> {
        if tag.is_empty() {
            return Err(CoreError::ConfigValidate(
                "tag cannot be empty for remove".to_string(),
            ));
        }
        for book_id in self.selected_ids.clone() {
            let mut tags = self.db.list_book_tags(book_id)?;
            tags.retain(|value| !value.eq_ignore_ascii_case(tag));
            self.db.replace_book_tags(book_id, &tags)?;
        }
        self.needs_refresh = true;
        Ok(())
    }

    fn renumber_selected_series(&mut self, series: &str, start: f64, step: f64) -> CoreResult<()> {
        if series.is_empty() {
            return Err(CoreError::ConfigValidate(
                "series cannot be empty for renumber".to_string(),
            ));
        }
        if step <= 0.0 {
            return Err(CoreError::ConfigValidate(
                "series renumber step must be > 0".to_string(),
            ));
        }
        let mut next_index = start;
        for book_id in self.selected_ids.clone() {
            self.db.set_book_series(book_id, series, next_index)?;
            next_index += step;
        }
        self.needs_refresh = true;
        Ok(())
    }

    fn export_custom_columns(&self, path: &Path) -> CoreResult<()> {
        if path.as_os_str().is_empty() {
            return Err(CoreError::ConfigValidate(
                "custom-column export path cannot be empty".to_string(),
            ));
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| CoreError::Io("create custom export parent".to_string(), err))?;
        }
        let columns = self.db.list_custom_columns()?;
        let export_rows = columns
            .into_iter()
            .map(|col| {
                serde_json::json!({
                    "label": col.label,
                    "name": col.name,
                    "datatype": col.datatype,
                    "display": col.display
                })
            })
            .collect::<Vec<_>>();
        let json = serde_json::to_string_pretty(&export_rows)
            .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
        fs::write(path, json)
            .map_err(|err| CoreError::Io("write custom columns export".to_string(), err))?;
        Ok(())
    }

    fn import_custom_columns(&self, path: &Path) -> CoreResult<()> {
        if path.as_os_str().is_empty() {
            return Err(CoreError::ConfigValidate(
                "custom-column import path cannot be empty".to_string(),
            ));
        }
        let content = fs::read_to_string(path)
            .map_err(|err| CoreError::Io("read custom column import".to_string(), err))?;
        let columns = serde_json::from_str::<Vec<serde_json::Value>>(&content)
            .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
        for col in columns {
            let label = col
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let name = col
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let datatype = col
                .get("datatype")
                .and_then(|v| v.as_str())
                .unwrap_or("text")
                .to_string();
            let display = col
                .get("display")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if label.trim().is_empty() || name.trim().is_empty() {
                continue;
            }
            let _ = self
                .db
                .create_custom_column(&label, &name, &datatype, &display);
        }
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
        let list = if query.is_empty() || self.search_scope != SearchScope::All {
            self.db.list_books()?
        } else {
            self.db.search_books(&query)?
        };
        let mut rows = Vec::new();
        for book in list {
            let row = self.build_row(&book)?;
            rows.push(row);
        }
        if !query.is_empty() && self.search_scope != SearchScope::All {
            let needle = query.to_lowercase();
            rows.retain(|book| match self.search_scope {
                SearchScope::All => true,
                SearchScope::Title => field_contains(&book.title, &needle),
                SearchScope::Authors => field_contains(&book.authors, &needle),
                SearchScope::Tags => field_contains(&book.tags, &needle),
                SearchScope::Series => field_contains(&book.series, &needle),
            });
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
        let (
            authors,
            tags,
            series,
            rating,
            publisher,
            languages,
            has_cover,
            date_added,
            date_modified,
            pubdate,
        ) = if let Some(details) = details {
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
                details.extras.timestamp.unwrap_or_default(),
                details.extras.last_modified.unwrap_or_default(),
                details.extras.pubdate.unwrap_or_default(),
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
                String::new(),
                String::new(),
                String::new(),
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
            date_added,
            date_modified,
            pubdate,
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
        if let Some(details) = &self.details {
            self.edit = EditState::from_details(details);
            self.load_edit_custom_fields(id)?;
            self.load_publish_slots(id)?;
        }
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
                let news_matches = if self.news_only_filter {
                    field_contains(&book.tags, "news")
                } else {
                    true
                };
                format_matches && browser_matches && news_matches
            })
            .cloned()
            .collect();
        self.sort_rows(&mut list);
        self.books = list;
    }

    fn sort_rows(&mut self, list: &mut Vec<BookRow>) {
        let primary = self.sort_mode;
        let secondary = self.secondary_sort;
        let group_mode = self.group_mode;
        let mut indexed: Vec<(usize, BookRow)> = list.drain(..).enumerate().collect();
        indexed.sort_by(|(a_idx, a), (b_idx, b)| {
            let mut cmp = compare_group(group_mode, a, b);
            if cmp == std::cmp::Ordering::Equal {
                cmp = compare_row(primary, a, b);
            }
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

    fn save_sort_preset(&mut self) {
        let name = self.sort_preset_name.trim().to_string();
        if name.is_empty() {
            self.push_toast("Sort preset name is required", ToastLevel::Warn);
            return;
        }
        let preset = SortPreset {
            primary: self.sort_mode,
            secondary: self.secondary_sort,
            direction: self.sort_dir,
        };
        self.sort_presets.insert(name.clone(), preset);
        self.active_sort_preset = Some(name.clone());
        self.sort_preset_name.clear();
        self.config_dirty = true;
        info!(component = "gui", preset = name, "sort preset saved");
        self.push_toast("Sort preset saved", ToastLevel::Info);
    }

    fn apply_sort_preset(&mut self, name: &str) {
        if let Some(preset) = self.sort_presets.get(name).cloned() {
            self.sort_mode = preset.primary;
            self.secondary_sort = preset.secondary;
            self.sort_dir = preset.direction;
            self.apply_filters();
            info!(component = "gui", preset = name, "sort preset applied");
        }
    }

    fn delete_active_sort_preset(&mut self) {
        let Some(name) = self.active_sort_preset.clone() else {
            return;
        };
        self.sort_presets.remove(&name);
        self.active_sort_preset = None;
        self.config_dirty = true;
        info!(component = "gui", preset = name, "sort preset deleted");
        self.push_toast("Sort preset deleted", ToastLevel::Info);
    }

    fn save_column_preset(&mut self) {
        let raw_name = self.column_preset_name.trim();
        if raw_name.is_empty() {
            self.push_toast("Column preset name is required", ToastLevel::Warn);
            return;
        }
        let name = self.scoped_column_preset_name(raw_name);
        let preset = ColumnPreset {
            order: self.column_order.clone(),
            visibility: self.columns.clone(),
            widths: self.column_widths.clone(),
        };
        self.column_presets.insert(name.clone(), preset);
        self.active_column_preset = Some(name.clone());
        self.column_preset_name.clear();
        self.config_dirty = true;
        info!(component = "gui", preset = name, "column preset saved");
        self.push_toast("Column preset saved", ToastLevel::Info);
    }

    fn apply_column_preset(&mut self, name: &str) {
        if let Some(preset) = self.column_presets.get(name).cloned() {
            self.column_order = preset.order;
            self.columns = preset.visibility;
            self.column_widths = preset.widths;
            self.layout_dirty = true;
            info!(component = "gui", preset = name, "column preset applied");
        }
    }

    fn delete_active_column_preset(&mut self) {
        let Some(name) = self.active_column_preset.clone() else {
            return;
        };
        self.column_presets.remove(&name);
        self.active_column_preset = None;
        self.config_dirty = true;
        info!(component = "gui", preset = name, "column preset deleted");
        self.push_toast("Column preset deleted", ToastLevel::Info);
    }

    fn scoped_column_preset_name(&self, name: &str) -> String {
        let trimmed = name.trim();
        match self.column_preset_scope {
            ColumnPresetScope::CurrentView => {
                format!("{}/{}", self.view_mode.preset_scope_key(), trimmed)
            }
            ColumnPresetScope::Global => format!("all/{trimmed}"),
        }
    }

    fn visible_column_preset_names(&self) -> Vec<String> {
        let view_scope = format!("{}/", self.view_mode.preset_scope_key());
        let mut names = self
            .column_presets
            .keys()
            .filter(|name| {
                name.starts_with("all/") || name.starts_with(&view_scope) || !name.contains('/')
            })
            .cloned()
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    fn selected_column_label(&self) -> Option<String> {
        if !self.manage_custom_columns.edit_label.trim().is_empty() {
            return Some(self.manage_custom_columns.edit_label.trim().to_string());
        }
        self.manage_custom_columns
            .columns
            .first()
            .map(|column| column.label.clone())
    }

    fn select_custom_column_for_edit(&mut self, label: &str) {
        if let Some(column) = self
            .manage_custom_columns
            .columns
            .iter()
            .find(|column| column.label == label)
            .cloned()
        {
            self.manage_custom_columns.edit_label = column.label.clone();
            self.manage_custom_columns.edit_name = column.name;
            self.manage_custom_columns.edit_datatype = column.datatype;
            self.manage_custom_columns.edit_display = column.display;
            self.manage_custom_columns.edit_editable = column.editable;
            self.manage_custom_columns.edit_is_multiple = column.is_multiple;
            self.manage_custom_columns.edit_normalized = column.normalized;
            self.manage_custom_columns.delete_label = column.label;
        }
    }

    fn save_custom_column_edits(&mut self) -> CoreResult<()> {
        let label = self.manage_custom_columns.edit_label.trim().to_string();
        if label.is_empty() {
            return Err(CoreError::ConfigValidate(
                "custom column label is required".to_string(),
            ));
        }
        let name = self.manage_custom_columns.edit_name.trim().to_string();
        if name.is_empty() {
            return Err(CoreError::ConfigValidate(
                "custom column name is required".to_string(),
            ));
        }
        self.db.update_custom_column(
            &label,
            &name,
            self.manage_custom_columns.edit_datatype.trim(),
            self.manage_custom_columns.edit_display.trim(),
            self.manage_custom_columns.edit_editable,
            self.manage_custom_columns.edit_is_multiple,
            self.manage_custom_columns.edit_normalized,
        )?;
        info!(component = "gui", label, "updated custom column metadata");
        Ok(())
    }

    fn begin_inline_edit(&mut self, book: &BookRow) {
        self.inline_edit.book_id = Some(book.id);
        self.inline_edit.title = book.title.clone();
        self.inline_edit.authors = book.authors.clone();
        self.inline_edit.tags = book.tags.clone();
    }

    fn cancel_inline_edit(&mut self) {
        self.inline_edit = InlineEditState::default();
    }

    fn save_inline_edit(&mut self) -> CoreResult<()> {
        let Some(book_id) = self.inline_edit.book_id else {
            return Ok(());
        };
        self.db
            .update_book_title(book_id, self.inline_edit.title.trim())?;
        self.db
            .replace_book_authors(book_id, &parse_list(&self.inline_edit.authors))?;
        self.db
            .replace_book_tags(book_id, &parse_list(&self.inline_edit.tags))?;
        self.cancel_inline_edit();
        self.refresh_books()?;
        self.push_toast("Inline metadata updated", ToastLevel::Info);
        Ok(())
    }

    fn row_text_color(&self, book: &BookRow) -> Option<egui::Color32> {
        if self.conditional_missing_cover && !book.has_cover {
            return parse_hex_color(&self.color_missing_cover);
        }
        if self.conditional_low_rating
            && parse_rating_value(&book.rating) <= self.low_rating_threshold
            && parse_rating_value(&book.rating) > 0
        {
            return parse_hex_color(&self.color_low_rating);
        }
        None
    }

    fn cancel_edit(&mut self) {
        if let Some(details) = &self.details {
            let book_id = details.book.id;
            self.edit = EditState::from_details(details);
            if let Err(err) = self.load_edit_custom_fields(book_id) {
                self.set_error(err);
            }
            if let Err(err) = self.load_publish_slots(book_id) {
                self.set_error(err);
            }
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
        let issues = collect_edit_validation_issues(&self.edit);
        if !issues.is_empty() {
            return Err(CoreError::ConfigValidate(issues.join("; ")));
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
        self.db
            .update_book_sort(book_id, self.edit.series_sort.trim())?;
        self.db
            .update_book_author_sort(book_id, self.edit.author_sort.trim())?;
        self.db
            .update_book_timestamp(book_id, self.edit.timestamp.trim())?;
        self.db
            .update_book_pubdate(book_id, self.edit.pubdate.trim())?;
        self.db
            .update_book_last_modified(book_id, self.edit.last_modified.trim())?;
        self.db.update_book_uuid(book_id, self.edit.uuid.trim())?;
        for field in &self.edit_custom_fields {
            self.db
                .set_custom_value(book_id, field.label.as_str(), field.value.as_str())?;
        }
        self.save_publish_slots(book_id)?;
        let languages = parse_list(&self.edit.languages);
        self.db.set_book_languages(book_id, &languages)?;
        self.status = "Metadata saved".to_string();
        self.edit_mode = false;
        self.refresh_books()?;
        self.load_details(book_id)?;
        info!(component = "gui", book_id, "saved metadata edits");
        Ok(())
    }

    fn load_edit_custom_fields(&mut self, book_id: i64) -> CoreResult<()> {
        let columns = self.db.list_custom_columns()?;
        let mut fields = Vec::new();
        for column in columns {
            if column.mark_for_delete || !column.editable {
                continue;
            }
            let value = self
                .db
                .get_custom_value(book_id, column.label.as_str())?
                .unwrap_or_default();
            fields.push(CustomEditField {
                label: column.label,
                name: column.name,
                datatype: column.datatype,
                value,
            });
        }
        self.edit_custom_fields = fields;
        Ok(())
    }

    fn ensure_publish_slot_columns(&self) -> CoreResult<()> {
        let existing = self
            .db
            .list_custom_columns()?
            .into_iter()
            .map(|column| column.label)
            .collect::<HashSet<_>>();
        for (label, name) in [
            ("imprint", "Imprint"),
            ("edition", "Edition"),
            ("rights", "Rights"),
        ] {
            if !existing.contains(label) {
                let _ = self.db.create_custom_column(label, name, "text", "");
            }
        }
        Ok(())
    }

    fn load_publish_slots(&mut self, book_id: i64) -> CoreResult<()> {
        self.ensure_publish_slot_columns()?;
        self.edit.imprint = self
            .db
            .get_custom_value(book_id, "imprint")?
            .unwrap_or_default();
        self.edit.edition = self
            .db
            .get_custom_value(book_id, "edition")?
            .unwrap_or_default();
        self.edit.rights = self
            .db
            .get_custom_value(book_id, "rights")?
            .unwrap_or_default();
        Ok(())
    }

    fn save_publish_slots(&self, book_id: i64) -> CoreResult<()> {
        self.db
            .set_custom_value(book_id, "imprint", self.edit.imprint.trim())?;
        self.db
            .set_custom_value(book_id, "edition", self.edit.edition.trim())?;
        self.db
            .set_custom_value(book_id, "rights", self.edit.rights.trim())?;
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
            ViewMode::Shelf => "shelf".to_string(),
        };
        config.gui.view_density = match self.view_density {
            ViewDensity::Compact => "compact".to_string(),
            ViewDensity::Comfortable => "comfortable".to_string(),
        };
        config.gui.group_mode = match self.group_mode {
            GroupMode::None => "none".to_string(),
            GroupMode::Series => "series".to_string(),
            GroupMode::Authors => "authors".to_string(),
            GroupMode::Tags => "tags".to_string(),
        };
        config.gui.shelf_columns = self.shelf_columns;
        config.gui.quick_details_panel = self.quick_details_panel;
        config.gui.pane_browser_visible = self.browser_visible;
        config.gui.pane_browser_side = self.browser_side.as_config().to_string();
        config.gui.pane_details_visible = self.details_visible;
        config.gui.pane_details_side = self.details_side.as_config().to_string();
        config.gui.pane_jobs_visible = self.jobs_visible;
        config.gui.pane_left_width = self.left_pane_width;
        config.gui.pane_right_width = self.right_pane_width;
        config.gui.column_order = self
            .column_order
            .iter()
            .map(|key| key.key().to_string())
            .collect();
        config.gui.column_presets = encode_column_presets(&self.column_presets);
        config.gui.active_column_preset = self.active_column_preset.clone();
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
        config.gui.group_mode = match self.group_mode {
            GroupMode::None => "none".to_string(),
            GroupMode::Series => "series".to_string(),
            GroupMode::Authors => "authors".to_string(),
            GroupMode::Tags => "tags".to_string(),
        };
        config.gui.shelf_columns = self.shelf_columns;
        config.gui.quick_details_panel = self.quick_details_panel;
        config.gui.column_order = self
            .column_order
            .iter()
            .map(|key| key.key().to_string())
            .collect();
        config.gui.sort_presets = encode_sort_presets(&self.sort_presets);
        config.gui.active_sort_preset = self.active_sort_preset.clone();
        config.gui.column_presets = encode_column_presets(&self.column_presets);
        config.gui.active_column_preset = self.active_column_preset.clone();
        config.gui.conditional_missing_cover = self.conditional_missing_cover;
        config.gui.conditional_low_rating = self.conditional_low_rating;
        config.gui.low_rating_threshold = self.low_rating_threshold;
        config.gui.color_missing_cover = self.color_missing_cover.clone();
        config.gui.color_low_rating = self.color_low_rating.clone();
        config.conversion.save_to_disk_template = self.save_to_disk.path_template.clone();
        config.conversion.save_to_disk_conflict_policy = self.save_to_disk.conflict_policy.clone();
        config.conversion.save_to_disk_presets = self
            .save_to_disk
            .presets
            .iter()
            .map(|(name, preset)| (name.clone(), preset.template.clone()))
            .collect();
        config.device.send_auto_convert = self.device_sync.auto_convert;
        config.device.send_overwrite = self.device_sync.overwrite;
        config.device.sync_metadata = self.device_sync.sync_metadata;
        config.device.sync_cover = self.device_sync.sync_cover;
        config.device.driver_backend = self.device_manager.driver_backend.clone();
        config.device.connection_timeout_ms = self.device_manager.connection_timeout_ms;
        config.news.auto_delete = self.news_manager.auto_delete;
        config.news.retention_days = self.news_manager.retention_days;
        config.news.source_enabled = self
            .news_manager
            .sources
            .iter()
            .map(|source| (source.name.clone(), source.enabled))
            .collect();
        config.news.source_schedule = self
            .news_manager
            .sources
            .iter()
            .map(|source| (source.name.clone(), source.schedule.clone()))
            .collect();
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

    fn trim_job_history(&mut self) {
        if self.jobs.len() > self.max_job_history {
            let remove = self.jobs.len() - self.max_job_history;
            self.jobs.drain(0..remove);
        }
    }

    fn save_job_history(&self) -> CoreResult<()> {
        if let Some(parent) = self.conversion_job_history_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                CoreError::Io("create conversion history parent".to_string(), err)
            })?;
        }
        let mut lines = Vec::with_capacity(self.jobs.len());
        for job in &self.jobs {
            let status = job.status.label();
            let output = job
                .output_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default();
            lines.push(format!(
                "{}\t{}\t{}\t{}\t{}",
                job.id,
                job.kind.label(),
                status,
                output.replace('\t', " "),
                job.name.replace('\t', " "),
            ));
        }
        fs::write(&self.conversion_job_history_path, lines.join("\n"))
            .map_err(|err| CoreError::Io("write conversion job history".to_string(), err))?;
        Ok(())
    }

    fn write_job_logs(&self, job_id: u64, logs: &[String]) -> CoreResult<PathBuf> {
        fs::create_dir_all(&self.conversion_job_logs_dir)
            .map_err(|err| CoreError::Io("create job logs dir".to_string(), err))?;
        let path = self
            .conversion_job_logs_dir
            .join(format!("job-{job_id}.log"));
        fs::write(&path, logs.join("\n"))
            .map_err(|err| CoreError::Io("write job log".to_string(), err))?;
        Ok(path)
    }

    fn load_job_history(&mut self) -> CoreResult<()> {
        if !self.conversion_job_history_path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(&self.conversion_job_history_path)
            .map_err(|err| CoreError::Io("read conversion job history".to_string(), err))?;
        for line in content.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 5 {
                continue;
            }
            let id = parts[0].parse::<u64>().unwrap_or(0);
            let kind = match parts[1] {
                "Convert" => JobKind::Convert,
                "Save to disk" => JobKind::SaveToDisk,
                _ => JobKind::Generic,
            };
            let status = match parts[2] {
                "Queued" => JobStatus::Queued,
                "Running" => JobStatus::Running,
                "Paused" => JobStatus::Paused,
                "Cancelled" => JobStatus::Cancelled,
                "Failed" => JobStatus::Failed,
                _ => JobStatus::Completed,
            };
            let output = if parts[3].trim().is_empty() {
                None
            } else {
                Some(PathBuf::from(parts[3]))
            };
            let name = parts[4].to_string();
            self.jobs.push(JobEntry {
                id,
                name,
                kind,
                status,
                progress: if matches!(status, JobStatus::Completed) {
                    1.0
                } else {
                    0.0
                },
                created_at: 0.0,
                updated_at: 0.0,
                output_path: output,
                details: Vec::new(),
                logs: Vec::new(),
                retry_action: None,
            });
            if id >= self.next_job_id {
                self.next_job_id = id + 1;
            }
        }
        self.trim_job_history();
        Ok(())
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
            kind: JobKind::Generic,
            status: JobStatus::Queued,
            progress: 0.0,
            created_at: now,
            updated_at: now,
            output_path: None,
            details: Vec::new(),
            logs: Vec::new(),
            retry_action: None,
        };
        self.next_job_id += 1;
        self.jobs.push(job);
        self.push_toast(&format!("Queued job: {name}"), ToastLevel::Info);
    }

    fn record_conversion_job(
        &mut self,
        name: String,
        status: JobStatus,
        output_path: Option<PathBuf>,
        mut details: Vec<(String, String)>,
        logs: Vec<String>,
        retry_action: Option<JobRetryAction>,
    ) {
        let now = self.last_tick;
        let job_id = self.next_job_id;
        if let Ok(log_path) = self.write_job_logs(job_id, &logs) {
            details.push(("log_file".to_string(), log_path.display().to_string()));
        }
        let job = JobEntry {
            id: job_id,
            name,
            kind: JobKind::Convert,
            status,
            progress: if matches!(status, JobStatus::Completed) {
                1.0
            } else {
                0.0
            },
            created_at: now,
            updated_at: now,
            output_path,
            details,
            logs,
            retry_action,
        };
        self.next_job_id += 1;
        self.jobs.push(job);
        self.trim_job_history();
        let _ = self.save_job_history();
    }

    fn record_save_to_disk_job(
        &mut self,
        name: String,
        status: JobStatus,
        output_path: Option<PathBuf>,
        mut details: Vec<(String, String)>,
        logs: Vec<String>,
        retry_action: Option<JobRetryAction>,
    ) {
        let now = self.last_tick;
        let job_id = self.next_job_id;
        if let Ok(log_path) = self.write_job_logs(job_id, &logs) {
            details.push(("log_file".to_string(), log_path.display().to_string()));
        }
        let job = JobEntry {
            id: job_id,
            name,
            kind: JobKind::SaveToDisk,
            status,
            progress: if matches!(status, JobStatus::Completed) {
                1.0
            } else {
                0.0
            },
            created_at: now,
            updated_at: now,
            output_path,
            details,
            logs,
            retry_action,
        };
        self.next_job_id += 1;
        self.jobs.push(job);
        self.trim_job_history();
        let _ = self.save_job_history();
    }

    fn enqueue_retry_job(&mut self, name: String, action: JobRetryAction) {
        let now = self.last_tick;
        let mut details = Vec::new();
        match &action {
            JobRetryAction::Convert {
                ids,
                output_format,
                output_dir,
            } => {
                details.push(("kind".to_string(), "convert".to_string()));
                details.push(("books".to_string(), ids.len().to_string()));
                details.push(("format".to_string(), output_format.clone()));
                details.push(("output_dir".to_string(), output_dir.display().to_string()));
            }
            JobRetryAction::SaveToDisk { ids, output_dir } => {
                details.push(("kind".to_string(), "save_to_disk".to_string()));
                details.push(("books".to_string(), ids.len().to_string()));
                details.push(("output_dir".to_string(), output_dir.display().to_string()));
            }
        }
        let kind = match action {
            JobRetryAction::Convert { .. } => JobKind::Convert,
            JobRetryAction::SaveToDisk { .. } => JobKind::SaveToDisk,
        };
        let job = JobEntry {
            id: self.next_job_id,
            name,
            kind,
            status: JobStatus::Queued,
            progress: 0.0,
            created_at: now,
            updated_at: now,
            output_path: None,
            details,
            logs: vec!["queued from retry/clone".to_string()],
            retry_action: Some(action),
        };
        self.next_job_id += 1;
        self.jobs.push(job);
        self.trim_job_history();
        let _ = self.save_job_history();
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
                    let mut move_up: Option<usize> = None;
                    let mut move_down: Option<usize> = None;
                    let mut save_dirty = false;
                    let mut queued_retries: Vec<(String, JobRetryAction)> = Vec::new();
                    let total_jobs = self.jobs.len();
                    for idx in 0..total_jobs {
                        let job = &mut self.jobs[idx];
                        ui.horizontal(|ui| {
                            ui.label(format!("#{} {}", job.id, job.name));
                            ui.label(job.status.label());
                            ui.label(job.kind.label());
                            if matches!(job.status, JobStatus::Queued) && idx > 0 {
                                if ui.small_button("↑").clicked() {
                                    move_up = Some(idx);
                                }
                            }
                            if matches!(job.status, JobStatus::Queued) && idx + 1 < total_jobs {
                                if ui.small_button("↓").clicked() {
                                    move_down = Some(idx);
                                }
                            }
                        });
                        ui.add(
                            egui::ProgressBar::new(job.progress)
                                .show_percentage()
                                .animate(true),
                        );
                        if let Some(path) = &job.output_path {
                            ui.horizontal(|ui| {
                                ui.label(format!("Output: {}", path.display()));
                                if ui.small_button("Open output").clicked() {
                                    if let Err(err) = open_path(path) {
                                        toasts.push((
                                            format!("open output failed: {err}"),
                                            ToastLevel::Warn,
                                        ));
                                    }
                                }
                            });
                        }
                        if !job.details.is_empty() {
                            egui::CollapsingHeader::new("Details")
                                .id_salt(format!("job-details-{}", job.id))
                                .show(ui, |ui| {
                                    for (name, value) in &job.details {
                                        ui.horizontal(|ui| {
                                            ui.monospace(name);
                                            ui.label(value);
                                        });
                                    }
                                });
                        }
                        if !job.logs.is_empty() {
                            egui::CollapsingHeader::new("Logs")
                                .id_salt(format!("job-logs-{}", job.id))
                                .show(ui, |ui| {
                                    egui::ScrollArea::vertical()
                                        .max_height(100.0)
                                        .show(ui, |ui| {
                                            for line in &job.logs {
                                                ui.monospace(line);
                                            }
                                        });
                                });
                        }
                        ui.horizontal(|ui| {
                            if matches!(job.status, JobStatus::Running) {
                                if ui.button("Pause").clicked() {
                                    job.status = JobStatus::Paused;
                                    toasts
                                        .push((format!("Paused job {}", job.id), ToastLevel::Warn));
                                    save_dirty = true;
                                }
                            } else if matches!(job.status, JobStatus::Paused) {
                                if ui.button("Resume").clicked() {
                                    job.status = JobStatus::Running;
                                    toasts.push((
                                        format!("Resumed job {}", job.id),
                                        ToastLevel::Info,
                                    ));
                                    save_dirty = true;
                                }
                            }
                            if !matches!(job.status, JobStatus::Completed | JobStatus::Cancelled) {
                                if ui.button("Cancel").clicked() {
                                    job.status = JobStatus::Cancelled;
                                    toasts.push((
                                        format!("Cancelled job {}", job.id),
                                        ToastLevel::Warn,
                                    ));
                                    save_dirty = true;
                                }
                            }
                            if matches!(job.status, JobStatus::Completed | JobStatus::Failed) {
                                if ui.button("Clone").clicked() {
                                    if let Some(action) = job.retry_action.clone() {
                                        let name = format!("Clone of {}", job.name);
                                        queued_retries.push((name, action));
                                        toasts.push((
                                            format!("Cloned job {}", job.id),
                                            ToastLevel::Info,
                                        ));
                                    }
                                }
                                if ui.button("Retry").clicked() {
                                    if let Some(action) = job.retry_action.clone() {
                                        let name = format!("Retry of {}", job.name);
                                        queued_retries.push((name, action));
                                        toasts.push((
                                            format!("Retried job {}", job.id),
                                            ToastLevel::Info,
                                        ));
                                    }
                                }
                            }
                        });
                        ui.separator();
                    }
                    if let Some(idx) = move_up {
                        self.jobs.swap(idx, idx - 1);
                        save_dirty = true;
                    }
                    if let Some(idx) = move_down {
                        self.jobs.swap(idx, idx + 1);
                        save_dirty = true;
                    }
                    for (name, action) in queued_retries {
                        self.enqueue_retry_job(name, action);
                        save_dirty = true;
                    }
                    for (message, level) in toasts {
                        self.push_toast(&message, level);
                    }
                    if save_dirty {
                        let _ = self.save_job_history();
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
    author_sort: String,
    series_sort: String,
    series_name: String,
    series_index: f64,
    identifiers: String,
    isbn: String,
    comment: String,
    publisher: String,
    imprint: String,
    edition: String,
    rights: String,
    languages: String,
    timestamp: String,
    pubdate: String,
    pubdate_year: i32,
    pubdate_month: u8,
    pubdate_day: u8,
    last_modified: String,
    rating: i64,
    uuid: String,
}

#[derive(Debug, Clone)]
struct CustomEditField {
    label: String,
    name: String,
    datatype: String,
    value: String,
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
    search_scope: ReaderSearchScope,
    search_highlighting: bool,
    search_results: Vec<ReaderSearchResult>,
    search_result_cursor: Option<usize>,
    last_match: Option<usize>,
    toc: Vec<ReaderTocEntry>,
    bookmarks: Vec<ReaderBookmark>,
    selected_text: String,
    highlight_note: String,
    highlight_color: ReaderHighlightColor,
    annotations: Vec<ReaderAnnotation>,
    go_to_page_input: usize,
    go_to_percent_input: f32,
    continuous_scroll: bool,
    fit_mode: ReaderFitMode,
    image_zoom: f32,
    font_family: ReaderFontFamily,
    margin_scale: f32,
    preset: ReaderPreset,
    justify_text: bool,
    hyphenation: bool,
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
            search_scope: ReaderSearchScope::CurrentBook,
            search_highlighting: true,
            search_results: Vec::new(),
            search_result_cursor: None,
            last_match: None,
            toc: Vec::new(),
            bookmarks: Vec::new(),
            selected_text: String::new(),
            highlight_note: String::new(),
            highlight_color: ReaderHighlightColor::Yellow,
            annotations: Vec::new(),
            go_to_page_input: 1,
            go_to_percent_input: 0.0,
            continuous_scroll: false,
            fit_mode: ReaderFitMode::FitWidth,
            image_zoom: 1.0,
            font_family: ReaderFontFamily::Sans,
            margin_scale: 1.0,
            preset: ReaderPreset::Balanced,
            justify_text: false,
            hyphenation: false,
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
        self.search_results.clear();
        self.search_result_cursor = None;
        self.temp_path = temp_path;
        self.content =
            ReaderContent::from_path(path, format, self.page_chars).unwrap_or_else(|err| {
                self.error = Some(err);
                ReaderContent::Unsupported
            });
        self.rebuild_toc();
        self.bookmarks.clear();
        self.annotations.clear();
        self.open = true;
        self.push_recent(book_id, title, self.page);
    }

    fn open_virtual_text(&mut self, book_id: i64, title: &str, raw: &str) {
        if let Some(path) = self.temp_path.take() {
            let _ = fs::remove_file(path);
        }
        self.book_id = Some(book_id);
        self.title = title.to_string();
        self.format = "txt".to_string();
        self.page = 0;
        self.error = None;
        self.search_query.clear();
        self.last_match = None;
        self.search_results.clear();
        self.search_result_cursor = None;
        self.temp_path = None;
        self.content = ReaderContent::Text {
            raw: raw.to_string(),
            pages: paginate_text(raw, self.page_chars),
        };
        self.rebuild_toc();
        self.bookmarks.clear();
        self.annotations.clear();
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
        self.search_results.clear();
        self.search_result_cursor = None;
        self.toc.clear();
        self.bookmarks.clear();
        self.annotations.clear();
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
            self.rebuild_toc();
        }
    }

    fn find_next(&mut self) {
        let query = self.search_query.trim().to_lowercase();
        if query.is_empty() {
            self.last_match = None;
            self.search_results.clear();
            self.search_result_cursor = None;
            return;
        }
        if let ReaderContent::Text { pages, .. } = &self.content {
            self.search_results = pages
                .iter()
                .enumerate()
                .filter(|(_, page)| page.to_lowercase().contains(&query))
                .map(|(idx, page)| ReaderSearchResult {
                    label: format!(
                        "Page {}: {}",
                        idx + 1,
                        page.lines().next().unwrap_or("").trim()
                    ),
                    page: Some(idx),
                    book_id: self.book_id,
                })
                .collect();
            let start = self.last_match.unwrap_or(0);
            for idx in start..pages.len() {
                if pages[idx].to_lowercase().contains(&query) {
                    self.page = idx;
                    self.last_match = Some(idx + 1);
                    self.search_result_cursor = self
                        .search_results
                        .iter()
                        .position(|result| result.page == Some(idx));
                    return;
                }
            }
            self.last_match = Some(0);
            self.search_result_cursor = None;
        }
    }

    fn jump_to(&mut self, book_id: i64, page: usize) {
        if self.book_id == Some(book_id) {
            self.page = page.min(self.page_count().saturating_sub(1));
        }
    }

    fn go_to_page(&mut self, page: usize) {
        let count = self.page_count();
        if count == 0 {
            return;
        }
        self.page = page.saturating_sub(1).min(count.saturating_sub(1));
    }

    fn go_to_percent(&mut self, percent: f32) {
        let count = self.page_count();
        if count == 0 {
            return;
        }
        let clamped = percent.clamp(0.0, 100.0) / 100.0;
        let page = ((count.saturating_sub(1)) as f32 * clamped).round() as usize;
        self.page = page.min(count.saturating_sub(1));
    }

    fn next_chapter(&mut self) {
        if let Some(next) = self.toc.iter().find(|entry| entry.page > self.page) {
            self.page = next.page;
        } else {
            self.next_page();
        }
    }

    fn prev_chapter(&mut self) {
        if let Some(prev) = self.toc.iter().rev().find(|entry| entry.page < self.page) {
            self.page = prev.page;
        } else {
            self.prev_page();
        }
    }

    fn add_bookmark(&mut self) {
        self.bookmarks.push(ReaderBookmark {
            title: format!("Page {}", self.page + 1),
            page: self.page,
        });
    }

    fn remove_bookmark(&mut self, idx: usize) {
        if idx < self.bookmarks.len() {
            self.bookmarks.remove(idx);
        }
    }

    fn add_annotation(&mut self) {
        let text = self.selected_text.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.annotations.push(ReaderAnnotation {
            page: self.page,
            text,
            note: self.highlight_note.trim().to_string(),
            color: self.highlight_color,
        });
        self.highlight_note.clear();
    }

    fn apply_preset(&mut self) {
        match self.preset {
            ReaderPreset::Balanced => {
                self.font_size = 18.0;
                self.line_spacing = 1.35;
                self.margin_scale = 1.0;
            }
            ReaderPreset::Focus => {
                self.font_size = 20.0;
                self.line_spacing = 1.5;
                self.margin_scale = 1.2;
            }
            ReaderPreset::Dense => {
                self.font_size = 15.0;
                self.line_spacing = 1.2;
                self.margin_scale = 0.8;
            }
        }
    }

    fn rebuild_toc(&mut self) {
        self.toc.clear();
        if let ReaderContent::Text { pages, .. } = &self.content {
            for (idx, page) in pages.iter().enumerate() {
                if let Some(line) = page
                    .lines()
                    .find(|line| line.trim_start().starts_with('#'))
                    .map(|line| line.trim_start_matches('#').trim())
                {
                    if !line.is_empty() {
                        self.toc.push(ReaderTocEntry {
                            title: line.to_string(),
                            page: idx,
                        });
                    }
                }
            }
        }
        if self.toc.is_empty() {
            self.toc.push(ReaderTocEntry {
                title: "Start".to_string(),
                page: 0,
            });
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
                let show_query = if self.search_highlighting {
                    self.search_query.as_str()
                } else {
                    ""
                };
                if self.continuous_scroll {
                    for raw in pages {
                        render_text_with_highlight_and_style(
                            ui,
                            raw,
                            show_query,
                            self.font_size,
                            self.font_family,
                            self.justify_text,
                            self.hyphenation,
                        );
                        ui.add_space(8.0 * self.margin_scale);
                    }
                } else {
                    let raw_text = pages.get(self.page).map(|s| s.as_str()).unwrap_or("");
                    let page_text = if self.line_spacing > 1.3 {
                        raw_text.replace('\n', "\n\n")
                    } else {
                        raw_text.to_string()
                    };
                    render_text_with_highlight_and_style(
                        ui,
                        &page_text,
                        show_query,
                        self.font_size,
                        self.font_family,
                        self.justify_text,
                        self.hyphenation,
                    );
                }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReaderSearchScope {
    CurrentBook,
    Library,
}

#[derive(Debug, Clone)]
struct ReaderSearchResult {
    label: String,
    page: Option<usize>,
    book_id: Option<i64>,
}

#[derive(Debug, Clone)]
struct ReaderTocEntry {
    title: String,
    page: usize,
}

#[derive(Debug, Clone)]
struct ReaderBookmark {
    title: String,
    page: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReaderHighlightColor {
    Yellow,
    Green,
    Blue,
    Pink,
}

#[derive(Debug, Clone)]
struct ReaderAnnotation {
    page: usize,
    text: String,
    note: String,
    color: ReaderHighlightColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReaderFitMode {
    FitWidth,
    FitPage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReaderFontFamily {
    Sans,
    Serif,
    Monospace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReaderPreset {
    Balanced,
    Focus,
    Dense,
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
    input_profile: String,
    output_profile: String,
    heuristic_enable: bool,
    heuristic_unwrap_lines: bool,
    heuristic_delete_blank_lines: bool,
    page_margin_left: f32,
    page_margin_right: f32,
    page_margin_top: f32,
    page_margin_bottom: f32,
    embed_fonts: bool,
    subset_fonts: bool,
    cover_policy: String,
    warn_unsupported_options: bool,
    preset_name: String,
    presets: HashMap<String, ConvertPreset>,
}

impl Default for ConvertBooksDialogState {
    fn default() -> Self {
        Self {
            open: false,
            output_format: "epub".to_string(),
            output_dir: String::new(),
            add_to_library: false,
            keep_output: true,
            input_profile: "default".to_string(),
            output_profile: "default".to_string(),
            heuristic_enable: true,
            heuristic_unwrap_lines: true,
            heuristic_delete_blank_lines: false,
            page_margin_left: 5.0,
            page_margin_right: 5.0,
            page_margin_top: 5.0,
            page_margin_bottom: 5.0,
            embed_fonts: false,
            subset_fonts: true,
            cover_policy: "keep".to_string(),
            warn_unsupported_options: true,
            preset_name: String::new(),
            presets: HashMap::new(),
        }
    }
}

impl ConvertBooksDialogState {
    fn apply_defaults(&mut self, config: &ControlPlane) {
        self.output_format = config.conversion.default_output_format.clone();
        self.output_dir = config.conversion.output_dir.display().to_string();
        self.add_to_library = false;
        self.keep_output = true;
        self.input_profile = config.conversion.default_input_profile.clone();
        self.output_profile = config.conversion.default_output_profile.clone();
        self.heuristic_enable = config.conversion.heuristic_enable;
        self.heuristic_unwrap_lines = config.conversion.heuristic_unwrap_lines;
        self.heuristic_delete_blank_lines = config.conversion.heuristic_delete_blank_lines;
        self.page_margin_left = config.conversion.page_margin_left;
        self.page_margin_right = config.conversion.page_margin_right;
        self.page_margin_top = config.conversion.page_margin_top;
        self.page_margin_bottom = config.conversion.page_margin_bottom;
        self.embed_fonts = config.conversion.embed_fonts;
        self.subset_fonts = config.conversion.subset_fonts;
        self.cover_policy = config.conversion.cover_policy.clone();
        self.warn_unsupported_options = config.conversion.warn_unsupported_options;
    }

    fn save_preset(&mut self, name: &str) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return;
        }
        let preset = ConvertPreset {
            output_format: self.output_format.clone(),
            output_dir: self.output_dir.clone(),
            input_profile: self.input_profile.clone(),
            output_profile: self.output_profile.clone(),
            heuristic_enable: self.heuristic_enable,
            heuristic_unwrap_lines: self.heuristic_unwrap_lines,
            heuristic_delete_blank_lines: self.heuristic_delete_blank_lines,
            page_margin_left: self.page_margin_left,
            page_margin_right: self.page_margin_right,
            page_margin_top: self.page_margin_top,
            page_margin_bottom: self.page_margin_bottom,
            embed_fonts: self.embed_fonts,
            subset_fonts: self.subset_fonts,
            cover_policy: self.cover_policy.clone(),
        };
        self.presets.insert(trimmed.to_string(), preset);
    }

    fn load_preset(&mut self, name: &str) -> bool {
        let Some(preset) = self.presets.get(name).cloned() else {
            return false;
        };
        self.output_format = preset.output_format;
        self.output_dir = preset.output_dir;
        self.input_profile = preset.input_profile;
        self.output_profile = preset.output_profile;
        self.heuristic_enable = preset.heuristic_enable;
        self.heuristic_unwrap_lines = preset.heuristic_unwrap_lines;
        self.heuristic_delete_blank_lines = preset.heuristic_delete_blank_lines;
        self.page_margin_left = preset.page_margin_left;
        self.page_margin_right = preset.page_margin_right;
        self.page_margin_top = preset.page_margin_top;
        self.page_margin_bottom = preset.page_margin_bottom;
        self.embed_fonts = preset.embed_fonts;
        self.subset_fonts = preset.subset_fonts;
        self.cover_policy = preset.cover_policy;
        true
    }
}

#[derive(Debug, Clone)]
struct SaveToDiskDialogState {
    open: bool,
    output_dir: String,
    export_all_formats: bool,
    path_template: String,
    conflict_policy: String,
    preset_name: String,
    presets: HashMap<String, SaveToDiskPreset>,
}

impl Default for SaveToDiskDialogState {
    fn default() -> Self {
        Self {
            open: false,
            output_dir: String::new(),
            export_all_formats: true,
            path_template: "{title}-{id}.{format}".to_string(),
            conflict_policy: "rename".to_string(),
            preset_name: String::new(),
            presets: HashMap::new(),
        }
    }
}

impl SaveToDiskDialogState {
    fn apply_defaults(&mut self, config: &ControlPlane) {
        self.output_dir = config.conversion.output_dir.display().to_string();
        self.export_all_formats = true;
        self.path_template = config.conversion.save_to_disk_template.clone();
        self.conflict_policy = config.conversion.save_to_disk_conflict_policy.clone();
        self.presets = config
            .conversion
            .save_to_disk_presets
            .iter()
            .map(|(name, template)| {
                (
                    name.clone(),
                    SaveToDiskPreset {
                        template: template.clone(),
                        conflict_policy: self.conflict_policy.clone(),
                        export_all_formats: true,
                    },
                )
            })
            .collect();
    }

    fn save_preset(&mut self, name: &str) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return;
        }
        self.presets.insert(
            trimmed.to_string(),
            SaveToDiskPreset {
                template: self.path_template.clone(),
                conflict_policy: self.conflict_policy.clone(),
                export_all_formats: self.export_all_formats,
            },
        );
    }

    fn load_preset(&mut self, name: &str) -> bool {
        let Some(preset) = self.presets.get(name).cloned() else {
            return false;
        };
        self.path_template = preset.template;
        self.conflict_policy = preset.conflict_policy;
        self.export_all_formats = preset.export_all_formats;
        true
    }
}

#[derive(Debug, Clone)]
struct DeviceSyncDialogState {
    open: bool,
    devices: Vec<DeviceInfo>,
    selected_device: Option<usize>,
    destination_name: String,
    auto_convert: bool,
    overwrite: bool,
    sync_metadata: bool,
    sync_cover: bool,
    queue: Vec<DeviceQueueRow>,
    error: Option<String>,
}

impl Default for DeviceSyncDialogState {
    fn default() -> Self {
        Self {
            open: false,
            devices: Vec::new(),
            selected_device: None,
            destination_name: String::new(),
            auto_convert: false,
            overwrite: false,
            sync_metadata: true,
            sync_cover: true,
            queue: Vec::new(),
            error: None,
        }
    }
}

impl DeviceSyncDialogState {
    fn apply_defaults(&mut self, config: &ControlPlane) {
        self.error = None;
        self.queue.clear();
        self.auto_convert = config.device.send_auto_convert;
        self.overwrite = config.device.send_overwrite;
        self.sync_metadata = config.device.sync_metadata;
        self.sync_cover = config.device.sync_cover;
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
struct DeviceQueueRow {
    item: String,
    status: String,
}

#[derive(Debug, Clone)]
struct DeviceManagerDialogState {
    open: bool,
    devices: Vec<DeviceInfo>,
    selected_device: Option<usize>,
    file_filter: String,
    files: Vec<PathBuf>,
    selected_file: Option<PathBuf>,
    collections: Vec<String>,
    last_scan_error: Option<String>,
    driver_backend: String,
    connection_timeout_ms: u64,
}

impl Default for DeviceManagerDialogState {
    fn default() -> Self {
        Self {
            open: false,
            devices: Vec::new(),
            selected_device: None,
            file_filter: String::new(),
            files: Vec::new(),
            selected_file: None,
            collections: Vec::new(),
            last_scan_error: None,
            driver_backend: "auto".to_string(),
            connection_timeout_ms: 5_000,
        }
    }
}

impl DeviceManagerDialogState {
    fn apply_defaults(&mut self, config: &ControlPlane) {
        self.driver_backend = config.device.driver_backend.clone();
        self.connection_timeout_ms = config.device.connection_timeout_ms;
        match detect_devices(&config.device) {
            Ok(devices) => {
                self.devices = devices;
                self.selected_device = if self.devices.is_empty() {
                    None
                } else {
                    Some(0)
                };
                self.last_scan_error = None;
            }
            Err(err) => {
                self.devices.clear();
                self.selected_device = None;
                self.last_scan_error = Some(err.to_string());
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
struct DeviceFileDeleteDialogState {
    open: bool,
    path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
struct FetchFromDeviceDialogState {
    open: bool,
    file_path: Option<PathBuf>,
    mode: IngestMode,
}

#[derive(Debug, Clone)]
struct NewsSourceRow {
    name: String,
    enabled: bool,
    schedule: String,
    status: String,
}

#[derive(Debug, Clone)]
struct NewsDownloadRow {
    source: String,
    path: PathBuf,
    status: String,
}

#[derive(Debug, Clone)]
struct NewsDialogState {
    open: bool,
    source_filter: String,
    sources: Vec<NewsSourceRow>,
    selected_source: Option<usize>,
    recipe_import_path: String,
    downloads: Vec<NewsDownloadRow>,
    selected_download: Option<usize>,
    history_lines: Vec<String>,
    retention_days: u64,
    auto_delete: bool,
}

impl Default for NewsDialogState {
    fn default() -> Self {
        Self {
            open: false,
            source_filter: String::new(),
            sources: Vec::new(),
            selected_source: None,
            recipe_import_path: String::new(),
            downloads: Vec::new(),
            selected_download: None,
            history_lines: Vec::new(),
            retention_days: 30,
            auto_delete: true,
        }
    }
}

impl NewsDialogState {
    fn apply_defaults(&mut self, config: &ControlPlane) {
        self.retention_days = config.news.retention_days;
        self.auto_delete = config.news.auto_delete;
    }
}

#[derive(Debug, Clone)]
struct ManageTagsDialogState {
    open: bool,
    tags: Vec<CategoryCount>,
    rename_from: String,
    rename_to: String,
    merge_from: String,
    merge_to: String,
    delete_name: String,
    bulk_tag: String,
    needs_refresh: bool,
}

impl Default for ManageTagsDialogState {
    fn default() -> Self {
        Self {
            open: false,
            tags: Vec::new(),
            rename_from: String::new(),
            rename_to: String::new(),
            merge_from: String::new(),
            merge_to: String::new(),
            delete_name: String::new(),
            bulk_tag: String::new(),
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
    merge_from: String,
    merge_to: String,
    delete_name: String,
    renumber_name: String,
    renumber_start: f64,
    renumber_step: f64,
    needs_refresh: bool,
}

impl Default for ManageSeriesDialogState {
    fn default() -> Self {
        Self {
            open: false,
            series: Vec::new(),
            rename_from: String::new(),
            rename_to: String::new(),
            merge_from: String::new(),
            merge_to: String::new(),
            delete_name: String::new(),
            renumber_name: String::new(),
            renumber_start: 1.0,
            renumber_step: 1.0,
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
    edit_label: String,
    edit_name: String,
    edit_datatype: String,
    edit_display: String,
    edit_editable: bool,
    edit_is_multiple: bool,
    edit_normalized: bool,
    value_filter: String,
    import_path: String,
    export_path: String,
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
            edit_label: String::new(),
            edit_name: String::new(),
            edit_datatype: "text".to_string(),
            edit_display: String::new(),
            edit_editable: true,
            edit_is_multiple: false,
            edit_normalized: false,
            value_filter: String::new(),
            import_path: String::new(),
            export_path: String::new(),
            needs_refresh: true,
        }
    }
}

#[derive(Debug, Clone)]
struct ManageVirtualLibrariesDialogState {
    open: bool,
    searches: Vec<(String, String)>,
    folders: HashMap<String, Vec<(String, String)>>,
    new_name: String,
    new_folder: String,
    new_query: String,
    builder_field: String,
    builder_op: String,
    builder_value: String,
    delete_name: String,
    import_path: String,
    export_path: String,
    assign_name: String,
    unassign_name: String,
    needs_refresh: bool,
}

impl Default for ManageVirtualLibrariesDialogState {
    fn default() -> Self {
        Self {
            open: false,
            searches: Vec::new(),
            folders: HashMap::new(),
            new_name: String::new(),
            new_folder: String::new(),
            new_query: String::new(),
            builder_field: "title".to_string(),
            builder_op: "contains".to_string(),
            builder_value: String::new(),
            delete_name: String::new(),
            import_path: String::new(),
            export_path: String::new(),
            assign_name: String::new(),
            unassign_name: String::new(),
            needs_refresh: true,
        }
    }
}

#[derive(Debug, Clone)]
struct PluginEntry {
    id: String,
    name: String,
    version: String,
    latest_version: String,
    author: String,
    description: String,
    enabled: bool,
    dependencies: Vec<String>,
    status: String,
    error: Option<String>,
    setting_key: String,
    setting_value: String,
    logs: Vec<String>,
}

#[derive(Debug, Clone)]
struct PluginManagerDialogState {
    open: bool,
    search: String,
    selected: Option<String>,
    install_id: String,
    install_name: String,
    install_version: String,
    install_author: String,
    install_description: String,
    remove_id: String,
    status_message: String,
    plugins: Vec<PluginEntry>,
}

impl Default for PluginManagerDialogState {
    fn default() -> Self {
        Self {
            open: false,
            search: String::new(),
            selected: None,
            install_id: String::new(),
            install_name: String::new(),
            install_version: "0.1.0".to_string(),
            install_author: String::new(),
            install_description: String::new(),
            remove_id: String::new(),
            status_message: String::new(),
            plugins: vec![
                PluginEntry {
                    id: "builtin.metadata".to_string(),
                    name: "Metadata Sources".to_string(),
                    version: "1.0.0".to_string(),
                    latest_version: "1.1.0".to_string(),
                    author: "Caliberate".to_string(),
                    description: "Built-in metadata source integration.".to_string(),
                    enabled: true,
                    dependencies: vec![],
                    status: "healthy".to_string(),
                    error: None,
                    setting_key: "timeout_ms".to_string(),
                    setting_value: "5000".to_string(),
                    logs: vec!["loaded plugin".to_string()],
                },
                PluginEntry {
                    id: "builtin.converter".to_string(),
                    name: "Converter Hooks".to_string(),
                    version: "1.0.0".to_string(),
                    latest_version: "1.0.0".to_string(),
                    author: "Caliberate".to_string(),
                    description: "Conversion pipeline extension hooks.".to_string(),
                    enabled: true,
                    dependencies: vec!["builtin.metadata".to_string()],
                    status: "healthy".to_string(),
                    error: None,
                    setting_key: "max_workers".to_string(),
                    setting_value: "2".to_string(),
                    logs: vec!["initialized hook registry".to_string()],
                },
            ],
        }
    }
}

#[derive(Debug, Clone)]
struct MetadataDownloadDialogState {
    open: bool,
    cover_only: bool,
    source: String,
    merge_mode: bool,
    merge_tags: bool,
    merge_identifiers: bool,
    overwrite_title: bool,
    overwrite_authors: bool,
    overwrite_publisher: bool,
    overwrite_language: bool,
    overwrite_pubdate: bool,
    overwrite_comment: bool,
    progress: f32,
    failed: bool,
    results: Vec<MetadataDownloadResult>,
    queue_rows: Vec<MetadataQueueRow>,
    selected_book_id: Option<i64>,
    selected_cover: usize,
    selected_cover_url: Option<String>,
    last_error: Option<String>,
    source_openlibrary: bool,
    source_google: bool,
    source_amazon: bool,
    source_isbndb: bool,
}

impl MetadataDownloadDialogState {
    fn default_from_config(config: &MetadataDownloadConfig) -> Self {
        let source = first_enabled_source(config).unwrap_or_else(|| "openlibrary".to_string());
        Self {
            open: false,
            cover_only: false,
            source,
            merge_mode: true,
            merge_tags: config.merge_tags_default,
            merge_identifiers: config.merge_identifiers_default,
            overwrite_title: config.overwrite_title_default,
            overwrite_authors: config.overwrite_authors_default,
            overwrite_publisher: config.overwrite_publisher_default,
            overwrite_language: config.overwrite_language_default,
            overwrite_pubdate: config.overwrite_pubdate_default,
            overwrite_comment: config.overwrite_comment_default,
            progress: 0.0,
            failed: false,
            results: Vec::new(),
            queue_rows: Vec::new(),
            selected_book_id: None,
            selected_cover: 1,
            selected_cover_url: None,
            last_error: None,
            source_openlibrary: config.openlibrary_enabled,
            source_google: config.googlebooks_enabled,
            source_amazon: false,
            source_isbndb: false,
        }
    }
}

impl Default for MetadataDownloadDialogState {
    fn default() -> Self {
        Self::default_from_config(&MetadataDownloadConfig::default())
    }
}

#[derive(Debug, Clone)]
struct MetadataDownloadResult {
    book_id: i64,
    provider: String,
    metadata: DownloadedMetadata,
}

impl MetadataDownloadResult {
    fn preview_line(&self) -> String {
        let title = self
            .metadata
            .title
            .as_deref()
            .unwrap_or("<untitled>")
            .to_string();
        let authors = if self.metadata.authors.is_empty() {
            "<unknown author>".to_string()
        } else {
            self.metadata.authors.join(", ")
        };
        format!("{title} — {authors}")
    }
}

#[derive(Debug, Clone)]
struct MetadataQueueRow {
    book_id: i64,
    title: String,
    status: MetadataQueueStatus,
    error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetadataQueueStatus {
    Pending,
    Running,
    Success,
    Failed,
}

impl MetadataQueueStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Success => "success",
            Self::Failed => "failed",
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
    PasteCoverClipboard,
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
    kind: JobKind,
    status: JobStatus,
    progress: f32,
    created_at: f64,
    updated_at: f64,
    output_path: Option<PathBuf>,
    details: Vec<(String, String)>,
    logs: Vec<String>,
    retry_action: Option<JobRetryAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JobKind {
    Generic,
    Convert,
    SaveToDisk,
}

impl JobKind {
    fn label(self) -> &'static str {
        match self {
            JobKind::Generic => "General",
            JobKind::Convert => "Convert",
            JobKind::SaveToDisk => "Save to disk",
        }
    }
}

#[derive(Debug, Clone)]
enum JobRetryAction {
    Convert {
        ids: Vec<i64>,
        output_format: String,
        output_dir: PathBuf,
    },
    SaveToDisk {
        ids: Vec<i64>,
        output_dir: PathBuf,
    },
}

#[derive(Debug, Clone)]
struct ConvertPreset {
    output_format: String,
    output_dir: String,
    input_profile: String,
    output_profile: String,
    heuristic_enable: bool,
    heuristic_unwrap_lines: bool,
    heuristic_delete_blank_lines: bool,
    page_margin_left: f32,
    page_margin_right: f32,
    page_margin_top: f32,
    page_margin_bottom: f32,
    embed_fonts: bool,
    subset_fonts: bool,
    cover_policy: String,
}

#[derive(Debug, Clone)]
struct SaveToDiskPreset {
    template: String,
    conflict_policy: String,
    export_all_formats: bool,
}

#[derive(Debug, Clone)]
struct LibraryStatsSummary {
    formats: Vec<(String, usize)>,
    format_sizes: Vec<(String, u64)>,
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
            author_sort: String::new(),
            series_sort: String::new(),
            series_name: String::new(),
            series_index: 1.0,
            identifiers: String::new(),
            isbn: String::new(),
            comment: String::new(),
            publisher: String::new(),
            imprint: String::new(),
            edition: String::new(),
            rights: String::new(),
            languages: String::new(),
            timestamp: String::new(),
            pubdate: String::new(),
            pubdate_year: 2000,
            pubdate_month: 1,
            pubdate_day: 1,
            last_modified: String::new(),
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
        let pubdate_text = details.extras.pubdate.clone().unwrap_or_default();
        let (pubdate_year, pubdate_month, pubdate_day) =
            parse_date_parts(&pubdate_text).unwrap_or((2000, 1, 1));
        Self {
            title: details.book.title.clone(),
            authors: details.authors.join(", "),
            tags: details.tags.join(", "),
            author_sort: details.extras.author_sort.clone().unwrap_or_default(),
            series_sort: details.extras.sort.clone().unwrap_or_default(),
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
            imprint: String::new(),
            edition: String::new(),
            rights: String::new(),
            languages: details.extras.languages.join(", "),
            timestamp: details.extras.timestamp.clone().unwrap_or_default(),
            pubdate: pubdate_text,
            pubdate_year,
            pubdate_month,
            pubdate_day,
            last_modified: details.extras.last_modified.clone().unwrap_or_default(),
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

fn normalize_csv_list(text: &str, lowercase: bool) -> String {
    let mut seen = BTreeSet::new();
    let mut values = Vec::new();
    for item in text.split(',') {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized = if lowercase {
            trimmed.to_lowercase()
        } else {
            trimmed.to_string()
        };
        let key = normalized.to_lowercase();
        if seen.insert(key) {
            values.push(normalized);
        }
    }
    values.join(", ")
}

fn normalize_identifier_lines(text: &str) -> String {
    let mut normalized = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some((id_type, value)) = trimmed.split_once(':') {
            let id_type = id_type.trim().to_lowercase();
            let value = value.trim();
            if !id_type.is_empty() && !value.is_empty() {
                normalized.push(format!("{id_type}:{value}"));
            }
        }
    }
    cleanup_identifier_lines(&normalized.join("\n"))
}

fn derive_title_sort(title: &str) -> String {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let lower = trimmed.to_lowercase();
    for prefix in ["the ", "a ", "an "] {
        if lower.starts_with(prefix) && trimmed.len() > prefix.len() {
            return format!(
                "{}, {}",
                trimmed[prefix.len()..].trim(),
                &trimmed[..prefix.len() - 1]
            );
        }
    }
    trimmed.to_string()
}

fn current_date_ymd() -> String {
    let Ok(format) = time::format_description::parse("[year]-[month]-[day]") else {
        return String::new();
    };
    OffsetDateTime::now_utc()
        .format(&format)
        .unwrap_or_else(|_| String::new())
}

fn parse_date_parts(value: &str) -> Option<(i32, u8, u8)> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(dt) =
        time::OffsetDateTime::parse(trimmed, &time::format_description::well_known::Rfc3339)
    {
        return Some((dt.year(), dt.month() as u8, dt.day()));
    }
    let format = time::format_description::parse("[year]-[month]-[day]").ok()?;
    let date = time::Date::parse(trimmed, &format).ok()?;
    Some((date.year(), date.month() as u8, date.day()))
}

fn duplicate_csv_hint(field: &str, value: &str) -> Option<String> {
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for part in value.split(',') {
        let token = part.trim();
        if token.is_empty() {
            continue;
        }
        let key = token.to_lowercase();
        if !seen.insert(key.clone()) {
            duplicates.insert(key);
        }
    }
    if duplicates.is_empty() {
        None
    } else {
        Some(format!(
            "{field} contains duplicates: {}",
            duplicates.into_iter().collect::<Vec<_>>().join(", ")
        ))
    }
}

fn identifier_conflict_hint(value: &str) -> Option<String> {
    let mut by_type: HashMap<String, BTreeSet<String>> = HashMap::new();
    for line in value.lines() {
        let trimmed = line.trim();
        if let Some((id_type, id_value)) = trimmed.split_once(':') {
            let id_type = id_type.trim().to_lowercase();
            let id_value = id_value.trim().to_string();
            if id_type.is_empty() || id_value.is_empty() {
                continue;
            }
            by_type.entry(id_type).or_default().insert(id_value);
        }
    }
    let mut conflicts = Vec::new();
    for (id_type, values) in by_type {
        if values.len() > 1 {
            conflicts.push(format!("{id_type} has {} values", values.len()));
        }
    }
    if conflicts.is_empty() {
        None
    } else {
        Some(format!("Identifier conflicts: {}", conflicts.join("; ")))
    }
}

fn language_hint(value: &str) -> Option<String> {
    let invalid: Vec<String> = parse_list(value)
        .into_iter()
        .filter(|token| {
            let len = token.trim().len();
            !(len == 2 || len == 3)
        })
        .collect();
    if invalid.is_empty() {
        None
    } else {
        Some(format!(
            "Language tokens should be 2-3 chars: {}",
            invalid.join(", ")
        ))
    }
}

fn collect_edit_validation_issues(edit: &EditState) -> Vec<String> {
    let mut issues = Vec::new();
    if edit.title.trim().is_empty() {
        issues.push("title cannot be empty".to_string());
    }
    if let Some(message) = duplicate_csv_hint("authors", &edit.authors) {
        issues.push(message);
    }
    if let Some(message) = duplicate_csv_hint("tags", &edit.tags) {
        issues.push(message);
    }
    if let Some(message) = language_hint(&edit.languages) {
        issues.push(message);
    }
    if let Some(message) = identifier_conflict_hint(&edit.identifiers) {
        issues.push(message);
    }
    if !edit.uuid.trim().is_empty() && uuid::Uuid::parse_str(edit.uuid.trim()).is_err() {
        issues.push("uuid must be a valid UUID".to_string());
    }
    if !is_loose_date_or_datetime(edit.pubdate.trim()) {
        issues.push("publication date must be YYYY-MM-DD or RFC3339 datetime".to_string());
    }
    if !is_loose_date_or_datetime(edit.timestamp.trim()) {
        issues.push("timestamp must be YYYY-MM-DD or RFC3339 datetime".to_string());
    }
    if !is_loose_date_or_datetime(edit.last_modified.trim()) {
        issues.push("last_modified must be YYYY-MM-DD or RFC3339 datetime".to_string());
    }
    issues
}

fn is_loose_date_or_datetime(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return true;
    }
    if time::OffsetDateTime::parse(trimmed, &time::format_description::well_known::Rfc3339).is_ok()
    {
        return true;
    }
    if let Ok(format) = time::format_description::parse("[year]-[month]-[day]") {
        return time::Date::parse(trimmed, &format).is_ok();
    }
    false
}

fn custom_field_editor_widget(ui: &mut egui::Ui, field: &mut CustomEditField) {
    match field.datatype.as_str() {
        "bool" | "boolean" => {
            let mut value = matches!(
                field.value.trim().to_lowercase().as_str(),
                "1" | "true" | "yes" | "y"
            );
            if ui.checkbox(&mut value, "").changed() {
                field.value = if value { "true" } else { "false" }.to_string();
            }
        }
        "int" | "integer" => {
            let mut value = field.value.trim().parse::<i64>().unwrap_or_default();
            if ui.add(egui::DragValue::new(&mut value)).changed() {
                field.value = value.to_string();
            }
        }
        "float" | "double" | "number" => {
            let mut value = field.value.trim().parse::<f64>().unwrap_or_default();
            if ui
                .add(egui::DragValue::new(&mut value).speed(0.1))
                .changed()
            {
                field.value = value.to_string();
            }
        }
        "date" => {
            ui.text_edit_singleline(&mut field.value);
            let valid = is_loose_date_or_datetime(field.value.trim());
            if !valid {
                ui.colored_label(
                    egui::Color32::from_rgb(170, 60, 30),
                    "expected YYYY-MM-DD or RFC3339",
                );
            }
        }
        _ => {
            ui.text_edit_singleline(&mut field.value);
        }
    }
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

fn build_output_from_template(
    output_dir: &Path,
    template: &str,
    title: &str,
    book_id: i64,
    format: &str,
    authors: &str,
) -> PathBuf {
    let rendered = template
        .replace("{title}", &sanitize_filename(title))
        .replace("{id}", &book_id.to_string())
        .replace("{format}", &sanitize_filename(format))
        .replace("{authors}", &sanitize_filename(authors));
    output_dir.join(rendered)
}

fn resolve_export_conflict_path(path: &Path, policy: &str) -> Option<PathBuf> {
    if !path.exists() {
        return Some(path.to_path_buf());
    }
    match policy {
        "overwrite" => Some(path.to_path_buf()),
        "skip" => None,
        "rename" => {
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("file");
            let ext = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            let parent = path.parent().unwrap_or_else(|| Path::new("."));
            for idx in 1..=9999 {
                let suffix = if ext.is_empty() {
                    format!("{stem}-{idx}")
                } else {
                    format!("{stem}-{idx}.{ext}")
                };
                let candidate = parent.join(suffix);
                if !candidate.exists() {
                    return Some(candidate);
                }
            }
            None
        }
        _ => Some(path.to_path_buf()),
    }
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

fn conversion_warnings(state: &ConvertBooksDialogState) -> Vec<String> {
    let mut warnings = Vec::new();
    let format = state.output_format.to_lowercase();
    if format == "mobi" && state.embed_fonts {
        warnings.push("MOBI embedding may be ignored by readers".to_string());
    }
    if format == "pdf" && state.heuristic_unwrap_lines {
        warnings.push("PDF output ignores hard line unwrap heuristics".to_string());
    }
    if state.page_margin_left == 0.0
        && state.page_margin_right == 0.0
        && state.page_margin_top == 0.0
        && state.page_margin_bottom == 0.0
    {
        warnings.push("Zero page margins can reduce readability on small screens".to_string());
    }
    warnings
}

fn render_format_options(ui: &mut egui::Ui, format_name: &str, selected_output: &str) {
    let active = selected_output.eq_ignore_ascii_case(format_name);
    let label = if active {
        format!("{format_name} options (active)")
    } else {
        format!("{format_name} options")
    };
    ui.group(|ui| {
        ui.strong(label);
        match format_name {
            "EPUB" => ui.label("Chapter split + TOC depth controls are enabled."),
            "MOBI" => ui.label("Old/new MOBI compatibility knobs are enabled."),
            "PDF" => ui.label("Page size + header/footer controls are enabled."),
            "AZW3" => ui.label("Kindle optimization controls are enabled."),
            _ => ui.label("No options"),
        };
    });
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

fn cleanup_identifier_lines(text: &str) -> String {
    let mut seen = BTreeSet::new();
    let mut lines = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.contains(':') {
            continue;
        }
        let normalized = trimmed.to_lowercase();
        if seen.insert(normalized) {
            lines.push(trimmed.to_string());
        }
    }
    lines.join("\n")
}

fn dedupe_identifier_lines(text: &str) -> String {
    normalize_identifier_lines(text)
}

fn find_identifier_value(text: &str, id_type: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let trimmed = line.trim();
        let (left, right) = trimmed.split_once(':')?;
        if left.trim().eq_ignore_ascii_case(id_type) {
            Some(right.trim().to_string())
        } else {
            None
        }
    })
}

fn first_identifier_from_details(details: &BookDetails, id_type: &str) -> Option<String> {
    details
        .identifiers
        .iter()
        .find(|entry| entry.id_type.eq_ignore_ascii_case(id_type))
        .map(|entry| entry.value.clone())
}

fn merge_tags_into_edit(
    edit: &mut EditState,
    incoming_tags: &[String],
    merge_mode: bool,
    merge_enabled: bool,
) {
    if incoming_tags.is_empty() {
        return;
    }
    if !merge_enabled && merge_mode {
        return;
    }
    let mut tags: Vec<String> = if merge_mode {
        parse_list(&edit.tags)
    } else {
        Vec::new()
    };
    for tag in incoming_tags {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !tags
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(trimmed))
        {
            tags.push(trimmed.to_string());
        }
    }
    edit.tags = tags.join(", ");
}

fn merge_identifiers_into_edit(
    edit: &mut EditState,
    incoming_identifiers: &[(String, String)],
    merge_mode: bool,
    merge_enabled: bool,
) {
    if incoming_identifiers.is_empty() {
        return;
    }
    if !merge_enabled && merge_mode {
        return;
    }
    let mut identifiers = if merge_mode {
        parse_identifiers(&edit.identifiers, &edit.isbn)
    } else {
        Vec::new()
    };
    for (id_type, value) in incoming_identifiers {
        let normalized_type = id_type.trim();
        let normalized_value = value.trim();
        if normalized_type.is_empty() || normalized_value.is_empty() {
            continue;
        }
        if !identifiers.iter().any(|(existing_type, existing_value)| {
            existing_type.eq_ignore_ascii_case(normalized_type)
                && existing_value.eq_ignore_ascii_case(normalized_value)
        }) {
            identifiers.push((normalized_type.to_string(), normalized_value.to_string()));
        }
    }
    edit.identifiers = identifiers
        .iter()
        .map(|(id_type, value)| format!("{id_type}:{value}"))
        .collect::<Vec<_>>()
        .join("\n");
}

fn enabled_sources(config: &MetadataDownloadConfig) -> Vec<String> {
    let mut sources = Vec::new();
    for provider in &config.providers {
        match provider.as_str() {
            "openlibrary" if config.openlibrary_enabled => sources.push(provider.clone()),
            "googlebooks" if config.googlebooks_enabled => sources.push(provider.clone()),
            _ => {}
        }
    }
    sources
}

fn active_sources_for_dialog(
    config: &MetadataDownloadConfig,
    state: &MetadataDownloadDialogState,
) -> Vec<String> {
    enabled_sources(config)
        .into_iter()
        .filter(|provider| match provider.as_str() {
            "openlibrary" => state.source_openlibrary,
            "googlebooks" => state.source_google,
            "amazon" => state.source_amazon,
            "isbndb" => state.source_isbndb,
            _ => false,
        })
        .collect()
}

fn first_enabled_source(config: &MetadataDownloadConfig) -> Option<String> {
    enabled_sources(config).into_iter().next()
}

fn to_provider_config(config: &MetadataDownloadConfig) -> ProviderConfig {
    ProviderConfig {
        timeout_ms: config.timeout_ms,
        user_agent: config.user_agent.clone(),
        openlibrary_enabled: config.openlibrary_enabled,
        openlibrary_base_url: config.openlibrary_base_url.clone(),
        googlebooks_enabled: config.googlebooks_enabled,
        googlebooks_base_url: config.googlebooks_base_url.clone(),
        googlebooks_api_key: config.googlebooks_api_key.clone(),
        googlebooks_max_results: config.max_results_per_provider,
        cover_max_bytes: config.cover_max_bytes,
    }
}

fn identifier_validation_badges(ui: &mut egui::Ui, identifiers: &str) {
    let mut valid = 0usize;
    let mut invalid = 0usize;
    for line in identifiers.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.split_once(':').is_some() {
            valid += 1;
        } else {
            invalid += 1;
        }
    }
    ui.horizontal(|ui| {
        ui.colored_label(
            egui::Color32::from_rgb(40, 150, 70),
            format!("Valid: {valid}"),
        );
        ui.colored_label(
            egui::Color32::from_rgb(170, 70, 40),
            format!("Invalid: {invalid}"),
        );
    });
}

fn format_half_star_rating(rating: i64) -> String {
    let whole = rating / 2;
    let half = rating % 2 != 0;
    if half {
        format!("{whole}.5")
    } else {
        whole.to_string()
    }
}

fn edit_diff_rows(before: &EditState, after: &EditState) -> Vec<(&'static str, String, String)> {
    let mut rows = Vec::new();
    macro_rules! push_diff {
        ($name:expr, $lhs:expr, $rhs:expr) => {
            if $lhs != $rhs {
                rows.push(($name, $lhs.to_string(), $rhs.to_string()));
            }
        };
    }
    push_diff!("title", before.title, after.title);
    push_diff!("authors", before.authors, after.authors);
    push_diff!("author_sort", before.author_sort, after.author_sort);
    push_diff!("series_sort", before.series_sort, after.series_sort);
    push_diff!("tags", before.tags, after.tags);
    push_diff!("series", before.series_name, after.series_name);
    push_diff!("series_index", before.series_index, after.series_index);
    push_diff!("identifiers", before.identifiers, after.identifiers);
    push_diff!("isbn", before.isbn, after.isbn);
    push_diff!("publisher", before.publisher, after.publisher);
    push_diff!("imprint", before.imprint, after.imprint);
    push_diff!("edition", before.edition, after.edition);
    push_diff!("rights", before.rights, after.rights);
    push_diff!("languages", before.languages, after.languages);
    push_diff!("timestamp", before.timestamp, after.timestamp);
    push_diff!("pubdate", before.pubdate, after.pubdate);
    push_diff!("last_modified", before.last_modified, after.last_modified);
    push_diff!("rating", before.rating, after.rating);
    push_diff!("uuid", before.uuid, after.uuid);
    push_diff!("comment", before.comment, after.comment);
    rows
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

fn row_text(text: &str, color: Option<egui::Color32>) -> egui::RichText {
    let mut rich = egui::RichText::new(text.to_string());
    if let Some(color) = color {
        rich = rich.color(color);
    }
    rich
}

fn highlight_rich_text(text: egui::RichText, query: &str) -> egui::RichText {
    if query.trim().is_empty() {
        return text;
    }
    text.background_color(egui::Color32::from_rgba_unmultiplied(120, 120, 30, 48))
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

fn compute_library_stats(rows: &[BookRow], db: &Database) -> CoreResult<LibraryStatsSummary> {
    let mut formats = HashMap::<String, usize>::new();
    let mut format_sizes = HashMap::<String, u64>::new();
    let mut languages = HashMap::<String, usize>::new();
    let mut tags = HashMap::<String, usize>::new();
    let mut authors = HashMap::<String, usize>::new();
    let mut series = HashMap::<String, usize>::new();
    let book_formats = rows
        .iter()
        .map(|row| (row.id, row.format.clone()))
        .collect::<HashMap<_, _>>();
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
    for asset in db.list_assets()? {
        let fallback = book_formats
            .get(&asset.book_id)
            .map(|value| value.as_str())
            .unwrap_or("unknown");
        let format = asset_format(&asset, fallback).unwrap_or_else(|| "unknown".to_string());
        *format_sizes.entry(format).or_insert(0) += asset.stored_size_bytes;
    }
    Ok(LibraryStatsSummary {
        formats: sort_count_map(formats),
        format_sizes: sort_size_map(format_sizes),
        languages: sort_count_map(languages),
        tags: sort_count_map(tags),
        authors: sort_count_map(authors),
        series: sort_count_map(series),
    })
}

fn sort_count_map(map: HashMap<String, usize>) -> Vec<(String, usize)> {
    let mut items = map.into_iter().collect::<Vec<_>>();
    items.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    items
}

fn sort_size_map(map: HashMap<String, u64>) -> Vec<(String, u64)> {
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

fn first_csv_item(value: &str) -> String {
    split_csv_field(value)
        .into_iter()
        .next()
        .unwrap_or_default()
}

fn split_saved_search_name(name: &str) -> (&str, &str) {
    if let Some((folder, leaf)) = name.rsplit_once('/') {
        if !folder.trim().is_empty() && !leaf.trim().is_empty() {
            return (folder, leaf);
        }
    }
    ("Ungrouped", name)
}

fn group_hierarchical_categories(
    rows: &[CategoryCount],
    delimiter: char,
) -> Vec<(String, Vec<CategoryCount>)> {
    let mut grouped: HashMap<String, Vec<CategoryCount>> = HashMap::new();
    for row in rows {
        let root = row
            .name
            .split(delimiter)
            .next()
            .unwrap_or(row.name.as_str())
            .to_string();
        grouped.entry(root).or_default().push(row.clone());
    }
    let mut entries = grouped.into_iter().collect::<Vec<_>>();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, values) in &mut entries {
        values.sort_by(|a, b| a.name.cmp(&b.name));
    }
    entries
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
    append_size_stat_lines(&mut lines, "format_size_bytes", &stats.format_sizes);
    fs::write(&output, lines.join("\n"))
        .map_err(|err| CoreError::Io("write stats csv".to_string(), err))?;
    Ok(output)
}

fn append_size_stat_lines(lines: &mut Vec<String>, kind: &str, items: &[(String, u64)]) {
    for (name, bytes) in items {
        lines.push(format!("{kind},{},{}", escape_csv_cell(name), bytes));
    }
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

fn format_bytes(bytes: u64) -> String {
    let mib = bytes as f64 / (1024.0 * 1024.0);
    format!("{mib:.1} MiB")
}

fn parse_rating_value(rating: &str) -> i64 {
    rating.trim().parse::<i64>().unwrap_or_default()
}

fn parse_hex_color(value: &str) -> Option<egui::Color32> {
    let raw = value.trim().trim_start_matches('#');
    if raw.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&raw[0..2], 16).ok()?;
    let g = u8::from_str_radix(&raw[2..4], 16).ok()?;
    let b = u8::from_str_radix(&raw[4..6], 16).ok()?;
    Some(egui::Color32::from_rgb(r, g, b))
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

fn encode_sort_presets(
    source: &HashMap<String, SortPreset>,
) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    for (name, preset) in source {
        let secondary = preset
            .secondary
            .map(|mode| mode.key().to_string())
            .unwrap_or_else(|| "none".to_string());
        let direction = match preset.direction {
            SortDirection::Asc => "asc",
            SortDirection::Desc => "desc",
        };
        let encoded = format!("{}|{}|{}", preset.primary.key(), secondary, direction);
        out.insert(name.clone(), encoded);
    }
    out
}

fn decode_sort_presets(
    source: &std::collections::BTreeMap<String, String>,
) -> HashMap<String, SortPreset> {
    let mut out = HashMap::new();
    for (name, value) in source {
        let mut parts = value.splitn(3, '|');
        let primary = parts.next().and_then(parse_sort_mode);
        let secondary = parts.next().and_then(|raw| {
            if raw == "none" {
                Some(None)
            } else {
                parse_sort_mode(raw).map(Some)
            }
        });
        let direction = parts.next().and_then(parse_sort_direction);
        if let (Some(primary), Some(secondary), Some(direction)) = (primary, secondary, direction) {
            out.insert(
                name.clone(),
                SortPreset {
                    primary,
                    secondary,
                    direction,
                },
            );
        }
    }
    out
}

fn parse_sort_direction(value: &str) -> Option<SortDirection> {
    match value {
        "asc" => Some(SortDirection::Asc),
        "desc" => Some(SortDirection::Desc),
        _ => None,
    }
}

fn decode_column_order(values: &[String]) -> Vec<ColumnKey> {
    let mut out = values
        .iter()
        .filter_map(|value| parse_column_key(value))
        .collect::<Vec<_>>();
    for default in default_column_order() {
        if !out.contains(&default) {
            out.push(default);
        }
    }
    out
}

fn encode_column_preset(preset: &ColumnPreset) -> Vec<String> {
    let mut entries = Vec::new();
    let order = preset
        .order
        .iter()
        .map(|key| key.key().to_string())
        .collect::<Vec<_>>()
        .join(",");
    entries.push(format!("order={order}"));
    let visibility = vec![
        ("title", preset.visibility.title),
        ("cover", preset.visibility.cover),
        ("authors", preset.visibility.authors),
        ("series", preset.visibility.series),
        ("tags", preset.visibility.tags),
        ("formats", preset.visibility.formats),
        ("rating", preset.visibility.rating),
        ("publisher", preset.visibility.publisher),
        ("languages", preset.visibility.languages),
        ("date_added", preset.visibility.date_added),
        ("date_modified", preset.visibility.date_modified),
        ("pubdate", preset.visibility.pubdate),
    ];
    entries.push(format!(
        "visible={}",
        visibility
            .into_iter()
            .filter(|(_, value)| *value)
            .map(|(key, _)| key.to_string())
            .collect::<Vec<_>>()
            .join(",")
    ));
    let widths = vec![
        ("title", preset.widths.title),
        ("cover", preset.widths.cover),
        ("authors", preset.widths.authors),
        ("series", preset.widths.series),
        ("tags", preset.widths.tags),
        ("formats", preset.widths.formats),
        ("rating", preset.widths.rating),
        ("publisher", preset.widths.publisher),
        ("languages", preset.widths.languages),
        ("date_added", preset.widths.date_added),
        ("date_modified", preset.widths.date_modified),
        ("pubdate", preset.widths.pubdate),
    ];
    entries.push(format!(
        "widths={}",
        widths
            .into_iter()
            .map(|(key, value)| format!("{key}:{value:.1}"))
            .collect::<Vec<_>>()
            .join(";")
    ));
    entries
}

fn decode_column_preset(entries: &[String]) -> ColumnPreset {
    let mut order = default_column_order();
    let mut visibility = ColumnVisibility {
        title: true,
        authors: true,
        series: true,
        tags: true,
        formats: true,
        rating: true,
        publisher: true,
        languages: true,
        cover: true,
        date_added: true,
        date_modified: true,
        pubdate: true,
    };
    let mut widths = ColumnWidths {
        title: 240.0,
        authors: 180.0,
        series: 140.0,
        tags: 180.0,
        formats: 120.0,
        rating: 90.0,
        publisher: 160.0,
        languages: 120.0,
        cover: 72.0,
        date_added: 140.0,
        date_modified: 140.0,
        pubdate: 140.0,
    };
    for entry in entries {
        if let Some(raw) = entry.strip_prefix("order=") {
            let parsed = raw
                .split(',')
                .filter_map(parse_column_key)
                .collect::<Vec<_>>();
            if !parsed.is_empty() {
                order = parsed;
            }
            continue;
        }
        if let Some(raw) = entry.strip_prefix("visible=") {
            let set = raw
                .split(',')
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .collect::<BTreeSet<_>>();
            visibility.title = set.contains("title");
            visibility.cover = set.contains("cover");
            visibility.authors = set.contains("authors");
            visibility.series = set.contains("series");
            visibility.tags = set.contains("tags");
            visibility.formats = set.contains("formats");
            visibility.rating = set.contains("rating");
            visibility.publisher = set.contains("publisher");
            visibility.languages = set.contains("languages");
            visibility.date_added = set.contains("date_added");
            visibility.date_modified = set.contains("date_modified");
            visibility.pubdate = set.contains("pubdate");
            continue;
        }
        if let Some(raw) = entry.strip_prefix("widths=") {
            for pair in raw.split(';') {
                let Some((key, value)) = pair.split_once(':') else {
                    continue;
                };
                let Ok(width) = value.parse::<f32>() else {
                    continue;
                };
                match key {
                    "title" => widths.title = width,
                    "cover" => widths.cover = width,
                    "authors" => widths.authors = width,
                    "series" => widths.series = width,
                    "tags" => widths.tags = width,
                    "formats" => widths.formats = width,
                    "rating" => widths.rating = width,
                    "publisher" => widths.publisher = width,
                    "languages" => widths.languages = width,
                    "date_added" => widths.date_added = width,
                    "date_modified" => widths.date_modified = width,
                    "pubdate" => widths.pubdate = width,
                    _ => {}
                }
            }
        }
    }
    ColumnPreset {
        order,
        visibility,
        widths,
    }
}

fn encode_column_presets(
    source: &HashMap<String, ColumnPreset>,
) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut out = std::collections::BTreeMap::new();
    for (name, preset) in source {
        out.insert(name.clone(), encode_column_preset(preset));
    }
    out
}

fn decode_column_presets(
    source: &std::collections::BTreeMap<String, Vec<String>>,
) -> HashMap<String, ColumnPreset> {
    let mut out = HashMap::new();
    for (name, entries) in source {
        out.insert(name.clone(), decode_column_preset(entries));
    }
    out
}

fn parse_view_mode(value: &str) -> ViewMode {
    match value {
        "grid" => ViewMode::Grid,
        "shelf" => ViewMode::Shelf,
        _ => ViewMode::Table,
    }
}

fn parse_view_density(value: &str) -> ViewDensity {
    match value {
        "compact" => ViewDensity::Compact,
        _ => ViewDensity::Comfortable,
    }
}

fn parse_group_mode(value: &str) -> GroupMode {
    match value {
        "series" => GroupMode::Series,
        "authors" => GroupMode::Authors,
        "tags" => GroupMode::Tags,
        _ => GroupMode::None,
    }
}

fn format_date_cell(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "-".to_string();
    }
    let date = trimmed.split('T').next().unwrap_or(trimmed);
    date.to_string()
}

fn column_width_control(ui: &mut egui::Ui, label: &str, value: &mut f32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        if ui
            .add(egui::DragValue::new(value).range(60.0..=720.0).speed(1.0))
            .changed()
        {
            changed = true;
        }
    });
    changed
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
        SortMode::DateAdded => a.date_added.cmp(&b.date_added),
        SortMode::DateModified => a.date_modified.cmp(&b.date_modified),
        SortMode::PubDate => a.pubdate.cmp(&b.pubdate),
        SortMode::Id => a.id.cmp(&b.id),
    }
}

fn compare_group(mode: GroupMode, a: &BookRow, b: &BookRow) -> std::cmp::Ordering {
    match mode {
        GroupMode::None => std::cmp::Ordering::Equal,
        GroupMode::Series => a.series.cmp(&b.series),
        GroupMode::Authors => first_csv_item(&a.authors).cmp(&first_csv_item(&b.authors)),
        GroupMode::Tags => first_csv_item(&a.tags).cmp(&first_csv_item(&b.tags)),
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
    render_text_with_highlight_and_style(
        ui,
        text,
        query,
        size,
        ReaderFontFamily::Sans,
        false,
        false,
    );
}

fn render_text_with_highlight_and_style(
    ui: &mut egui::Ui,
    text: &str,
    query: &str,
    size: f32,
    family: ReaderFontFamily,
    justify: bool,
    hyphenation: bool,
) {
    let font_id = match family {
        ReaderFontFamily::Sans => egui::FontId::proportional(size),
        ReaderFontFamily::Serif => egui::FontId::new(size, egui::FontFamily::Name("serif".into())),
        ReaderFontFamily::Monospace => egui::FontId::monospace(size),
    };
    let mut rendered = text.to_string();
    if hyphenation {
        rendered = rendered
            .split_whitespace()
            .map(|token| {
                if token.len() > 14 {
                    let split = token.len() / 2;
                    format!("{}-{}", &token[..split], &token[split..])
                } else {
                    token.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
    }
    let mut job = egui::text::LayoutJob::default();
    if query.trim().is_empty() {
        job.append(
            &rendered,
            0.0,
            egui::TextFormat {
                font_id,
                ..Default::default()
            },
        );
        if justify {
            job.justify = true;
        }
        ui.label(job);
        return;
    }
    let query_lower = query.to_lowercase();
    let mut remaining = rendered.as_str();
    while let Some(pos) = remaining.to_lowercase().find(&query_lower) {
        let (prefix, rest) = remaining.split_at(pos);
        if !prefix.is_empty() {
            job.append(
                prefix,
                0.0,
                egui::TextFormat {
                    font_id: font_id.clone(),
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
                    font_id: font_id.clone(),
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
                font_id,
                ..Default::default()
            },
        );
    }
    if justify {
        job.justify = true;
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

fn ui_collapsing_troubleshooting(ui: &mut egui::Ui, device: &DeviceInfo) {
    ui.collapsing("Connection troubleshooting", |ui| {
        let mount_exists = device.mount_path.exists();
        let library_exists = device.library_path.exists();
        let mount_writable = fs::metadata(&device.mount_path)
            .map(|meta| !meta.permissions().readonly())
            .unwrap_or(false);
        let library_writable = fs::metadata(&device.library_path)
            .map(|meta| !meta.permissions().readonly())
            .unwrap_or(false);
        ui.label(format!(
            "mount path exists: {}",
            if mount_exists { "yes" } else { "no" }
        ));
        ui.label(format!(
            "library path exists: {}",
            if library_exists { "yes" } else { "no" }
        ));
        ui.label(format!(
            "mount writable: {}",
            if mount_writable { "yes" } else { "no" }
        ));
        ui.label(format!(
            "library writable: {}",
            if library_writable { "yes" } else { "no" }
        ));
    });
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

fn open_url(url: &str) -> CoreResult<()> {
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|err| CoreError::Io("open url".to_string(), err))?;
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        tracing::warn!(component = "gui", url = %url, "open url not supported");
        Err(CoreError::ConfigValidate(
            "open url not supported on this platform".to_string(),
        ))
    }
}
