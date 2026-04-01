use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use caliberate_assets::stats::{
    AssetDescriptor, apply_compaction, compute_storage_stats, plan_compaction, verify_assets,
};
use caliberate_assets::storage::{LocalAssetStore, StorageMode};
use caliberate_core::config::IngestMode;
use caliberate_db::database::Database;
use caliberate_library::ingest::{IngestOutcome, IngestRequest, Ingestor};
use caliberate_metadata::extract::extract_archive_entry;

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

            let db = Database::open_with_fts(&config.db, &config.fts)?;
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

            println!("Added book {id}");
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
