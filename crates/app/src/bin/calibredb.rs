use clap::{Parser, Subcommand, ValueEnum};
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
    List,
    Search {
        #[arg(long)]
        query: String,
    },
    Assets {
        #[command(subcommand)]
        command: AssetsCommand,
    },
    Fts {
        #[command(subcommand)]
        command: FtsCommand,
    },
    Formats {
        #[command(subcommand)]
        command: FormatsCommand,
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
    let config = bootstrap.config;

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
        Some(CalibredbCommand::List) => {
            let db = Database::open_with_fts(&config.db, &config.fts)?;
            for book in db.list_books()? {
                println!(
                    "{}\t{}\t{}\t{}",
                    book.id, book.title, book.format, book.path
                );
            }
        }
        Some(CalibredbCommand::Search { query }) => {
            let db = Database::open_with_fts(&config.db, &config.fts)?;
            for book in db.search_books(&query)? {
                println!(
                    "{}\t{}\t{}\t{}",
                    book.id, book.title, book.format, book.path
                );
            }
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
