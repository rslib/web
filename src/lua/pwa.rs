//! PWA (Progressive Web App) module (rs.pwa)
//!
//! Provides:
//! - rs.pwa.manifest(options) - Generate a web app manifest
//! - rs.pwa.service_worker(options) - Generate a service worker for offline caching

use crate::lua::async_io::{AsyncIOTask, runtime};
use crate::tracker::SharedTracker;
use mlua::{Lua, Result, Table};
use std::path::{Path, PathBuf};

/// Create the rs.pwa module table
pub fn create_module(lua: &Lua, project_root: &Path, tracker: SharedTracker) -> Result<Table> {
    let pwa_module = lua.create_table()?;
    let root = project_root.to_path_buf();

    // rs.pwa.manifest(options) - Generate web app manifest (async)
    // options: { name, short_name, description?, start_url?, display?, background_color?, theme_color?, icons?, output }
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let manifest_fn = lua.create_function(move |lua, options: Table| {
        let opts = ManifestOptions::from_lua(&options, lua)?;
        let output = resolve_output_path(&opts.output, &root_clone);
        let tracker = tracker_clone.clone();

        let handle = runtime().spawn(async move {
            let manifest = generate_manifest(&opts);
            write_output_async(&output, &manifest, &tracker).await
        });

        Ok(AsyncIOTask::new(handle))
    })?;
    pwa_module.set("manifest", manifest_fn)?;

    // rs.pwa.service_worker(options) - Generate service worker (async)
    // options: { cache_name, assets?, precache?, network_first?, cache_first?, output }
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let sw_fn = lua.create_function(move |lua, options: Table| {
        let opts = ServiceWorkerOptions::from_lua(&options, lua)?;
        let output = resolve_output_path(&opts.output, &root_clone);
        let tracker = tracker_clone.clone();

        let handle = runtime().spawn(async move {
            let sw = generate_service_worker(&opts);
            write_output_async(&output, &sw, &tracker).await
        });

        Ok(AsyncIOTask::new(handle))
    })?;
    pwa_module.set("service_worker", sw_fn)?;

    Ok(pwa_module)
}

/// Options for web app manifest generation
#[derive(Debug, Clone)]
struct ManifestOptions {
    name: String,
    short_name: String,
    description: Option<String>,
    start_url: String,
    display: String,
    background_color: String,
    theme_color: String,
    icons: Vec<ManifestIcon>,
    output: String,
}

#[derive(Debug, Clone)]
struct ManifestIcon {
    src: String,
    sizes: String,
    icon_type: String,
    purpose: Option<String>,
}

impl ManifestOptions {
    fn from_lua(options: &Table, _lua: &Lua) -> mlua::Result<Self> {
        let name: String = options
            .get("name")
            .map_err(|_| mlua::Error::external("pwa.manifest: 'name' is required"))?;

        let short_name: String = options
            .get("short_name")
            .map_err(|_| mlua::Error::external("pwa.manifest: 'short_name' is required"))?;

        let output: String = options
            .get("output")
            .map_err(|_| mlua::Error::external("pwa.manifest: 'output' is required"))?;

        let description: Option<String> = options.get("description").ok();
        let start_url: String = options.get("start_url").unwrap_or_else(|_| "/".to_string());
        let display: String = options
            .get("display")
            .unwrap_or_else(|_| "standalone".to_string());
        let background_color: String = options
            .get("background_color")
            .unwrap_or_else(|_| "#ffffff".to_string());
        let theme_color: String = options
            .get("theme_color")
            .unwrap_or_else(|_| "#000000".to_string());

        // Parse icons array
        let mut icons = Vec::new();
        if let Ok(icons_table) = options.get::<Table>("icons") {
            for (_, icon_table) in icons_table.pairs::<i64, Table>().flatten() {
                let src: String = icon_table.get("src").unwrap_or_default();
                let sizes: String = icon_table.get("sizes").unwrap_or_default();
                let icon_type: String = icon_table
                    .get("type")
                    .unwrap_or_else(|_| "image/png".to_string());
                let purpose: Option<String> = icon_table.get("purpose").ok();

                if !src.is_empty() && !sizes.is_empty() {
                    icons.push(ManifestIcon {
                        src,
                        sizes,
                        icon_type,
                        purpose,
                    });
                }
            }
        }

        Ok(Self {
            name,
            short_name,
            description,
            start_url,
            display,
            background_color,
            theme_color,
            icons,
            output,
        })
    }
}

