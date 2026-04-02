use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::path::PathBuf;

use caliberate_assets::stats::{
    AssetDescriptor, apply_compaction, compute_storage_stats, plan_compaction, verify_assets,
};
use caliberate_assets::storage::{AssetStore, LocalAssetStore, StorageMode};
use caliberate_core::config::IngestMode;
use caliberate_db::database::Database;
use caliberate_device::detection::{DeviceInfo, detect_devices};
use caliberate_device::sync::{cleanup_device_orphans, list_device_entries, send_to_device};
use caliberate_library::ingest::{IngestOutcome, IngestRequest, Ingestor};
use caliberate_metadata::extract::{extract_archive_entry, extract_basic};

#[derive(Debug, Parser)]
#[command(name = "calibredb", version, about = "Caliberate database CLI")]
struct CalibredbCli {
    #[arg(long, default_value = "config/control-plane.toml")]
    config: PathBuf,
    #[command(subcommand)]
    command: Option<CalibredbCommand>,
}

#[derive(Debug, Subcommand)]
enum CalibredbCommand {
    CheckConfig,
    Init,
    Add {
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        mode: Option<IngestModeValue>,
    },
    Show {
        #[arg(long)]
        id: i64,
    },
    ShowMetadata {
        #[arg(long)]
        id: i64,
        #[arg(long, default_value_t = false)]
        as_opf: bool,
    },
    Remove {
        #[arg(long)]
        id: i64,
        #[arg(long, default_value_t = false)]
        delete_files: bool,
        #[arg(long, default_value_t = false)]
        delete_reference_files: bool,
    },
    ExtractArchive {
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        entry: String,
        #[arg(long)]
        output_dir: Option<PathBuf>,
    },
    List {
        #[arg(long)]
        fields: Vec<String>,
        #[arg(long)]
        sort_by: Option<String>,
        #[arg(long, default_value_t = false)]
        ascending: bool,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long, default_value_t = false)]
        for_machine: bool,
    },
    Search {
        #[arg(long)]
        query: String,
        #[arg(long)]
        fields: Vec<String>,
        #[arg(long)]
        sort_by: Option<String>,
        #[arg(long, default_value_t = false)]
        ascending: bool,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long, default_value_t = false)]
        for_machine: bool,
    },
    Assets {
        #[command(subcommand)]
        command: AssetsCommand,
    },
    Fts {
        #[command(subcommand)]
        command: FtsCommand,
    },
    ListCategories {
        #[arg(long)]
        category: Option<CategoryValue>,
    },
    SavedSearches {
        #[command(subcommand)]
        command: SavedSearchesCommand,
    },
    CustomColumns {
        #[command(subcommand)]
        command: CustomColumnsCommand,
    },
    SetCustom {
        #[arg(long)]
        id: i64,
        #[arg(long)]
        label: String,
        #[arg(long)]
        value: String,
    },
    SetMetadata {
        #[arg(long)]
        id: i64,
        #[arg(long, default_value_t = false)]
        list_fields: bool,
        #[arg(long)]
        field: Vec<String>,
    },
    RestoreDatabase {
        #[arg(long)]
        input_dir: PathBuf,
    },
    Clone {
        #[arg(long)]
        output_dir: PathBuf,
        #[arg(long, default_value_t = false)]
        include_references: bool,
    },
    EmbedMetadata {
        #[arg(long)]
        id: Vec<i64>,
        #[arg(long, default_value_t = false)]
        all: bool,
    },
    Formats {
        #[command(subcommand)]
        command: FormatsCommand,
    },
    Notes {
        #[command(subcommand)]
        command: NotesCommand,
    },
    Set {
        #[command(subcommand)]
        command: SetCommand,
    },
    CheckLibrary,
    Export {
        #[arg(long)]
        id: Vec<i64>,
        #[arg(long, default_value_t = false)]
        all: bool,
        #[arg(long)]
        output_dir: PathBuf,
    },
    BackupMetadata {
        #[arg(long)]
        id: Vec<i64>,
        #[arg(long, default_value_t = false)]
        all: bool,
        #[arg(long)]
        output_dir: PathBuf,
    },
    Catalog {
        #[arg(long)]
        id: Vec<i64>,
        #[arg(long, default_value_t = false)]
        all: bool,
        #[arg(long)]
        output: PathBuf,
    },
    Device {
        #[command(subcommand)]
        command: DeviceCommand,
    },
    Info,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum IngestModeValue {
    Copy,
    Reference,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CategoryValue {
    Authors,
    Tags,
    Series,
    Publishers,
    Ratings,
    Languages,
}

#[derive(Debug, Subcommand)]
enum AssetsCommand {
    List,
    Stats,
    Verify,
    Compact {
        #[arg(long, default_value_t = false)]
        apply: bool,
    },
}

#[derive(Debug, Subcommand)]
enum FtsCommand {
    Status,
    Rebuild,
    Search {
        #[arg(long)]
        query: String,
    },
    Enable {
        #[arg(long, default_value_t = false)]
        rebuild: bool,
    },
    Disable,
}

#[derive(Debug, Subcommand)]
enum FormatsCommand {
    List {
        #[arg(long)]
        id: i64,
    },
    Add {
        #[arg(long)]
        id: i64,
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        mode: Option<IngestModeValue>,
        #[arg(long)]
        format: Option<String>,
    },
    Remove {
        #[arg(long)]
        id: i64,
        #[arg(long)]
        format: Option<String>,
        #[arg(long)]
        asset_id: Option<i64>,
        #[arg(long, default_value_t = false)]
        delete_files: bool,
        #[arg(long, default_value_t = false)]
        delete_reference_files: bool,
    },
}

#[derive(Debug, Subcommand)]
enum SavedSearchesCommand {
    List,
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        query: String,
    },
    Remove {
        #[arg(long)]
        name: String,
    },
}

#[derive(Debug, Subcommand)]
enum CustomColumnsCommand {
    List,
    Add {
        #[arg(long)]
        label: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        datatype: String,
    },
    Remove {
        #[arg(long)]
        label: String,
    },
}
#[derive(Debug, Subcommand)]
enum NotesCommand {
    Add {
        #[arg(long)]
        book_id: i64,
        #[arg(long)]
        text: String,
    },
    List {
        #[arg(long)]
        book_id: i64,
    },
    Delete {
        #[arg(long)]
        note_id: i64,
    },
}

