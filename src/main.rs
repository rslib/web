use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Instant;

mod assets;
mod build;
mod config;
mod content;
mod encryption;
mod links;
mod markdown;
mod rss;
mod templates;

use crate::build::Builder;
use crate::config::Config;

#[derive(Parser)]
#[command(name = "rs-web")]
#[command(about = "A custom static site generator", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build the static site
    Build {
        /// Output directory
        #[arg(short, long, default_value = "dist")]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build { output } => {
            let start = Instant::now();

            let config = Config::load("config.toml")?;
            let mut builder = Builder::new(config, output);
            builder.build()?;

            println!("Built in {:?}", start.elapsed());
        }
    }

    Ok(())
}
