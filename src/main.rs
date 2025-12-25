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
mod text;

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
        /// Project directory containing config.toml
        #[arg(short = 'd', long = "dir")]
        directory: Option<PathBuf>,

        /// Output directory (relative to cwd; defaults to config's output_dir in project)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build { directory, output } => {
            let start = Instant::now();

            // Determine project directory and change to it if specified
            let project_dir = directory.unwrap_or_else(|| PathBuf::from("."));
            let project_dir = project_dir.canonicalize().unwrap_or(project_dir);

            // Config path is relative to project directory
            let config_path = project_dir.join("config.toml");
            let config = Config::load(&config_path)?;

            // Output directory:
            // - If -o specified: relative to current working directory
            // - If not specified: use config value relative to project directory
            let output_dir = if let Some(out) = output {
                if out.is_absolute() {
                    out
                } else {
                    std::env::current_dir()?.join(out)
                }
            } else {
                project_dir.join(&config.build.output_dir)
            };

            let mut builder = Builder::new(config, output_dir, project_dir);
            builder.build()?;

            println!("Built in {:?}", start.elapsed());
        }
    }

    Ok(())
}