#[derive(Debug, Subcommand)]
enum SetCommand {
    Title {
        #[arg(long)]
        id: i64,
        #[arg(long)]
        title: String,
    },
    Authors {
        #[arg(long)]
        id: i64,
        #[arg(long)]
        value: Vec<String>,
    },
    Tags {
        #[arg(long)]
        id: i64,
        #[arg(long)]
        value: Vec<String>,
    },
    Series {
        #[arg(long)]
        id: i64,
        #[arg(long)]
        name: String,
        #[arg(long, default_value_t = 1.0)]
        index: f64,
    },
    Identifiers {
        #[arg(long)]
        id: i64,
        #[arg(long)]
        identifier: Vec<String>,
    },
    Comment {
        #[arg(long)]
        id: i64,
        #[arg(long)]
        text: String,
    },
    Publisher {
        #[arg(long)]
        id: i64,
        #[arg(long)]
        name: String,
    },
    Rating {
        #[arg(long)]
        id: i64,
        #[arg(long)]
        rating: i64,
    },
    Languages {
        #[arg(long)]
        id: i64,
        #[arg(long)]
        value: Vec<String>,
    },
    Dates {
        #[arg(long)]
        id: i64,
        #[arg(long)]
        timestamp: Option<String>,
        #[arg(long)]
        pubdate: Option<String>,
        #[arg(long)]
        last_modified: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum DeviceCommand {
    List,
    ListFiles {
        #[arg(long)]
        device: Option<String>,
    },
    Send {
        #[arg(long)]
        device: Option<String>,
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        dest_name: Option<String>,
    },
    Cleanup {
        #[arg(long)]
        device: Option<String>,
        #[arg(long)]
        keep: Vec<String>,
    },
}

impl From<IngestModeValue> for IngestMode {
    fn from(value: IngestModeValue) -> Self {
        match value {
            IngestModeValue::Copy => IngestMode::Copy,
            IngestModeValue::Reference => IngestMode::Reference,
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = CalibredbCli::parse();
    let bootstrap = caliberate_app::bootstrap::init(&cli.config)?;
    let mut config = bootstrap.config;

    match cli.command {
        Some(CalibredbCommand::CheckConfig) => {
            tracing::info!(component = "calibredb", "configuration check passed");
        }
        Some(CalibredbCommand::Init) => {
            let _db = Database::open_with_fts(&config.db, &config.fts)?;
            println!(
                "Database initialized at {}",
                config.db.sqlite_path.display()
            );
        }
        Some(CalibredbCommand::Add { path, mode }) => {
            let store = LocalAssetStore::from_config(&config);
            let ingestor = Ingestor::new(std::sync::Arc::new(store), config.clone());
            let request = IngestRequest {
                source_path: &path,
                mode: mode.map(Into::into),
            };
            let outcome = ingestor.ingest(request)?;
            let result = match outcome {
                IngestOutcome::Ingested(result) => result,
                IngestOutcome::Skipped(skip) => {
                    println!(
                        "Skipped ingest; duplicate {:?} at {}",
                        skip.reason,
                        skip.existing_path.display()
                    );
                    return Ok(());
                }
            };

            let mut db = Database::open_with_fts(&config.db, &config.fts)?;
            let created_at = time::OffsetDateTime::now_utc().format(
                &time::format_description::parse("[year]-[month]-[day]T[hour]:[minute]:[second]Z")?,
            )?;
            let id = db.add_book(
                &result.metadata.title,
                &result.metadata.format,
                &result.asset.stored_path.display().to_string(),
                &created_at,
            )?;
            let storage_mode = match result.asset.storage_mode {
                StorageMode::Copy => "copy",
                StorageMode::Reference => "reference",
            };
            let _asset_id = db.add_asset(
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
            db.add_book_authors(id, &result.metadata.authors)?;
            db.add_book_tags(id, &result.metadata.tags)?;
            if let Some(series) = &result.metadata.series {
                db.set_book_series(id, &series.name, series.index)?;
            }
            db.add_book_identifiers(id, &result.metadata.identifiers)?;
            if let Some(comment) = &result.metadata.comment {
                db.set_book_comment(id, comment)?;
            }

            println!("Added book {id}");
        }
        Some(CalibredbCommand::Show { id }) => {
            let db = Database::open_with_fts(&config.db, &config.fts)?;
            let Some(book) = db.get_book(id)? else {
                println!("Book not found: {id}");
                return Ok(());
            };
            println!("Book {id}");
            println!("Title: {}", book.title);
            println!("Format: {}", book.format);
            println!("Path: {}", book.path);

            let authors = db.list_book_authors(id)?;
            if authors.is_empty() {
                println!("Authors: none");
            } else {
                println!("Authors: {}", authors.join(", "));
            }

            let tags = db.list_book_tags(id)?;
            if tags.is_empty() {
                println!("Tags: none");
            } else {
                println!("Tags: {}", tags.join(", "));
            }

            if let Some(series) = db.get_book_series(id)? {
                println!("Series: {} ({})", series.name, series.index);
            } else {
                println!("Series: none");
            }

            let identifiers = db.list_book_identifiers(id)?;
            if identifiers.is_empty() {
                println!("Identifiers: none");
            } else {
                println!("Identifiers:");
                for identifier in identifiers {
                    println!("  {}: {}", identifier.id_type, identifier.value);
                }
            }

            if let Some(comment) = db.get_book_comment(id)? {
                println!("Comment: {comment}");
            } else {
                println!("Comment: none");
            }

            let extras = db.get_book_extras(id)?;
            if let Some(publisher) = extras.publisher {
                println!("Publisher: {publisher}");
            } else {
                println!("Publisher: none");
            }
            if let Some(rating) = extras.rating {
                println!("Rating: {rating}");
            } else {
                println!("Rating: none");
            }
            if extras.languages.is_empty() {
                println!("Languages: none");
            } else {
                println!("Languages: {}", extras.languages.join(", "));
            }
            if let Some(uuid) = extras.uuid {
                println!("UUID: {uuid}");
            } else {
                println!("UUID: none");
            }
            println!("Has cover: {}", extras.has_cover);
            if let Some(timestamp) = extras.timestamp {
                println!("Timestamp: {timestamp}");
            } else {
                println!("Timestamp: none");
            }
            if let Some(pubdate) = extras.pubdate {
                println!("Pubdate: {pubdate}");
            } else {
                println!("Pubdate: none");
            }
            if let Some(last_modified) = extras.last_modified {
                println!("Last modified: {last_modified}");
            } else {
                println!("Last modified: none");
            }

            let assets = db.list_assets_for_book(id)?;
            if assets.is_empty() {
                println!("Assets: none");
            } else {
                println!("Assets:");
                for asset in assets {
                    println!(
                        "{}\t{}\t{}\t{}\t{}\t{}",
                        asset.id,
                        asset.storage_mode,
                        asset.stored_path,
                        asset.source_path.as_deref().unwrap_or("-"),
                        asset.size_bytes,
                        asset.stored_size_bytes
                    );
                }
            }
        }
        Some(CalibredbCommand::ShowMetadata { id, as_opf }) => {
            let db = Database::open_with_fts(&config.db, &config.fts)?;
            let output = build_metadata_output(&db, id)?;
            if as_opf {
                let opf = render_opf(&output)?;
                println!("{opf}");
            } else {
                let json = serde_json::to_string_pretty(&output)?;
                println!("{json}");
            }
        }
        Some(CalibredbCommand::Remove {
            id,
            delete_files,
            delete_reference_files,
        }) => {
            let mut db = Database::open_with_fts(&config.db, &config.fts)?;
            let assets = db.list_assets_for_book(id)?;
            let delete_files = delete_files || config.library.delete_files_on_remove;
            let delete_reference_files =
                delete_reference_files || config.library.delete_reference_files;

            if delete_files {
                delete_asset_files(&assets, delete_reference_files)?;
            }

            let summary = db.delete_book_with_assets(id)?;
            if !summary.book_deleted {
                println!("Book not found: {id}");
            } else {
                println!("Deleted book {id} and {} assets", summary.assets_deleted);
            }
        }
        Some(CalibredbCommand::ExtractArchive {
            path,
            entry,
            output_dir,
        }) => {
            let store = LocalAssetStore::from_config(&config);
            let ingestor = Ingestor::new(std::sync::Arc::new(store), config.clone());
            let extracted = if let Some(output_dir) = output_dir {
                extract_archive_entry(&path, &entry, &output_dir, &config.formats)?
            } else {
                ingestor.extract_archive_on_demand(&path, &entry)?
            };
            println!("Extracted to {}", extracted.display());
        }
        Some(CalibredbCommand::List {
            fields,
            sort_by,
            ascending,
            limit,
            for_machine,
        }) => {
            let db = Database::open_with_fts(&config.db, &config.fts)?;
            let books = db.list_books()?;
            let displays = build_displays(&db, &books)?;
            render_books(
                &displays,
                &fields,
                sort_by.as_deref(),
                ascending,
                limit,
                for_machine,
            )?;
        }
        Some(CalibredbCommand::Search {
            query,
            fields,
            sort_by,
            ascending,
            limit,
            for_machine,
        }) => {
            let db = Database::open_with_fts(&config.db, &config.fts)?;
            let books = db.search_books(&query)?;
            let displays = build_displays(&db, &books)?;
            render_books(
                &displays,
                &fields,
                sort_by.as_deref(),
                ascending,
                limit,
                for_machine,
            )?;
        }
        Some(CalibredbCommand::Assets { command }) => {
            let mut db = Database::open_with_fts(&config.db, &config.fts)?;
            let assets = db.list_assets()?;
            let descriptors = assets
                .iter()
                .map(|asset| asset_to_descriptor(asset))
                .collect::<Result<Vec<_>, _>>()?;
            match command {
                AssetsCommand::List => {
                    for asset in &assets {
                        println!(
                            "{}\t{}\t{}\t{}\t{}",
                            asset.id,
                            asset.book_id,
                            asset.storage_mode,
                            asset.stored_path,
                            asset.size_bytes
                        );
                    }
                }
                AssetsCommand::Stats => {
                    let stats = compute_storage_stats(&descriptors, &config.paths.library_dir)?;
                    println!("Total assets: {}", stats.total_assets);
                    println!("Copied assets: {}", stats.copied_assets);
                    println!("Referenced assets: {}", stats.referenced_assets);
                    println!("Compressed assets: {}", stats.compressed_assets);
                    println!("Total bytes: {}", stats.total_bytes);
                    println!("Stored bytes: {}", stats.stored_bytes);
                    println!("Library files: {}", stats.library_files);
                    println!("Library bytes: {}", stats.library_bytes);
                    println!("Orphan files: {}", stats.orphan_files);
                    println!("Orphan bytes: {}", stats.orphan_bytes);
                }
                AssetsCommand::Verify => {
                    let issues = verify_assets(&descriptors, &config.assets)?;
                    if issues.is_empty() {
                        println!("No integrity issues detected");
                    } else {
                        for issue in issues {
                            println!(
                                "{}\t{}\t{:?}\t{}",
                                issue.asset_id,
                                issue.stored_path.display(),
                                issue.kind,
                                issue.detail
                            );
                        }
                    }
                }
                AssetsCommand::Compact { apply } => {
                    let plan = plan_compaction(&descriptors, &config.paths.library_dir)?;
                    println!("Missing asset records: {}", plan.missing_asset_ids.len());
                    println!("Orphan files: {}", plan.orphan_files.len());
                    if apply {
                        let result = apply_compaction(&plan)?;
                        let deleted = db.delete_assets(&plan.missing_asset_ids)?;
                        println!("Removed orphan files: {}", result.orphan_files_removed);
                        println!("Removed orphan bytes: {}", result.orphan_bytes_removed);
                        println!(
                            "Pruned missing asset records: {}",
                            deleted.min(result.missing_assets_pruned)
                        );
                    } else {
                        println!(
                            "Dry run: pass --apply to delete orphan files and prune missing records"
                        );
                    }
                }
            }
        }
        Some(CalibredbCommand::Fts { command }) => {
            let db = Database::open_with_fts(&config.db, &config.fts)?;
            match command {
                FtsCommand::Status => {
                    println!("FTS enabled: {}", config.fts.enabled);
                    println!("FTS tokenizer: {}", config.fts.tokenizer);
                    if config.fts.enabled {
                        let count = db.fts_count()?;
                        println!("FTS indexed rows: {}", count);
                    }
                }
                FtsCommand::Rebuild => {
                    db.rebuild_fts()?;
                    println!("FTS index rebuilt");
                }
                FtsCommand::Search { query } => {
                    if !config.fts.enabled {
                        return Err("fts is disabled".into());
                    }
                    let results = db.search_books_fts(&query)?;
                    for book in results {
                        println!(
                            "{}\t{}\t{}\t{}",
                            book.id, book.title, book.format, book.path
                        );
                    }
                }
                FtsCommand::Enable { rebuild } => {
                    if !config.fts.enabled {
                        config.fts.enabled = true;
                        config.save_to_path(&cli.config)?;
                    }
                    let db = Database::open_with_fts(&config.db, &config.fts)?;
                    if rebuild {
                        db.rebuild_fts()?;
                        println!("FTS enabled and rebuilt");
                    } else {
                        println!("FTS enabled");
                    }
                }
                FtsCommand::Disable => {
                    if config.fts.enabled {
                        config.fts.enabled = false;
                        config.save_to_path(&cli.config)?;
                    }
                    println!("FTS disabled");
                }
            }
        }
        Some(CalibredbCommand::SavedSearches { command }) => {
            let db = Database::open_with_fts(&config.db, &config.fts)?;
            match command {
                SavedSearchesCommand::List => {
                    let searches = db.list_saved_searches()?;
                    if searches.is_empty() {
                        println!("No saved searches");
                    } else {
                        for (name, query) in searches {
                            println!("{name}\t{query}");
                        }
                    }
                }
                SavedSearchesCommand::Add { name, query } => {
                    db.add_saved_search(&name, &query)?;
                    println!("Saved search added: {name}");
                }
                SavedSearchesCommand::Remove { name } => {
                    if db.remove_saved_search(&name)? {
                        println!("Saved search removed: {name}");
                    } else {
                        println!("Saved search not found: {name}");
                    }
                }
            }
        }
        Some(CalibredbCommand::CustomColumns { command }) => {
            let db = Database::open_with_fts(&config.db, &config.fts)?;
            match command {
                CustomColumnsCommand::List => {
                    let columns = db.list_custom_columns()?;
                    if columns.is_empty() {
                        println!("No custom columns");
                    } else {
                        for column in columns {
                            println!(
                                "{}\t{}\t{}\t{}",
                                column.label, column.name, column.datatype, column.id
                            );
                        }
                    }
                }
                CustomColumnsCommand::Add {
                    label,
                    name,
                    datatype,
                } => {
                    let display = "{}";
                    let id = db.create_custom_column(&label, &name, &datatype, display)?;
                    println!("Added custom column {label} ({id})");
                }
                CustomColumnsCommand::Remove { label } => {
                    if db.delete_custom_column(&label)? {
                        println!("Removed custom column {label}");
                    } else {
                        println!("Custom column not found: {label}");
                    }
                }
            }
        }
        Some(CalibredbCommand::SetCustom { id, label, value }) => {
            let db = Database::open_with_fts(&config.db, &config.fts)?;
            if db.get_book(id)?.is_none() {
                return Err(format!("book not found: {id}").into());
            }
            db.set_custom_value(id, &label, &value)?;
            println!("Updated custom column {label} for book {id}");
        }
        Some(CalibredbCommand::SetMetadata {
            id,
            list_fields,
            field,
        }) => {
            if list_fields {
                print_metadata_fields();
                return Ok(());
            }
            if field.is_empty() {
                return Err("set-metadata requires --field or --list-fields".into());
            }
            let mut db = Database::open_with_fts(&config.db, &config.fts)?;
            if db.get_book(id)?.is_none() {
                return Err(format!("book not found: {id}").into());
            }
            apply_metadata_fields(&mut db, id, &field)?;
            println!("Updated metadata for book {id}");
        }
        Some(CalibredbCommand::RestoreDatabase { input_dir }) => {
            let mut db = Database::open_with_fts(&config.db, &config.fts)?;
            let restored = restore_database(&mut db, &input_dir)?;
            println!("Restored metadata for {restored} books");
        }
        Some(CalibredbCommand::Clone {
            output_dir,
            include_references,
        }) => {
            clone_library(&config, &output_dir, include_references)?;
            println!("Cloned library to {}", output_dir.display());
        }
        Some(CalibredbCommand::EmbedMetadata { id, all }) => {
            let db = Database::open_with_fts(&config.db, &config.fts)?;
            let ids = resolve_book_ids(&db, &id, all)?;
            let written = embed_metadata(&db, &ids)?;
            println!("Wrote metadata for {written} books");
        }
        Some(CalibredbCommand::ListCategories { category }) => {
            let db = Database::open_with_fts(&config.db, &config.fts)?;
            if let Some(category) = category {
                print_category(&db, category)?;
            } else {
                for category in [
                    CategoryValue::Authors,
                    CategoryValue::Tags,
                    CategoryValue::Series,
                    CategoryValue::Publishers,
                    CategoryValue::Ratings,
                    CategoryValue::Languages,
                ] {
                    print_category(&db, category)?;
                }
            }
        }
        Some(CalibredbCommand::Formats { command }) => match command {
            FormatsCommand::List { id } => {
                let db = Database::open_with_fts(&config.db, &config.fts)?;
                let assets = db.list_assets_for_book(id)?;
                if assets.is_empty() {
                    println!("No formats found for book {id}");
                } else {
                    for asset in assets {
                        let format = format_from_path(&asset.stored_path)
                            .unwrap_or_else(|| "unknown".to_string());
                        println!("{}\t{}\t{}", format, asset.id, asset.stored_path);
                    }
                }
            }
            FormatsCommand::Add {
                id,
                path,
                mode,
                format,
            } => {
                let store = LocalAssetStore::from_config(&config);
                let storage_mode = match mode.map(Into::into) {
                    Some(IngestMode::Reference) => StorageMode::Reference,
                    _ => StorageMode::Copy,
                };
                let format = match format {
                    Some(format) => format,
                    None => extract_basic(&path, &config.formats)?.format,
                };
                let asset_outcome = store.store(&path, storage_mode)?;
                let asset = match asset_outcome {
                    caliberate_assets::storage::StoreOutcome::Stored(asset) => asset,
                    caliberate_assets::storage::StoreOutcome::Skipped(skip) => {
                        println!(
                            "Skipped add-format; duplicate {:?} at {}",
                            skip.reason,
                            skip.existing_path.display()
                        );
                        return Ok(());
                    }
                };
                let db = Database::open_with_fts(&config.db, &config.fts)?;
                let created_at =
                    time::OffsetDateTime::now_utc().format(&time::format_description::parse(
                        "[year]-[month]-[day]T[hour]:[minute]:[second]Z",
                    )?)?;
                let storage_mode_label = match asset.storage_mode {
                    StorageMode::Copy => "copy",
                    StorageMode::Reference => "reference",
                };
                let _asset_id = db.add_asset(
                    id,
                    storage_mode_label,
                    &asset.stored_path.display().to_string(),
                    asset
                        .source_path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .as_deref(),
                    asset.size_bytes,
                    asset.stored_size_bytes,
                    asset.checksum.as_deref(),
                    asset.is_compressed,
                    &created_at,
                )?;
                println!("Added format {format} for book {id}");
            }
            FormatsCommand::Remove {
                id,
                format,
                asset_id,
                delete_files,
                delete_reference_files,
            } => {
                if format.is_none() && asset_id.is_none() {
                    return Err("remove-format requires --format or --asset-id".into());
                }
                let mut db = Database::open_with_fts(&config.db, &config.fts)?;
                let assets = db.list_assets_for_book(id)?;
                let delete_files = delete_files || config.library.delete_files_on_remove;
                let delete_reference_files =
                    delete_reference_files || config.library.delete_reference_files;
                let mut targets = Vec::new();
                for asset in &assets {
                    if let Some(asset_id) = asset_id {
                        if asset.id == asset_id {
                            targets.push(asset.clone());
                        }
                        continue;
                    }
                    if let Some(format) = &format {
                        if format_from_path(&asset.stored_path)
                            .map(|value| value == *format)
                            .unwrap_or(false)
                        {
                            targets.push(asset.clone());
                        }
                    }
                }
                if targets.is_empty() {
                    println!("No matching formats found for book {id}");
                    return Ok(());
                }
                if delete_files {
                    delete_asset_files(&targets, delete_reference_files)?;
                }
                let ids = targets.iter().map(|asset| asset.id).collect::<Vec<_>>();
                let deleted = db.delete_assets(&ids)?;
                println!("Removed {deleted} formats for book {id}");
            }
        },
        Some(CalibredbCommand::Notes { command }) => match command {
            NotesCommand::Add { book_id, text } => {
                let mut db = Database::open_with_fts(&config.db, &config.fts)?;
                let created_at =
                    time::OffsetDateTime::now_utc().format(&time::format_description::parse(
                        "[year]-[month]-[day]T[hour]:[minute]:[second]Z",
                    )?)?;
                let note_id = db.add_note(book_id, &text, &created_at)?;
                println!("Added note {note_id} for book {book_id}");
            }
            NotesCommand::List { book_id } => {
                let db = Database::open_with_fts(&config.db, &config.fts)?;
                let notes = db.list_notes_for_book(book_id)?;
                if notes.is_empty() {
                    println!("No notes for book {book_id}");
                } else {
                    for note in notes {
                        println!("{}\t{}\t{}", note.id, note.created_at, note.text);
                    }
                }
            }
            NotesCommand::Delete { note_id } => {
                let mut db = Database::open_with_fts(&config.db, &config.fts)?;
                if db.delete_note(note_id)? {
                    println!("Deleted note {note_id}");
                } else {
                    println!("Note not found: {note_id}");
                }
            }
        },
        Some(CalibredbCommand::Set { command }) => match command {
            SetCommand::Title { id, title } => {
                let mut db = Database::open_with_fts(&config.db, &config.fts)?;
                if db.update_book_title(id, &title)? {
                    println!("Updated title for book {id}");
                } else {
                    println!("Book not found: {id}");
                }
            }
            SetCommand::Authors { id, value } => {
                let mut db = Database::open_with_fts(&config.db, &config.fts)?;
                db.replace_book_authors(id, &value)?;
                println!("Updated authors for book {id}");
            }
            SetCommand::Tags { id, value } => {
                let mut db = Database::open_with_fts(&config.db, &config.fts)?;
                db.replace_book_tags(id, &value)?;
                println!("Updated tags for book {id}");
            }
            SetCommand::Series { id, name, index } => {
                let mut db = Database::open_with_fts(&config.db, &config.fts)?;
                db.set_book_series(id, &name, index)?;
                println!("Updated series for book {id}");
            }
            SetCommand::Identifiers { id, identifier } => {
                let mut db = Database::open_with_fts(&config.db, &config.fts)?;
                let parsed = parse_identifiers(&identifier)?;
                db.replace_book_identifiers(id, &parsed)?;
                println!("Updated identifiers for book {id}");
            }
            SetCommand::Comment { id, text } => {
                let db = Database::open_with_fts(&config.db, &config.fts)?;
                db.set_book_comment(id, &text)?;
                println!("Updated comment for book {id}");
            }
            SetCommand::Publisher { id, name } => {
                let mut db = Database::open_with_fts(&config.db, &config.fts)?;
                db.set_book_publisher(id, &name)?;
                println!("Updated publisher for book {id}");
            }
            SetCommand::Rating { id, rating } => {
                let mut db = Database::open_with_fts(&config.db, &config.fts)?;
                db.set_book_rating(id, rating)?;
                println!("Updated rating for book {id}");
            }
            SetCommand::Languages { id, value } => {
                let mut db = Database::open_with_fts(&config.db, &config.fts)?;
                db.set_book_languages(id, &value)?;
                println!("Updated languages for book {id}");
            }
            SetCommand::Dates {
                id,
                timestamp,
                pubdate,
                last_modified,
            } => {
                if timestamp.is_none() && pubdate.is_none() && last_modified.is_none() {
                    return Err("set dates requires at least one value".into());
                }
                let mut db = Database::open_with_fts(&config.db, &config.fts)?;
                if let Some(value) = timestamp {
                    db.update_book_timestamp(id, &value)?;
                }
                if let Some(value) = pubdate {
                    db.update_book_pubdate(id, &value)?;
                }
                if let Some(value) = last_modified {
                    db.update_book_last_modified(id, &value)?;
                }
                println!("Updated dates for book {id}");
            }
        },
        Some(CalibredbCommand::Device { command }) => match command {
            DeviceCommand::List => {
                let devices = detect_devices(&config.device)?;
                if devices.is_empty() {
                    println!("No devices detected");
                } else {
                    for device in devices {
                        println!(
                            "{}\t{}\t{}",
                            device.name,
                            device.mount_path.display(),
                            device.library_path.display()
                        );
                    }
                }
            }
            DeviceCommand::ListFiles { device } => {
                let device = resolve_device(&config.device, device.as_deref())?;
                let entries = list_device_entries(&device)?;
                if entries.is_empty() {
                    println!("No files found on device {}", device.name);
                } else {
                    for entry in entries {
                        println!("{}", entry.display());
                    }
                }
            }
            DeviceCommand::Send {
                device,
                path,
                dest_name,
            } => {
                let device = resolve_device(&config.device, device.as_deref())?;
                let result = send_to_device(&path, &device, dest_name.as_deref())?;
                println!(
                    "Sent {} to {} ({} bytes)",
                    result.source.display(),
                    result.destination.display(),
                    result.bytes_copied
                );
            }
            DeviceCommand::Cleanup { device, keep } => {
                let device = resolve_device(&config.device, device.as_deref())?;
                let removed = cleanup_device_orphans(&device, &keep)?;
                println!("Removed {removed} files from device {}", device.name);
            }
        },
        Some(CalibredbCommand::CheckLibrary) => {
            let db = Database::open_with_fts(&config.db, &config.fts)?;
            let assets = db.list_assets()?;
            let descriptors = assets
                .iter()
                .map(|asset| asset_to_descriptor(asset))
                .collect::<Result<Vec<_>, _>>()?;
            let issues = verify_assets(&descriptors, &config.assets)?;
            if issues.is_empty() {
                println!("Library check OK");
            } else {
                println!("Library check found {} issues", issues.len());
                for issue in issues {
                    println!(
                        "{}\t{}\t{}",
                        issue.asset_id,
                        issue.stored_path.display(),
                        issue.detail
                    );
                }
                return Err("library check failed".into());
            }
        }
        Some(CalibredbCommand::Export {
            id,
            all,
            output_dir,
        }) => {
            let db = Database::open_with_fts(&config.db, &config.fts)?;
            let ids = resolve_book_ids(&db, &id, all)?;
            export_books(&db, &ids, &output_dir)?;
            println!("Exported {} books to {}", ids.len(), output_dir.display());
        }
        Some(CalibredbCommand::BackupMetadata {
            id,
            all,
            output_dir,
        }) => {
            let db = Database::open_with_fts(&config.db, &config.fts)?;
            let ids = resolve_book_ids(&db, &id, all)?;
            backup_metadata(&db, &ids, &output_dir)?;
            println!(
                "Wrote metadata for {} books to {}",
                ids.len(),
                output_dir.display()
            );
        }
        Some(CalibredbCommand::Catalog { id, all, output }) => {
            let db = Database::open_with_fts(&config.db, &config.fts)?;
            let ids = resolve_book_ids(&db, &id, all)?;
            write_catalog(&db, &ids, &output)?;
            println!(
                "Wrote catalog for {} books to {}",
                ids.len(),
                output.display()
            );
        }
        Some(CalibredbCommand::Info) => {
            println!("Caliberate DB CLI");
            println!("Library dir: {}", config.paths.library_dir.display());
            println!("DB path: {}", config.db.sqlite_path.display());
        }
        None => {
            println!("calibredb: no command provided (use --help)");
        }
    }

    Ok(())
}

fn resolve_device(
    config: &caliberate_core::config::DeviceConfig,
    name: Option<&str>,
) -> Result<DeviceInfo, Box<dyn std::error::Error>> {
    let devices = detect_devices(config)?;
    if devices.is_empty() {
        return Err("no devices detected".into());
    }
    if let Some(name) = name {
        return devices
            .into_iter()
            .find(|device| device.name == name)
            .ok_or_else(|| format!("device not found: {name}").into());
    }
    if devices.len() == 1 {
        return Ok(devices.into_iter().next().expect("device"));
    }
    Err("multiple devices detected; pass --device".into())
}

fn format_from_path(path: &str) -> Option<String> {
    let path = std::path::Path::new(path);
    let file_name = path.file_name()?.to_string_lossy();
    if let Some(stripped) = file_name.strip_suffix(".zst") {
        return std::path::Path::new(stripped)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_string());
    }
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_string())
}

fn parse_identifiers(
    values: &[String],
) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let mut parsed = Vec::new();
    for value in values {
        let (key, val) = value
            .split_once('=')
            .ok_or_else(|| format!("invalid identifier: {value}"))?;
        if key.trim().is_empty() || val.trim().is_empty() {
            return Err(format!("invalid identifier: {value}").into());
        }
        parsed.push((key.trim().to_string(), val.trim().to_string()));
    }
    Ok(parsed)
}

fn print_category(
    db: &Database,
    category: CategoryValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let (label, entries) = match category {
        CategoryValue::Authors => ("authors", db.list_author_categories()?),
        CategoryValue::Tags => ("tags", db.list_tag_categories()?),
        CategoryValue::Series => ("series", db.list_series_categories()?),
        CategoryValue::Publishers => ("publishers", db.list_publisher_categories()?),
        CategoryValue::Ratings => ("ratings", db.list_rating_categories()?),
        CategoryValue::Languages => ("languages", db.list_language_categories()?),
    };
    println!("Category: {label}");
    if entries.is_empty() {
        println!("(none)");
    } else {
        for entry in entries {
            println!("{}\t{}\t{}", entry.id, entry.name, entry.count);
        }
    }
    Ok(())
}

fn resolve_book_ids(
    db: &Database,
    ids: &[i64],
    all: bool,
) -> Result<Vec<i64>, Box<dyn std::error::Error>> {
    if all {
        let books = db.list_books()?;
        let ids = books.into_iter().map(|book| book.id).collect::<Vec<_>>();
        if ids.is_empty() {
            return Err("no books found in library".into());
        }
        return Ok(ids);
    }
    if ids.is_empty() {
        return Err("specify --id or --all".into());
    }
    let mut resolved = Vec::new();
    for id in ids {
        if db.get_book(*id)?.is_none() {
            return Err(format!("book not found: {id}").into());
        }
        resolved.push(*id);
    }
    Ok(resolved)
}

fn export_books(
    db: &Database,
    ids: &[i64],
    output_dir: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(output_dir)?;
    for book_id in ids {
        let assets = db.list_assets_for_book(*book_id)?;
        let book_dir = output_dir.join(format!("book-{book_id}"));
        std::fs::create_dir_all(&book_dir)?;
        for asset in assets {
            let file_name = std::path::Path::new(&asset.stored_path)
                .file_name()
                .ok_or("invalid asset path")?;
            let dest = book_dir.join(file_name);
            std::fs::copy(&asset.stored_path, &dest)?;
        }
    }
    Ok(())
}

#[derive(Debug, Serialize, serde::Deserialize)]
struct BackupIdentifier {
    id_type: String,
    value: String,
}

#[derive(Debug, Serialize, serde::Deserialize)]
struct BackupMetadata {
    id: i64,
    title: String,
    format: String,
    path: String,
    authors: Vec<String>,
    tags: Vec<String>,
    series: Option<String>,
    series_index: Option<f64>,
    identifiers: Vec<BackupIdentifier>,
    comment: Option<String>,
    publisher: Option<String>,
    rating: Option<i64>,
    languages: Vec<String>,
    uuid: Option<String>,
    timestamp: Option<String>,
    pubdate: Option<String>,
    last_modified: Option<String>,
}

#[derive(Debug, Serialize)]
struct ShowMetadataOutput {
    id: i64,
    title: String,
    format: String,
    path: String,
    authors: Vec<String>,
    tags: Vec<String>,
    series: Option<String>,
    series_index: Option<f64>,
    identifiers: Vec<BackupIdentifier>,
    comment: Option<String>,
    publisher: Option<String>,
    rating: Option<i64>,
    languages: Vec<String>,
    uuid: Option<String>,
    timestamp: Option<String>,
    pubdate: Option<String>,
    last_modified: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct BookDisplay {
    id: i64,
    title: String,
    format: String,
    path: String,
    authors: Vec<String>,
    tags: Vec<String>,
    series: Option<String>,
    series_index: Option<f64>,
    publisher: Option<String>,
    rating: Option<i64>,
    languages: Vec<String>,
    timestamp: Option<String>,
    pubdate: Option<String>,
    last_modified: Option<String>,
}

fn backup_metadata(
    db: &Database,
    ids: &[i64],
    output_dir: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(output_dir)?;
    for book_id in ids {
        let book = db
            .get_book(*book_id)?
            .ok_or("book not found while backing up metadata")?;
        let authors = db.list_book_authors(*book_id)?;
        let tags = db.list_book_tags(*book_id)?;
        let series = db.get_book_series(*book_id)?;
        let identifiers = db
            .list_book_identifiers(*book_id)?
            .into_iter()
            .map(|item| BackupIdentifier {
                id_type: item.id_type,
                value: item.value,
            })
            .collect::<Vec<_>>();
        let comment = db.get_book_comment(*book_id)?;
        let extras = db.get_book_extras(*book_id)?;
        let payload = BackupMetadata {
            id: book.id,
            title: book.title,
            format: book.format,
            path: book.path,
            authors,
            tags,
            series: series.as_ref().map(|entry| entry.name.clone()),
            series_index: series.map(|entry| entry.index),
            identifiers,
            comment,
            publisher: extras.publisher,
            rating: extras.rating,
            languages: extras.languages,
            uuid: extras.uuid,
            timestamp: extras.timestamp,
            pubdate: extras.pubdate,
            last_modified: extras.last_modified,
        };
        let out_path = output_dir.join(format!("metadata-{book_id}.json"));
        let encoded = serde_json::to_string_pretty(&payload)?;
        std::fs::write(out_path, encoded)?;
    }
    Ok(())
}

fn build_displays(
    db: &Database,
    books: &[caliberate_db::database::BookRecord],
) -> Result<Vec<BookDisplay>, Box<dyn std::error::Error>> {
    let mut displays = Vec::new();
    for book in books {
        displays.push(build_display(db, book)?);
    }
    Ok(displays)
}

fn build_display(
    db: &Database,
    book: &caliberate_db::database::BookRecord,
) -> Result<BookDisplay, Box<dyn std::error::Error>> {
    let authors = db.list_book_authors(book.id)?;
    let tags = db.list_book_tags(book.id)?;
    let series = db.get_book_series(book.id)?;
    let extras = db.get_book_extras(book.id)?;
    Ok(BookDisplay {
        id: book.id,
        title: book.title.clone(),
        format: book.format.clone(),
        path: book.path.clone(),
        authors,
        tags,
        series: series.as_ref().map(|entry| entry.name.clone()),
        series_index: series.map(|entry| entry.index),
        publisher: extras.publisher,
        rating: extras.rating,
        languages: extras.languages,
        timestamp: extras.timestamp,
        pubdate: extras.pubdate,
        last_modified: extras.last_modified,
    })
}

fn render_books(
    displays: &[BookDisplay],
    fields: &[String],
    sort_by: Option<&str>,
    ascending: bool,
    limit: Option<usize>,
    for_machine: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let selected = parse_fields(fields)?;
    let mut items = displays.to_vec();
    if let Some(sort_by) = sort_by {
        sort_displays(&mut items, sort_by, ascending)?;
    }
    if let Some(limit) = limit {
        items.truncate(limit);
    }
    if for_machine {
        for item in items {
            let value = build_json_fields(&item, &selected)?;
            println!("{}", serde_json::to_string(&value)?);
        }
    } else {
        for item in items {
            let line = selected
                .iter()
                .map(|field| field_to_string(&item, field))
                .collect::<Result<Vec<_>, _>>()?
                .join("\t");
            println!("{line}");
        }
    }
    Ok(())
}

fn parse_fields(values: &[String]) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    if values.is_empty() {
        return Ok(vec![
            "id".to_string(),
            "title".to_string(),
            "format".to_string(),
            "path".to_string(),
        ]);
    }
    let mut fields = Vec::new();
    for value in values {
        for part in value.split(',') {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }
            fields.push(trimmed.to_string());
        }
    }
    if fields.is_empty() {
        return Err("no fields selected".into());
    }
    Ok(fields)
}

fn sort_displays(
    items: &mut [BookDisplay],
    sort_by: &str,
    ascending: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !matches!(
        sort_by,
        "id" | "title"
            | "format"
            | "path"
            | "authors"
            | "author"
            | "tags"
            | "series"
            | "publisher"
            | "rating"
            | "languages"
            | "timestamp"
            | "pubdate"
            | "last_modified"
    ) {
        return Err(format!("unknown sort field: {sort_by}").into());
    }
    let key = sort_by.to_string();
    items.sort_by(|a, b| {
        let left = sort_key(a, &key);
        let right = sort_key(b, &key);
        if ascending {
            left.cmp(&right)
        } else {
            right.cmp(&left)
        }
    });
    Ok(())
}

fn sort_key(display: &BookDisplay, field: &str) -> String {
    match field {
        "id" => display.id.to_string(),
        "title" => display.title.clone(),
        "format" => display.format.clone(),
        "path" => display.path.clone(),
        "authors" | "author" => display.authors.join(", "),
        "tags" => display.tags.join(", "),
        "series" => display.series.clone().unwrap_or_default(),
        "publisher" => display.publisher.clone().unwrap_or_default(),
        "rating" => display.rating.map(|v| v.to_string()).unwrap_or_default(),
        "languages" => display.languages.join(", "),
        "timestamp" => display.timestamp.clone().unwrap_or_default(),
        "pubdate" => display.pubdate.clone().unwrap_or_default(),
        "last_modified" => display.last_modified.clone().unwrap_or_default(),
        _ => String::new(),
    }
}

fn field_to_string(
    display: &BookDisplay,
    field: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let value = match field {
        "id" => display.id.to_string(),
        "title" => display.title.clone(),
        "format" => display.format.clone(),
        "path" => display.path.clone(),
        "authors" | "author" => display.authors.join(", "),
        "tags" => display.tags.join(", "),
        "series" => display.series.clone().unwrap_or_default(),
        "series_index" => display
            .series_index
            .map(|value| value.to_string())
            .unwrap_or_default(),
        "publisher" => display.publisher.clone().unwrap_or_default(),
        "rating" => display
            .rating
            .map(|value| value.to_string())
            .unwrap_or_default(),
        "languages" => display.languages.join(", "),
        "timestamp" => display.timestamp.clone().unwrap_or_default(),
        "pubdate" => display.pubdate.clone().unwrap_or_default(),
        "last_modified" => display.last_modified.clone().unwrap_or_default(),
        _ => return Err(format!("unknown field: {field}").into()),
    };
    Ok(value)
}

fn build_json_fields(
    display: &BookDisplay,
    fields: &[String],
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut map = serde_json::Map::new();
    for field in fields {
        let value = match field.as_str() {
            "id" => serde_json::Value::Number(display.id.into()),
            "title" => serde_json::Value::String(display.title.clone()),
            "format" => serde_json::Value::String(display.format.clone()),
            "path" => serde_json::Value::String(display.path.clone()),
            "authors" | "author" => serde_json::Value::Array(
                display
                    .authors
                    .iter()
                    .map(|value| serde_json::Value::String(value.clone()))
                    .collect(),
            ),
            "tags" => serde_json::Value::Array(
                display
                    .tags
                    .iter()
                    .map(|value| serde_json::Value::String(value.clone()))
                    .collect(),
            ),
            "series" => display
                .series
                .clone()
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null),
            "series_index" => display
                .series_index
                .and_then(serde_json::Number::from_f64)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            "publisher" => display
                .publisher
                .clone()
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null),
            "rating" => display
                .rating
                .map(|value| serde_json::Value::Number(value.into()))
                .unwrap_or(serde_json::Value::Null),
            "languages" => serde_json::Value::Array(
                display
                    .languages
                    .iter()
                    .map(|value| serde_json::Value::String(value.clone()))
                    .collect(),
            ),
            "timestamp" => display
                .timestamp
                .clone()
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null),
            "pubdate" => display
                .pubdate
                .clone()
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null),
            "last_modified" => display
                .last_modified
                .clone()
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null),
            _ => return Err(format!("unknown field: {field}").into()),
        };
        map.insert(field.clone(), value);
    }
    Ok(serde_json::Value::Object(map))
}

