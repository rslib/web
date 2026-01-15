use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use log::{LevelFilter, error};
use std::path::{Path, PathBuf};
use std::time::Instant;

use rs_web::build::Builder;
use rs_web::config::Config;
use rs_web::watch::{FileWatcher, format_changes};

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
enum LogLevel {
    Trace,
    Debug,
    Info,
    #[default]
    Warning,
    Error,
}

impl From<LogLevel> for LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => LevelFilter::Trace,
            LogLevel::Debug => LevelFilter::Debug,
            LogLevel::Info => LevelFilter::Info,
            LogLevel::Warning => LevelFilter::Warn,
            LogLevel::Error => LevelFilter::Error,
        }
    }
}

#[derive(Parser)]
#[command(name = "rs-web")]
#[command(about = "A custom static site generator", long_about = None)]
struct Cli {
    /// Enable debug logging (shorthand for --log-level debug)
    #[arg(long, global = true)]
    debug: bool,

    /// Set the logging level (can also use RS_WEB_LOG_LEVEL env var)
    #[arg(long, value_enum, global = true)]
    log_level: Option<LogLevel>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build the static site
    Build {
        /// Project directory containing config.lua or config.toml
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

fn init_logger(debug: bool, log_level: Option<LogLevel>) {
    let level = if debug {
        // --debug flag takes highest priority
        LevelFilter::Debug
    } else if let Some(level) = log_level {
        // Explicit --log-level arg
        level.into()
    } else if let Ok(env_level) = std::env::var("RS_WEB_LOG_LEVEL") {
        // Environment variable
        match env_level.to_lowercase().as_str() {
            "trace" => LevelFilter::Trace,
            "debug" => LevelFilter::Debug,
            "info" => LevelFilter::Info,
            "warning" | "warn" => LevelFilter::Warn,
            "error" => LevelFilter::Error,
            _ => LevelFilter::Warn, // Invalid value, use default
        }
    } else {
        // Default
        LevelFilter::Warn
    };

    env_logger::Builder::new()
        .filter_level(level)
        .format_timestamp(None)
        .format_target(false)
        .init();
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    init_logger(cli.debug, cli.log_level);

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

            // Load config from project directory (supports both config.lua and config.toml)
            let (mut config, lua_config) = Config::load_with_lua(&project_dir)?;

            // Allow overriding base_url via environment variable (useful for CI/CD)
            if let Ok(base_url) = std::env::var("SITE_BASE_URL") {
                log::info!("Using base_url from SITE_BASE_URL: {}", base_url);
                config.site.base_url = base_url;
            }

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

            let mut builder = Builder::new(config, output_dir.clone(), project_dir.clone())
                .with_lua_config(lua_config);

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
            error!("Failed to reload config: {}", e);
            continue;
        }

        // Perform incremental build
        match builder.incremental_build(&changes) {
            Ok(()) => {
                println!("Rebuilt in {:?}\n", start.elapsed());
            }
            Err(e) => {
                error!("Build failed: {}", e);
            }
        }
    }
}
