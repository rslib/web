use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use log::{LevelFilter, error};
use std::path::{Path, PathBuf};
use std::time::Instant;

use rs_web::build::Builder;
use rs_web::config::Config;
use rs_web::rs_print;
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
    /// Create a new project
    New {
        /// Project name (creates directory with this name)
        name: String,

        /// Parent directory where project will be created (default: current directory)
        #[arg(short = 'd', long = "dir")]
        directory: Option<PathBuf>,

        /// Initialize in existing directory (will not overwrite existing files)
        #[arg(short, long)]
        force: bool,
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
        // Always show rs_print and lua_print (rs.print) regardless of log level
        .filter_module("rs_print", LevelFilter::Info)
        .filter_module("lua_print", LevelFilter::Info)
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
            rs_print!("Built in {:?}", start.elapsed());

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
                rs_print!("Generated {}", output_path.display());
            } else {
                // Output to directory
                std::fs::create_dir_all(&output_path)?;

                if generate_lua {
                    let content = rs_web::lua::generate_emmylua();
                    let path = output_path.join("rs-web.lua");
                    std::fs::write(&path, content)?;
                    rs_print!("Generated {}", path.display());
                }

                if generate_markdown {
                    let content = rs_web::lua::generate_markdown();
                    let path = output_path.join("LUA_API.md");
                    std::fs::write(&path, content)?;
                    rs_print!("Generated {}", path.display());
                }
            }
        }
        Commands::New {
            name,
            directory,
            force,
        } => {
            // Determine parent directory
            let parent_dir = directory.unwrap_or_else(|| PathBuf::from("."));
            let project_dir = parent_dir.join(&name);

            // Check if directory already exists
            if project_dir.exists() && !force {
                anyhow::bail!(
                    "Directory '{}' already exists. Use --force to initialize anyway.",
                    project_dir.display()
                );
            }

            // Create project directory
            std::fs::create_dir_all(&project_dir)?;
            if !project_dir.exists() || !force {
                log::info!("Created project directory: {}", project_dir.display());
            } else {
                log::info!(
                    "Initializing in existing directory: {}",
                    project_dir.display()
                );
            }

            // Helper to write file only if it doesn't exist (when using --force)
            let write_if_not_exists = |path: &Path, content: &str| -> Result<bool> {
                if path.exists() {
                    log::info!("Skipped {} (already exists)", path.display());
                    Ok(false)
                } else {
                    std::fs::write(path, content)?;
                    Ok(true)
                }
            };

            // Create static/ directory
            let static_dir = project_dir.join("static");
            if !static_dir.exists() {
                std::fs::create_dir_all(&static_dir)?;
                log::info!("Created {}", static_dir.display());
            }

            // Create templates/ directory with base template
            let templates_dir = project_dir.join("templates");
            if !templates_dir.exists() {
                std::fs::create_dir_all(&templates_dir)?;
                log::info!("Created {}", templates_dir.display());
            }

            let base_template = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{% block title %}{{ site.title }}{% endblock %}</title>
</head>
<body>
    {% block content %}{% endblock %}
</body>
</html>
"#;
            if write_if_not_exists(&templates_dir.join("base.html"), base_template)? {
                log::info!("Created {}", templates_dir.join("base.html").display());
            }

            let page_template = r#"{% extends "base.html" %}

{% block title %}{{ page.title }} | {{ site.title }}{% endblock %}

{% block content %}
<article>
    <h1>{{ page.title }}</h1>
    {{ content | safe }}
</article>
{% endblock %}
"#;
            if write_if_not_exists(&templates_dir.join("page.html"), page_template)? {
                log::info!("Created {}", templates_dir.join("page.html").display());
            }

            // Create .types/ directory and generate Lua types (always regenerate)
            let types_dir = project_dir.join(".types");
            std::fs::create_dir_all(&types_dir)?;
            let lua_types = rs_web::lua::generate_emmylua();
            let lua_types_path = types_dir.join("rs-web.lua");
            std::fs::write(&lua_types_path, &lua_types)?;
            log::info!("Created {}", lua_types_path.display());

            // Create site/ directory with index.md
            let site_dir = project_dir.join("site");
            if !site_dir.exists() {
                std::fs::create_dir_all(&site_dir)?;
                log::info!("Created {}", site_dir.display());
            }

            let index_md = r#"---
title: Home
---

Welcome to your new site!
"#;
            if write_if_not_exists(&site_dir.join("index.md"), index_md)? {
                log::info!("Created {}", site_dir.join("index.md").display());
            }

            // Create config.lua with require statement
            let config_content = format!(
                r#"local rs = require("rs-web")

return {{
  site = {{
    title = "{}",
    description = "A new rs-web site",
    base_url = "http://localhost:3000",
    author = "Author",
  }},

  pages = function(ctx)
    local index = rs.read_frontmatter("site/index.md")

    return {{
      {{
        path = "/",
        template = "page.html",
        title = index.title,
        content = rs.render_markdown(index.content),
      }},
    }}
  end,
}}
"#,
                name
            );
            let config_path = project_dir.join("config.lua");
            if write_if_not_exists(&config_path, &config_content)? {
                log::info!("Created {}", config_path.display());
            }

            // Create .luarc.json for LSP
            let luarc_content = r#"{
  "$schema": "https://raw.githubusercontent.com/LuaLS/vscode-lua/master/setting/schema.json",
  "workspace.library": [".types"]
}
"#;
            let luarc_path = project_dir.join(".luarc.json");
            if write_if_not_exists(&luarc_path, luarc_content)? {
                log::info!("Created {}", luarc_path.display());
            }

            // Create .gitignore
            let gitignore_content = r#"# Build output