fn build_metadata_output(
    db: &Database,
    id: i64,
) -> Result<ShowMetadataOutput, Box<dyn std::error::Error>> {
    let book = db
        .get_book(id)?
        .ok_or_else(|| format!("Id #{id} is not present in database."))?;
    let authors = db.list_book_authors(id)?;
    let tags = db.list_book_tags(id)?;
    let series = db.get_book_series(id)?;
    let identifiers = db
        .list_book_identifiers(id)?
        .into_iter()
        .map(|item| BackupIdentifier {
            id_type: item.id_type,
            value: item.value,
        })
        .collect::<Vec<_>>();
    let comment = db.get_book_comment(id)?;
    let extras = db.get_book_extras(id)?;
    Ok(ShowMetadataOutput {
        id: book.id,
        title: book.title,
        format: book.format,
        path: book.path,
        authors,
        tags,
        series: series.as_ref().map(|entry| entry.name.clone()),
        series_index: series.map(|entry| entry.index),
        identifiers,
        comment,
        publisher: extras.publisher,
        rating: extras.rating,
        languages: extras.languages,
        uuid: extras.uuid,
        timestamp: extras.timestamp,
        pubdate: extras.pubdate,
        last_modified: extras.last_modified,
    })
}

fn render_opf(metadata: &ShowMetadataOutput) -> Result<String, Box<dyn std::error::Error>> {
    let mut opf = String::new();
    opf.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    opf.push_str(
        "<package version=\"2.0\" xmlns=\"http://www.idpf.org/2007/opf\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n",
    );
    opf.push_str("  <metadata>\n");
    opf.push_str(&format!(
        "    <dc:title>{}</dc:title>\n",
        xml_escape(&metadata.title)
    ));
    for author in &metadata.authors {
        opf.push_str(&format!(
            "    <dc:creator>{}</dc:creator>\n",
            xml_escape(author)
        ));
    }
    for tag in &metadata.tags {
        opf.push_str(&format!(
            "    <dc:subject>{}</dc:subject>\n",
            xml_escape(tag)
        ));
    }
    for identifier in &metadata.identifiers {
        opf.push_str(&format!(
            "    <dc:identifier id=\"{}\">{}</dc:identifier>\n",
            xml_escape(&identifier.id_type),
            xml_escape(&identifier.value)
        ));
    }
    if let Some(series) = metadata.series.as_ref() {
        opf.push_str(&format!(
            "    <meta name=\"calibre:series\" content=\"{}\" />\n",
            xml_escape(series)
        ));
    }
    if let Some(series_index) = metadata.series_index {
        opf.push_str(&format!(
            "    <meta name=\"calibre:series_index\" content=\"{}\" />\n",
            series_index
        ));
    }
    if let Some(publisher) = metadata.publisher.as_ref() {
        opf.push_str(&format!(
            "    <dc:publisher>{}</dc:publisher>\n",
            xml_escape(publisher)
        ));
    }
    if let Some(comment) = metadata.comment.as_ref() {
        opf.push_str(&format!(
            "    <dc:description>{}</dc:description>\n",
            xml_escape(comment)
        ));
    }
    opf.push_str("  </metadata>\n");
    opf.push_str("</package>\n");
    Ok(opf)
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
        .replace('\'', "&apos;")
}