/// Generate the manifest.json content
fn generate_manifest(opts: &ManifestOptions) -> String {
    let mut manifest = serde_json::json!({
        "name": opts.name,
        "short_name": opts.short_name,
        "start_url": opts.start_url,
        "display": opts.display,
        "background_color": opts.background_color,
        "theme_color": opts.theme_color,
    });

    if let Some(desc) = &opts.description {
        manifest["description"] = serde_json::json!(desc);
    }

    if !opts.icons.is_empty() {
        let icons: Vec<serde_json::Value> = opts
            .icons
            .iter()
            .map(|icon| {
                let mut obj = serde_json::json!({
                    "src": icon.src,
                    "sizes": icon.sizes,
                    "type": icon.icon_type,
                });
                if let Some(purpose) = &icon.purpose {
                    obj["purpose"] = serde_json::json!(purpose);
                }
                obj
            })
            .collect();
        manifest["icons"] = serde_json::json!(icons);
    }

    serde_json::to_string_pretty(&manifest).unwrap_or_else(|_| "{}".to_string())
}

/// Options for service worker generation
#[derive(Debug, Clone)]
struct ServiceWorkerOptions {
    cache_name: String,
    version: String,
    precache: Vec<String>,
    network_first: Vec<String>,
    cache_first: Vec<String>,
    offline_page: Option<String>,
    output: String,
}

impl ServiceWorkerOptions {
    fn from_lua(options: &Table, _lua: &Lua) -> mlua::Result<Self> {
        let cache_name: String = options
            .get("cache_name")
            .map_err(|_| mlua::Error::external("pwa.service_worker: 'cache_name' is required"))?;

        let output: String = options
            .get("output")
            .map_err(|_| mlua::Error::external("pwa.service_worker: 'output' is required"))?;

        let version: String = options.get("version").unwrap_or_else(|_| "1".to_string());

        let precache = parse_string_array(options, "precache");
        let network_first = parse_string_array(options, "network_first");
        let cache_first = parse_string_array(options, "cache_first");
        let offline_page: Option<String> = options.get("offline_page").ok();

        Ok(Self {
            cache_name,
            version,
            precache,
            network_first,
            cache_first,
            offline_page,
            output,
        })
    }
}

fn parse_string_array(options: &Table, key: &str) -> Vec<String> {
    options
        .get::<Table>(key)
        .ok()
        .map(|t| {
            t.pairs::<i64, String>()
                .filter_map(|p| p.ok())
                .map(|(_, v)| v)
                .collect()
        })
        .unwrap_or_default()
}

/// Generate the service worker JavaScript content
fn generate_service_worker(opts: &ServiceWorkerOptions) -> String {
    let cache_name = format!("{}-v{}", opts.cache_name, opts.version);
    let precache_json = serde_json::to_string(&opts.precache).unwrap_or_else(|_| "[]".to_string());
    let network_first_json =
        serde_json::to_string(&opts.network_first).unwrap_or_else(|_| "[]".to_string());
    let cache_first_json =
        serde_json::to_string(&opts.cache_first).unwrap_or_else(|_| "[]".to_string());
    let offline_page_json = opts
        .offline_page
        .as_ref()
        .map(|p| format!("'{}'", p))
        .unwrap_or_else(|| "null".to_string());

    format!(
        r#"// Service Worker - Generated by rs-web
const CACHE_NAME = '{cache_name}';
const PRECACHE_URLS = {precache_json};
const NETWORK_FIRST_PATTERNS = {network_first_json};
const CACHE_FIRST_PATTERNS = {cache_first_json};
const OFFLINE_PAGE = {offline_page_json};

// Install: precache static assets and offline page
self.addEventListener('install', (event) => {{
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => {{
      const urls = [...PRECACHE_URLS];
      if (OFFLINE_PAGE && !urls.includes(OFFLINE_PAGE)) {{
        urls.push(OFFLINE_PAGE);
      }}
      return cache.addAll(urls);
    }}).then(() => self.skipWaiting())
  );
}});

