use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use faa_registry_mirror::db::ingest::{self, IngestOptions, DEFAULT_MIN_MASTER_ROWS};
use faa_registry_mirror::db::query;
use faa_registry_mirror::download::{resolve_user_agent, DEFAULT_ZIP_URL};

#[derive(Parser)]
#[command(
    name = "faa-registry-mirror",
    about = "Capture and historically track FAA aircraft ownership in SQLite",
    version
)]
struct Cli {
    /// SQLite database path
    #[arg(
        long,
        global = true,
        default_value = "data/faa-registry.sqlite",
        env = "FAA_REGISTRY_DB"
    )]
    db: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Download (or read) ReleasableAircraft.zip and upsert ownership history
    Ingest {
        /// Local zip instead of downloading
        #[arg(long)]
        zip: Option<PathBuf>,
        /// After an origin GET, write ReleasableAircraft.zip here for later `--zip` reruns
        #[arg(long, env = "FAA_REGISTRY_CACHE")]
        cache_dir: Option<PathBuf>,
        /// FAA zip URL used when --zip is omitted
        #[arg(long, default_value = DEFAULT_ZIP_URL)]
        url: String,
        /// Abort if MASTER.txt has fewer data rows than this (truncated dump guard)
        #[arg(long, default_value_t = DEFAULT_MIN_MASTER_ROWS)]
        min_master_rows: usize,
        /// Re-run SCD even if zip_hash matches the last ok ingest
        #[arg(long)]
        force: bool,
        /// Override FAA download User-Agent (else FAA_USER_AGENT, else Safari token)
        #[arg(long)]
        faa_user_agent: Option<String>,
    },
    /// Show current registration and ownership history for an N-number or Mode S hex
    Lookup {
        n_number: String,
    },
    /// Full-text search of current registrant names
    SearchOwner {
        name: String,
        #[arg(long, default_value_t = 25)]
        limit: usize,
    },
    /// Show the most recent ingest run
    Status,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Ingest {
            zip,
            cache_dir,
            url,
            min_master_rows,
            force,
            faa_user_agent,
        } => {
            let stats = ingest::ingest(&IngestOptions {
                db_path: cli.db,
                zip_path: zip,
                cache_dir,
                zip_url: url,
                min_master_rows,
                force,
                user_agent: resolve_user_agent(faa_user_agent.as_deref()),
            })?;
            if stats.skipped_same_zip {
                println!(
                    "ingest skipped  same zip_hash as last ok run ({})",
                    stats.zip_hash
                );
            } else {
                println!(
                    "ingest ok  master={}  new={}  changed={}  closed={}  unchanged={}  dereg_new={}  dereg_changed={}  dereg_closed={}  docs={}  dealers={}  reserved={}  skipped={}",
                    stats.master_rows,
                    stats.new_rows,
                    stats.changed_rows,
                    stats.closed_rows,
                    stats.unchanged_rows,
                    stats.dereg_new,
                    stats.dereg_changed,
                    stats.dereg_closed,
                    stats.documents_inserted,
                    stats.dealer_rows,
                    stats.reserved_rows,
                    stats.skipped_rows
                );
            }
        }
        Command::Lookup { n_number } => {
            let conn = faa_registry_mirror::db::open(&cli.db)?;
            let result = query::lookup(&conn, &n_number)?;
            print!("{}", query::format_lookup(&result));
        }
        Command::SearchOwner { name, limit } => {
            let conn = faa_registry_mirror::db::open(&cli.db)?;
            let hits = query::search_owner(&conn, &name, limit)?;
            print!("{}", query::format_owner_hits(&hits));
        }
        Command::Status => {
            let conn = faa_registry_mirror::db::open(&cli.db)?;
            match query::latest_status(&conn).context("read ingest status")? {
                Some(status) => print!("{}", query::format_status(&status)),
                None => println!("No ingest runs yet."),
            }
        }
    }
    Ok(())
}