fn print_metadata_fields() {
    println!("title");
    println!("authors");
    println!("tags");
    println!("series");
    println!("series_index");
    println!("identifiers");
    println!("comment");
    println!("publisher");
    println!("rating");
    println!("languages");
    println!("timestamp");
    println!("pubdate");
    println!("last_modified");
}

fn apply_metadata_fields(
    db: &mut Database,
    id: i64,
    fields: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fields {
        let (field, raw) = entry
            .split_once(':')
            .ok_or_else(|| format!("invalid field format: {entry}"))?;
        match field {
            "title" => {
                db.update_book_title(id, raw)?;
            }
            "authors" => {
                let values = split_list(raw);
                db.replace_book_authors(id, &values)?;
            }
            "tags" => {
                let values = split_list(raw);
                db.replace_book_tags(id, &values)?;
            }
            "series" => {
                db.set_book_series(id, raw, 1.0)?;
            }
            "series_index" => {
                let index: f64 = raw.parse()?;
                if let Some(series) = db.get_book_series(id)? {
                    db.set_book_series(id, &series.name, index)?;
                } else {
                    return Err("series_index requires series to be set".into());
                }
            }
            "identifiers" => {
                let identifiers = parse_identifiers_field(raw)?;
                db.replace_book_identifiers(id, &identifiers)?;
            }
            "comment" => {
                db.set_book_comment(id, raw)?;
            }
            "publisher" => {
                db.set_book_publisher(id, raw)?;
            }
            "rating" => {
                let rating: i64 = raw.parse()?;
                db.set_book_rating(id, rating)?;
            }
            "languages" => {
                let values = split_list(raw);
                db.set_book_languages(id, &values)?;
            }
            "timestamp" => {
                db.update_book_timestamp(id, raw)?;
            }
            "pubdate" => {
                db.update_book_pubdate(id, raw)?;
            }
            "last_modified" => {
                db.update_book_last_modified(id, raw)?;
            }
            _ => {
                return Err(format!("unknown field: {field}").into());
            }
        }
    }
    Ok(())
}

