use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "polymarket-analysis")]
#[command(about = "Collect Polymarket and odds-site match data")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Collect(CollectArgs),
    Export(ExportArgs),
}

#[derive(Debug, Parser)]
pub struct CollectArgs {
    #[arg(long)]
    pub polymarket_url: Option<String>,
    #[arg(long)]
    pub odds_url: Option<String>,
    #[arg(long, default_value_t = 1)]
    pub polymarket_interval_seconds: u64,
    #[arg(long, default_value_t = 60)]
    pub odds_interval_seconds: u64,
    #[arg(long, default_value = "data/polymarket_analysis.sqlite")]
    pub db: PathBuf,
}

#[derive(Debug, Parser)]
pub struct ExportArgs {
    #[arg(long)]
    pub db: PathBuf,
    #[arg(long)]
    pub match_id: String,
    #[arg(long)]
    pub format: ExportFormat,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum ExportFormat {
    Jsonl,
    Csv,
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Collect(args) => {
            if args.polymarket_url.is_none() && args.odds_url.is_none() {
                bail!("at least one of --polymarket-url or --odds-url is required");
            }
            if args.polymarket_interval_seconds == 0 {
                bail!("--polymarket-interval-seconds must be greater than 0");
            }
            if args.odds_interval_seconds == 0 {
                bail!("--odds-interval-seconds must be greater than 0");
            }
            crate::collector::collect(args).await
        }
        Command::Export(args) => crate::storage::export_match(args).await,
    }
}