dist/

# Generated types
.types/
"#;
            let gitignore_path = project_dir.join(".gitignore");
            if write_if_not_exists(&gitignore_path, gitignore_content)? {
                log::info!("Created {}", gitignore_path.display());
            }

            rs_print!("Created project '{}' at {}", name, project_dir.display());
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

            // Create builder
            let mut builder = Builder::new(config, output_dir.clone(), project_dir.clone());

            // Build if needed
            if !no_build {
                let start = Instant::now();
                builder.build()?;
                rs_print!("Built in {:?}", start.elapsed());
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
                run_serve_watch_loop(builder, &project_dir, &output_dir, reload_tx)?;
            } else {
                // Just keep running
                rs_print!("Press Ctrl+C to stop.\n");
                tokio::signal::ctrl_c().await?;
            }
        }
    }

    Ok(())
}

/// Run the watch loop for incremental builds
fn run_watch_loop(mut builder: Builder, project_dir: &Path, output_dir: &Path) -> Result<()> {
    rs_print!("Entering watch mode. Press Ctrl+C to stop.");

    let watcher = FileWatcher::new(project_dir, builder.config(), output_dir)?;

    loop {
        let changes = watcher.wait_for_changes()?;

        if changes.is_empty() {
            continue;
        }

        rs_print!("Change detected: {}", format_changes(&changes));
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
                rs_print!("Rebuilt in {:?}", start.elapsed());
            }
            Err(e) => {
                error!("Build failed: {}", e);
            }
        }
    }
}

/// Run the watch loop with live reload for serve command
fn run_serve_watch_loop(
    mut builder: Builder,
    project_dir: &Path,
    output_dir: &Path,
    reload_tx: tokio::sync::broadcast::Sender<ReloadMessage>,
) -> Result<()> {
    rs_print!("Watching for changes. Press Ctrl+C to stop.");

    let watcher = FileWatcher::new(project_dir, builder.config(), output_dir)?;

    loop {
        let changes = watcher.wait_for_changes()?;

        if changes.is_empty() {
            continue;
        }

        rs_print!("Change detected: {}", format_changes(&changes));
        let start = Instant::now();

        // Check if only CSS changed for hot reload
        let css_only = changes.rebuild_css
            && !changes.full_rebuild
            && !changes.has_asset_changes()
            && !changes.has_template_changes()
            && !changes.rebuild_home
            && changes.content_files.is_empty();

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
                rs_print!("Rebuilt in {:?}", start.elapsed());

                // Send reload notification
                if css_only {
                    notify_reload(&reload_tx, ReloadMessage::CssReload("*".to_string()));
                    rs_print!("CSS hot reloaded");
                } else {
                    notify_reload(&reload_tx, ReloadMessage::Reload);
                    rs_print!("Page reloaded");
                }
            }
            Err(e) => {
                error!("Build failed: {}", e);
            }
        }
    }
}