fn split_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn parse_identifiers_field(raw: &str) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let mut identifiers = Vec::new();
    for entry in raw.split(',') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (key, value) = trimmed
            .split_once(':')
            .ok_or_else(|| format!("invalid identifier: {trimmed}"))?;
        identifiers.push((key.trim().to_string(), value.trim().to_string()));
    }
    Ok(identifiers)
}

fn write_catalog(
    db: &Database,
    ids: &[i64],
    output: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut writer = csv::Writer::from_path(output)?;
    writer.write_record([
        "id",
        "title",
        "authors",
        "tags",
        "series",
        "publisher",
        "rating",
        "languages",
        "formats",
    ])?;
    for book_id in ids {
        let book = db
            .get_book(*book_id)?
            .ok_or("book not found while writing catalog")?;
        let authors = db.list_book_authors(*book_id)?.join(", ");
        let tags = db.list_book_tags(*book_id)?.join(", ");
        let series = db
            .get_book_series(*book_id)?
            .map(|entry| entry.name)
            .unwrap_or_default();
        let extras = db.get_book_extras(*book_id)?;
        let publisher = extras.publisher.unwrap_or_default();
        let rating = extras
            .rating
            .map(|value| value.to_string())
            .unwrap_or_default();
        let languages = extras.languages.join(", ");
        let formats = db
            .list_assets_for_book(*book_id)?
            .into_iter()
            .filter_map(|asset| format_from_path(&asset.stored_path))
            .collect::<Vec<_>>()
            .join(", ");
        writer.write_record([
            book.id.to_string(),
            book.title,
            authors,
            tags,
            series,
            publisher,
            rating,
            languages,
            formats,
        ])?;
    }
    writer.flush()?;
    Ok(())
}

