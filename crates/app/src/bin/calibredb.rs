use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use caliberate_assets::storage::LocalAssetStore;
use caliberate_core::config::IngestMode;
use caliberate_db::database::Database;
use caliberate_library::ingest::{IngestRequest, Ingestor};

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
    List,
    Search {
        #[arg(long)]
        query: String,
    },
    Info,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum IngestModeValue {
    Copy,
    Reference,
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
            let _db = Database::open(&config.db)?;
            println!("Database initialized at {}", config.db.sqlite_path.display());
        }
        Some(CalibredbCommand::Add { path, mode }) => {
            let store = LocalAssetStore::from_config(&config);
            let ingestor = Ingestor::new(std::sync::Arc::new(store), config.clone());
            let request = IngestRequest {
                source_path: &path,
                mode: mode.map(Into::into),
            };
            let result = ingestor.ingest(request)?;

            let db = Database::open(&config.db)?;
            let created_at = time::OffsetDateTime::now_utc().format(
                &time::format_description::parse("[year]-[month]-[day]T[hour]:[minute]:[second]Z")?,
            )?;
            let id = db.add_book(
                &result.metadata.title,
                &result.metadata.format,
                &result.asset.stored_path.display().to_string(),
                &created_at,
            )?;

            println!("Added book {id}");
        }
        Some(CalibredbCommand::List) => {
            let db = Database::open(&config.db)?;
            for book in db.list_books()? {
                println!("{}\t{}\t{}\t{}", book.id, book.title, book.format, book.path);
            }
        }
        Some(CalibredbCommand::Search { query }) => {
            let db = Database::open(&config.db)?;
            for book in db.search_books(&query)? {
                println!("{}\t{}\t{}\t{}", book.id, book.title, book.format, book.path);
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