// Activate: clean old caches
self.addEventListener('activate', (event) => {{
  event.waitUntil(
    caches.keys().then((cacheNames) => {{
      return Promise.all(
        cacheNames
          .filter((name) => name !== CACHE_NAME)
          .map((name) => caches.delete(name))
      );
    }}).then(() => self.clients.claim())
  );
}});

// Helper: check if URL matches any pattern
function matchesPattern(url, patterns) {{
  return patterns.some((pattern) => {{
    if (pattern.startsWith('/') && pattern.endsWith('/')) {{
      return new RegExp(pattern.slice(1, -1)).test(url);
    }}
    return url.includes(pattern);
  }});
}}

// Helper: check if request is for HTML
function isHtmlRequest(request) {{
  const accept = request.headers.get('Accept') || '';
  return request.mode === 'navigate' || accept.includes('text/html');
}}

// Fetch: apply caching strategies
self.addEventListener('fetch', (event) => {{
  const url = event.request.url;

  // Network-first strategy (HTML pages, API calls)
  if (matchesPattern(url, NETWORK_FIRST_PATTERNS)) {{
    event.respondWith(
      fetch(event.request)
        .then((response) => {{
          if (response.ok) {{
            const clone = response.clone();
            caches.open(CACHE_NAME).then((cache) => cache.put(event.request, clone));
          }}
          return response;
        }})
        .catch(() => {{
          return caches.match(event.request).then((cached) => {{
            if (cached) return cached;
            if (OFFLINE_PAGE && isHtmlRequest(event.request)) {{
              return caches.match(OFFLINE_PAGE);
            }}
            return cached;
          }});
        }})
    );
    return;
  }}

  // Cache-first strategy (static assets)
  if (matchesPattern(url, CACHE_FIRST_PATTERNS)) {{
    event.respondWith(
      caches.match(event.request).then((cached) => {{
        if (cached) return cached;
        return fetch(event.request).then((response) => {{
          if (response.ok) {{
            const clone = response.clone();
            caches.open(CACHE_NAME).then((cache) => cache.put(event.request, clone));
          }}
          return response;
        }});
      }})
    );
    return;
  }}

  // Default: network with cache fallback, offline page for HTML
  event.respondWith(
    fetch(event.request)
      .catch(() => {{
        return caches.match(event.request).then((cached) => {{
          if (cached) return cached;
          if (OFFLINE_PAGE && isHtmlRequest(event.request)) {{
            return caches.match(OFFLINE_PAGE);
          }}
          return cached;
        }});
      }})
  );
}});
"#
    )
}

fn resolve_output_path(output_path: &str, root: &Path) -> PathBuf {
    if Path::new(output_path).is_absolute() {
        PathBuf::from(output_path)
    } else {
        root.join(output_path)
    }
}

async fn write_output_async(
    output: &Path,
    content: &str,
    tracker: &SharedTracker,
) -> std::result::Result<(), String> {
    if let Some(parent) = output.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    tokio::fs::write(output, content)
        .await
        .map_err(|e| format!("Failed to write: {}", e))?;

    let canonical = output
        .canonicalize()
        .unwrap_or_else(|_| output.to_path_buf());
    tracker.record_write_async(canonical, content.as_bytes());
    Ok(())
}