fn restore_database(
    db: &mut Database,
    input_dir: &PathBuf,
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut restored = 0usize;
    if !input_dir.exists() {
        return Err("input directory does not exist".into());
    }
    for entry in std::fs::read_dir(input_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&path)?;
        let metadata: BackupMetadata = serde_json::from_str(&raw)?;
        if db.get_book(metadata.id)?.is_none() {
            continue;
        }
        db.update_book_title(metadata.id, &metadata.title)?;
        db.replace_book_authors(metadata.id, &metadata.authors)?;
        db.replace_book_tags(metadata.id, &metadata.tags)?;
        if let Some(series) = metadata.series.as_ref() {
            let index = metadata.series_index.unwrap_or(1.0);
            db.set_book_series(metadata.id, series, index)?;
        }
        let identifiers = metadata
            .identifiers
            .into_iter()
            .map(|item| (item.id_type, item.value))
            .collect::<Vec<_>>();
        db.replace_book_identifiers(metadata.id, &identifiers)?;
        if let Some(comment) = metadata.comment.as_ref() {
            db.set_book_comment(metadata.id, comment)?;
        }
        if let Some(publisher) = metadata.publisher.as_ref() {
            db.set_book_publisher(metadata.id, publisher)?;
        }
        if let Some(rating) = metadata.rating {
            db.set_book_rating(metadata.id, rating)?;
        }
        if !metadata.languages.is_empty() {
            db.set_book_languages(metadata.id, &metadata.languages)?;
        }
        if let Some(timestamp) = metadata.timestamp.as_ref() {
            db.update_book_timestamp(metadata.id, timestamp)?;
        }
        if let Some(pubdate) = metadata.pubdate.as_ref() {
            db.update_book_pubdate(metadata.id, pubdate)?;
        }
        if let Some(last_modified) = metadata.last_modified.as_ref() {
            db.update_book_last_modified(metadata.id, last_modified)?;
        }
        restored += 1;
    }
    Ok(restored)
}

