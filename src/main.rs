use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::time::Instant;

use rs_web::build::Builder;
use rs_web::config::Config;
use rs_web::watch::{FileWatcher, format_changes};

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

        /// Watch for changes and rebuild incrementally
        #[arg(short, long)]
        watch: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build {
            directory,
            output,
            watch,
        } => {
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

            let mut builder = Builder::new(config, output_dir.clone(), project_dir.clone());

            // Initial build
            builder.build()?;
            println!("Built in {:?}", start.elapsed());

            // Watch mode
            if watch {
                run_watch_loop(builder, &project_dir, &output_dir)?;
            }
        }
    }

    Ok(())
}

/// Run the watch loop for incremental builds
fn run_watch_loop(mut builder: Builder, project_dir: &Path, output_dir: &Path) -> Result<()> {
    println!("\nEntering watch mode. Press Ctrl+C to stop.\n");

    let watcher = FileWatcher::new(project_dir, builder.config(), output_dir)?;

    loop {
        let changes = watcher.wait_for_changes()?;

        if changes.is_empty() {
            continue;
        }

        println!("\nChange detected: {}", format_changes(&changes));
        let start = Instant::now();

        // Reload config if it changed
        if changes.full_rebuild
            && let Err(e) = builder.reload_config()
        {
            eprintln!("Failed to reload config: {}", e);
            continue;
        }

        // Perform incremental build
        match builder.incremental_build(&changes) {
            Ok(()) => {
                println!("Rebuilt in {:?}\n", start.elapsed());
            }
            Err(e) => {
                eprintln!("Build failed: {}\n", e);
            }
        }
    }
}
