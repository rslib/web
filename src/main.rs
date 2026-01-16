use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use log::{LevelFilter, error};
use std::path::{Path, PathBuf};
use std::time::Instant;

use rs_web::build::Builder;
use rs_web::config::Config;
use rs_web::server::{ReloadMessage, ServerConfig, notify_reload, run_server};
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
#[command(about = "A data-driven static site generator", long_about = None)]
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
        /// Project directory containing config.lua
        #[arg(short = 'd', long = "dir")]
        directory: Option<PathBuf>,

        /// Output directory (relative to cwd; defaults to config's output_dir in project)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Watch for changes and rebuild incrementally
        #[arg(short, long)]
        watch: bool,
    },
    /// Generate Lua API type definitions
    Types {
        /// Generate EmmyLua annotations (.lua file)
        #[arg(long)]
        lua: bool,

        /// Generate Markdown documentation (.md file)
        #[arg(long)]
        markdown: bool,

        /// Output directory (default: current directory)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Start development server with live reload
    Serve {
        /// Project directory containing config.lua
        #[arg(short = 'd', long = "dir")]
        directory: Option<PathBuf>,

        /// Output directory to serve (relative to cwd; defaults to config's output_dir)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,

        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Watch for changes and rebuild (enables live reload)
        #[arg(short, long)]
        watch: bool,

        /// Build before serving (default: true unless --no-build)
        #[arg(long = "no-build")]
        no_build: bool,
    },
}

fn init_logger(debug: bool, log_level: Option<LogLevel>) {
    let level = if debug {
        LevelFilter::Debug
    } else if let Some(level) = log_level {
        level.into()
    } else if let Ok(env_level) = std::env::var("RS_WEB_LOG_LEVEL") {
        match env_level.to_lowercase().as_str() {
            "trace" => LevelFilter::Trace,
            "debug" => LevelFilter::Debug,
            "info" => LevelFilter::Info,
            "warning" | "warn" => LevelFilter::Warn,
            "error" => LevelFilter::Error,
            _ => LevelFilter::Warn,
        }
    } else {
        LevelFilter::Warn
    };

    env_logger::Builder::new()
        .filter_level(level)
        .format_timestamp(None)
        .format_target(false)
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    init_logger(cli.debug, cli.log_level);

    match cli.command {
        Commands::Build {
            directory,
            output,
            watch,
        } => {
            let start = Instant::now();

            // Determine project directory
            let project_dir = directory.unwrap_or_else(|| PathBuf::from("."));
            let project_dir = project_dir.canonicalize().unwrap_or(project_dir);

            // Load config from project directory
            let mut config = Config::load(&project_dir)?;

            // Allow overriding base_url via environment variable
            if let Ok(base_url) = std::env::var("SITE_BASE_URL") {
                log::info!("Using base_url from SITE_BASE_URL: {}", base_url);
                config.site.base_url = base_url;
            }

            // Output directory
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
        Commands::Types {
            lua,
            markdown,
            output,
        } => {
            // Default to generating both if neither specified
            let generate_lua = lua || !markdown;
            let generate_markdown = markdown || !lua;

            let output_path = output.unwrap_or_else(|| PathBuf::from("."));

            // Check if output is a file path or directory
            let is_file = output_path
                .extension()
                .map(|e| e == "lua" || e == "md")
                .unwrap_or(false);

            if is_file {
                // Output to specific file
                if let Some(parent) = output_path.parent()
                    && !parent.as_os_str().is_empty()
                {
                    std::fs::create_dir_all(parent)?;
                }
                let content = if output_path.extension().map(|e| e == "lua").unwrap_or(false) {
                    rs_web::lua::generate_emmylua()
                } else {
                    rs_web::lua::generate_markdown()
                };
                std::fs::write(&output_path, content)?;
                println!("Generated {}", output_path.display());
            } else {
                // Output to directory
                std::fs::create_dir_all(&output_path)?;

                if generate_lua {
                    let content = rs_web::lua::generate_emmylua();
                    let path = output_path.join("rs-web.lua");
                    std::fs::write(&path, content)?;
                    println!("Generated {}", path.display());
                }

                if generate_markdown {
                    let content = rs_web::lua::generate_markdown();
                    let path = output_path.join("LUA_API.md");
                    std::fs::write(&path, content)?;
                    println!("Generated {}", path.display());
                }
            }
        }
        Commands::Serve {
            directory,
            output,
            port,
            host,
            watch,
            no_build,
        } => {
            // Determine project directory
            let project_dir = directory.unwrap_or_else(|| PathBuf::from("."));
            let project_dir = project_dir.canonicalize().unwrap_or(project_dir);

            // Load config from project directory
            let mut config = Config::load(&project_dir)?;

            // Allow overriding base_url via environment variable
            if let Ok(base_url) = std::env::var("SITE_BASE_URL") {
                log::info!("Using base_url from SITE_BASE_URL: {}", base_url);
                config.site.base_url = base_url;
            }

            // Output directory
            let output_dir = if let Some(out) = output {
                if out.is_absolute() {
                    out
                } else {
                    std::env::current_dir()?.join(out)
                }
            } else {
                project_dir.join(&config.build.output_dir)
            };

            // Build if needed
            if !no_build {
                let start = Instant::now();
                let mut builder = Builder::new(config, output_dir.clone(), project_dir.clone());
                builder.build()?;
                println!("Built in {:?}", start.elapsed());
                // Reload config for server
                config = Config::load(&project_dir)?;
            }

            // Start the server
            let server_config = ServerConfig {
                port,
                host: host.clone(),
                output_dir: output_dir.clone(),
            };

            let reload_tx = run_server(server_config).await?;

            // Watch mode with live reload
            if watch {
                run_serve_watch_loop(config, &project_dir, &output_dir, reload_tx)?;
            } else {
                // Just keep running
                println!("Press Ctrl+C to stop.\n");
                tokio::signal::ctrl_c().await?;
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

/// Run the watch loop with live reload for serve command
fn run_serve_watch_loop(
    config: Config,
    project_dir: &Path,
    output_dir: &Path,
    reload_tx: tokio::sync::broadcast::Sender<ReloadMessage>,
) -> Result<()> {
    println!("Watching for changes. Press Ctrl+C to stop.\n");

    let watcher = FileWatcher::new(project_dir, &config, output_dir)?;
    let mut builder = Builder::new(config, output_dir.to_path_buf(), project_dir.to_path_buf());

    loop {
        let changes = watcher.wait_for_changes()?;

        if changes.is_empty() {
            continue;
        }

        println!("\nChange detected: {}", format_changes(&changes));
        let start = Instant::now();

        // Check if only CSS changed for hot reload
        let css_only = changes.rebuild_css
            && !changes.full_rebuild
            && !changes.reload_templates
            && !changes.rebuild_home
            && changes.content_files.is_empty()
            && changes.static_files.is_empty()
            && changes.image_files.is_empty();

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
                println!("Rebuilt in {:?}", start.elapsed());

                // Send reload notification
                if css_only {
                    notify_reload(&reload_tx, ReloadMessage::CssReload("*".to_string()));
                    println!("CSS hot reloaded\n");
                } else {
                    notify_reload(&reload_tx, ReloadMessage::Reload);
                    println!("Page reloaded\n");
                }
            }
            Err(e) => {
                error!("Build failed: {}", e);
            }
        }
    }
}