fn clone_library(
    config: &caliberate_core::config::ControlPlane,
    output_dir: &PathBuf,
    include_references: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(output_dir)?;
    let output_library_dir = output_dir.join("library");
    std::fs::create_dir_all(&output_library_dir)?;
    let output_db_path = output_dir.join("caliberate.db");
    std::fs::copy(&config.db.sqlite_path, &output_db_path)?;

    let mut cloned_config = config.db.clone();
    cloned_config.sqlite_path = output_db_path;
    let db = Database::open_with_fts(&cloned_config, &config.fts)?;

    let assets = db.list_assets()?;
    for asset in &assets {
        let stored_path = PathBuf::from(&asset.stored_path);
        if stored_path.starts_with(&config.paths.library_dir) {
            let relative = stored_path.strip_prefix(&config.paths.library_dir)?;
            let dest = output_library_dir.join(relative);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&stored_path, &dest)?;
            db.update_asset_paths(
                asset.id,
                &dest.display().to_string(),
                &asset.storage_mode,
                asset.source_path.as_deref(),
            )?;
        } else if include_references {
            let file_name = stored_path.file_name().ok_or("invalid stored path")?;
            let dest = output_library_dir.join("references").join(file_name);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&stored_path, &dest)?;
            db.update_asset_paths(
                asset.id,
                &dest.display().to_string(),
                "copy",
                Some(&asset.stored_path),
            )?;
        }
    }

    let books = db.list_books()?;
    for book in books {
        let path = PathBuf::from(&book.path);
        if path.starts_with(&config.paths.library_dir) {
            let relative = path.strip_prefix(&config.paths.library_dir)?;
            let dest = output_library_dir.join(relative);
            db.update_book_path(book.id, &dest.display().to_string())?;
        }
    }
    Ok(())
}

fn embed_metadata(db: &Database, ids: &[i64]) -> Result<usize, Box<dyn std::error::Error>> {
    let mut written = 0usize;
    for book_id in ids {
        let book = db.get_book(*book_id)?.ok_or("book not found")?;
        let authors = db.list_book_authors(*book_id)?;
        let tags = db.list_book_tags(*book_id)?;
        let identifiers = db.list_book_identifiers(*book_id)?;
        let comment = db.get_book_comment(*book_id)?;
        let extras = db.get_book_extras(*book_id)?;
        let series = db.get_book_series(*book_id)?;

        let mut opf = String::new();
        opf.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        opf.push_str(
            "<package version=\"2.0\" xmlns=\"http://www.idpf.org/2007/opf\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n",
        );
        opf.push_str("  <metadata>\n");
        opf.push_str(&format!("    <dc:title>{}</dc:title>\n", book.title));
        for author in &authors {
            opf.push_str(&format!("    <dc:creator>{author}</dc:creator>\n"));
        }
        for tag in &tags {
            opf.push_str(&format!("    <dc:subject>{tag}</dc:subject>\n"));
        }
        for identifier in &identifiers {
            opf.push_str(&format!(
                "    <dc:identifier id=\"{}\">{}</dc:identifier>\n",
                identifier.id_type, identifier.value
            ));
        }
        if let Some(series) = series.as_ref() {
            opf.push_str(&format!(
                "    <meta name=\"calibre:series\" content=\"{}\" />\n",
                series.name
            ));
            opf.push_str(&format!(
                "    <meta name=\"calibre:series_index\" content=\"{}\" />\n",
                series.index
            ));
        }
        if let Some(publisher) = extras.publisher.as_ref() {
            opf.push_str(&format!("    <dc:publisher>{publisher}</dc:publisher>\n"));
        }
        if let Some(comment) = comment.as_ref() {
            opf.push_str(&format!("    <dc:description>{comment}</dc:description>\n"));
        }
        opf.push_str("  </metadata>\n");
        opf.push_str("</package>\n");

        let path = PathBuf::from(&book.path);
        let dir = path.parent().ok_or("book path missing parent")?;
        std::fs::create_dir_all(dir)?;
        let opf_path = dir.join("metadata.opf");
        std::fs::write(opf_path, opf)?;
        written += 1;
    }
    Ok(written)
}

fn delete_asset_files(
    assets: &[caliberate_db::database::AssetRow],
    delete_reference_files: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    for asset in assets {
        let should_delete = match asset.storage_mode.as_str() {
            "copy" => true,
            "reference" => delete_reference_files,
            _ => false,
        };
        if !should_delete {
            continue;
        }
        let path = std::path::Path::new(&asset.stored_path);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn asset_to_descriptor(
    asset: &caliberate_db::database::AssetRow,
) -> Result<AssetDescriptor, Box<dyn std::error::Error>> {
    let storage_mode = match asset.storage_mode.as_str() {
        "copy" => StorageMode::Copy,
        "reference" => StorageMode::Reference,
        other => {
            return Err(format!("unsupported storage mode: {other}").into());
        }
    };

    Ok(AssetDescriptor {
        id: asset.id,
        stored_path: PathBuf::from(&asset.stored_path),
        storage_mode,
        size_bytes: asset.size_bytes,
        stored_size_bytes: asset.stored_size_bytes,
        checksum: asset.checksum.clone(),
        is_compressed: asset.is_compressed,
    })
}
