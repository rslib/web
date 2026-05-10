# rs-web - Static Site Generator

A fast, opinionated static site generator built in Rust with support for:

- Markdown processing with syntax highlighting, external link handling
- Content encryption (full post or partial `:::encrypted` blocks)
- HTML content files with Tera templating support
- Link graph with backlinks and visualization (Obsidian-style)
- RSS feed generation with section filtering
- Parallel processing for fast builds
- Live reload with automatic browser refresh during watch mode
- Asset minification for CSS, JS (with dead code elimination), and HTML

## Installation

```bash
cargo install rs-web
```

## Quick Start

```bash
# Build the site
rs-web build

# Build to custom output directory
rs-web build --output public

# Watch for changes and rebuild incrementally with live reload
rs-web build --watch

# Watch mode with custom port
rs-web build --watch --port 8080
```

## Incremental Builds

When using `--watch`, rs-web tracks file dependencies automatically and only rebuilds what's necessary:

- **Dependency tracking**: All file reads/writes in Lua and Tera are tracked automatically
- **Smart rebuilds**: Only rebuilds when tracked dependencies change
- **Stale page cleanup**: Pages that no longer exist (e.g., renamed tags) are automatically deleted
- **Change output**: Shows which files changed during each rebuild
- **Live reload**: Browser automatically refreshes when content changes
- **Memoization**: Transform functions like `render_markdown` cache results
- **Cache persistence**: Dependencies are cached in `.rs-web-cache/deps.bin`

```bash
# Watch mode with smart incremental builds and live reload
rs-web build --watch

# Watch mode with custom port
rs-web build --watch --port 8080

# Clean the cache to force a full rebuild
rm -rf .rs-web-cache
```

## Logging

Control log verbosity with `--debug`, `--log-level`, or the `RS_WEB_LOG_LEVEL` environment variable.

```bash
# Enable debug logging (shorthand)
rs-web --debug build

# Set specific log level (trace, debug, info, warning, error)
rs-web --log-level trace build

# Use environment variable
RS_WEB_LOG_LEVEL=debug rs-web build
```

**Priority order:** `--debug` > `--log-level` > `RS_WEB_LOG_LEVEL` > default (`warning`)

## Configuration

Configure via `config.lua`:

```lua
return {
  site = {
    title = "My Site",
    description = "Site description",
    base_url = "https://example.com",
    author = "Your Name",
  },

  build = {
    output_dir = "dist",
  },

  -- Generate pages via Lua
  pages = function()
    return {
      { path = "/", template = "home.html", title = "Home" },
      { path = "/about/", template = "page.html", title = "About", minify = true },
    }
  end,

  -- Build hooks
  hooks = {
    before_build = function()
      print("Starting build...")
    end,
    after_build = function()
      print("Build complete!")
    end,
  },
}
```

#### Lua Sandbox

By default, Lua file operations are sandboxed to the project directory (where `config.lua` is located). This prevents accidental or malicious access to files outside your project.

```lua
return {
  -- Sandbox is enabled by default. Set to false to allow access outside project directory.
  lua = {
    sandbox = false,  -- Disable sandbox (use with caution)
  },

  site = { ... },
}
```

When sandbox is enabled:
- File operations (`read_file`, `write_file`, `load_json`, etc.) only work within the project directory
- Attempting to access files outside returns an error with a helpful message
- Relative paths are resolved from the project root

#### Lua API Functions

The `rs-web` module must be explicitly required:

```lua
local rs = require("rs-web")
```

Available functions:

**File System (rs.fs):**

| Function | Description |
|----------|-------------|
| `rs.fs.read(path)` | Read file contents, returns nil if not found |
| `rs.fs.write(path, content)` | Write content to file, returns true/false |
| `rs.fs.copy(src, dest)` | Copy file (binary-safe), returns true/false |
| `rs.fs.exists(path)` | Check if file exists |
| `rs.fs.list(path, pattern?)` | List files matching glob pattern |
| `rs.fs.list_dirs(path)` | List subdirectories |
| `rs.fs.glob(pattern)` | Find files matching glob pattern |
| `rs.fs.scan(dir, pattern?)` | Scan directory recursively |

Parallel file operations:
| Function | Description |
|----------|-------------|
| `rs.fs.par.read(paths)` | Read multiple files in parallel |
| `rs.fs.par.exists(paths)` | Check multiple files exist in parallel |
| `rs.fs.par.copy(sources, dests)` | Copy multiple files in parallel |
| `rs.fs.par.create_dirs(paths)` | Create directories in parallel |

**Data Loading (rs.data):**

| Function | Description |
|----------|-------------|
| `rs.data.load_json(path)` | Load and parse JSON file |
| `rs.data.load_yaml(path)` | Load and parse YAML file |
| `rs.data.load_toml(path)` | Load and parse TOML file |
| `rs.data.load_frontmatter(path)` | Extract frontmatter and content from markdown |
| `rs.data.from_json(str)` | Parse JSON string to Lua value |
| `rs.data.to_json(value, pretty?)` | Serialize Lua value to JSON string |
| `rs.data.from_yaml(str)` | Parse YAML string to Lua value |
| `rs.data.to_yaml(value)` | Serialize Lua value to YAML string |
| `rs.data.from_toml(str)` | Parse TOML string to Lua value |
| `rs.data.to_toml(value)` | Serialize Lua value to TOML string |

Parallel data operations:
| Function | Description |
|----------|-------------|
| `rs.data.par.load_json(paths)` | Load multiple JSON files in parallel |
| `rs.data.par.load_yaml(paths)` | Load multiple YAML files in parallel |
| `rs.data.par.load_frontmatter(paths)` | Parse frontmatter from multiple files in parallel |

**HTML Processing (rs.html):**

| Function | Description |
|----------|-------------|
| `rs.html.to_text(html)` | Convert HTML to plain text |
| `rs.html.strip_tags(html)` | Remove HTML tags |
| `rs.html.extract_links(html)` | Extract all links from HTML |
| `rs.html.extract_images(html)` | Extract all image sources from HTML |

**Date & Time (rs.date):**

All date functions accept: Unix timestamp (number), date string, or table `{year, month, day, hour?, min?, sec?}`

| Function | Description |
|----------|-------------|
| `rs.date.now()` | Get current Unix timestamp |
| `rs.date.from_timestamp(ts)` | Convert Unix timestamp to DateTime table |
| `rs.date.to_timestamp(date)` | Convert date to Unix timestamp |
| `rs.date.format(date, format)` | Format date using strftime format |
| `rs.date.parse(str, format?)` | Parse date string to DateTime table (auto-detects or custom format) |
| `rs.date.rss_format(date)` | Format date for RSS (RFC 2822) |
| `rs.date.iso_format(date)` | Format date as ISO 8601 |
| `rs.date.add(date, delta)` | Add time: `{years?, months?, days?, hours?, mins?, secs?}` |
| `rs.date.diff(date1, date2)` | Get difference in seconds |

DateTime table: `{year, month, day, hour, min, sec, weekday, yday}`

**Markdown (rs.markdown):**

| Function | Description |
|----------|-------------|
| `rs.markdown.render(content, opts?)` | Render markdown to HTML with optional plugins |
| `rs.markdown.plugins(...)` | Combine/flatten plugins into array |
| `rs.markdown.plugins.default(opts?)` | Get default plugins (lazy_images, heading_anchors, external_links) |
| `rs.markdown.plugins.lazy_images(opts?)` | Plugin: add `loading="lazy" decoding="async"` to images |
| `rs.markdown.plugins.heading_anchors(opts?)` | Plugin: add `id="slug"` to headings |
| `rs.markdown.plugins.external_links(opts?)` | Plugin: add `target="_blank" rel="noopener"` to external links |

Example:
```lua
-- Simple (uses default plugins)
local html = rs.markdown.render(content)

-- With custom plugins
local html = rs.markdown.render(content, {
  plugins = rs.markdown.plugins(
    rs.markdown.plugins.default({ lazy_images = false }),
    my_custom_plugin()
  ),
})

-- Custom plugin example
local function highlight_plugin()
  return function(ast)
    local new_ast = {}
    for _, event in ipairs(ast) do
      -- Transform events here
      table.insert(new_ast, event)
    end
    return new_ast
  end
end
```

**Image Processing (rs.image):**

| Function | Description |
|----------|-------------|
| `rs.image.dimensions(path)` | Get image width and height |
| `rs.image.resize(input, output, options)` | Resize image (options: width, height?, quality?) |
| `rs.image.convert(input, output, options?)` | Convert image format (options: format?, quality?) |
| `rs.image.optimize(input, output, options?)` | Optimize/compress image (options: quality?) |

Parallel image operations:
| Function | Description |
|----------|-------------|
| `rs.image.par.resize(inputs, outputs, options?)` | Resize multiple images in parallel |
| `rs.image.par.convert(inputs, outputs, options?)` | Convert multiple images in parallel |
| `rs.image.par.optimize(inputs, outputs, options?)` | Optimize multiple images in parallel |

**Fonts (rs.fonts):**

| Function | Description |
|----------|-------------|
| `rs.fonts.download_google_font(family, options)` | Download Google Font files and generate local CSS (async) |

**JS Module (rs.js):**

| Function | Description |
|----------|-------------|
| `rs.js.concat(paths, output, options?)` | Concatenate JS files with optional minification (async) |
| `rs.js.bundle(entry, output, options?)` | Bundle JS with imports via Rolldown (async) |
| `rs.js.bundle_many(entries, output_dir, options?)` | Bundle multiple JS entries to separate files (async) |

**CSS Module (rs.css):**

| Function | Description |
|----------|-------------|
| `rs.css.concat(paths, output, options?)` | Concatenate CSS files with optional minification (async) |
| `rs.css.bundle(paths, output, options?)` | Bundle CSS with @import resolution via LightningCSS (async) |
| `rs.css.bundle_many(paths, output_dir, options?)` | Bundle multiple CSS entries to separate files (async) |
| `rs.css.purge(css_path, options?)` | Remove unused CSS based on HTML/JS output (async, call in after_build) |
| `rs.css.critical(html, css_path, options?)` | Extract critical CSS for a specific HTML page (async, returns string) |
| `rs.css.inline_critical(html_path, css_path, options?)` | Inline critical CSS into HTML with async loading for full CSS (async) |

CSS options: `minify` (bool), `purge` (bool, default false), `safelist` (string[] regex patterns to always keep)
Critical options: `minify` (bool), `safelist` (string[]), `css_href` (string, URL for async loading)

**Asset Hashing (rs.assets):**

| Function | Description |
|----------|-------------|
| `rs.assets.hash(content, length?)` | Compute SHA256 hash of content (async, returns handle) |
| `rs.assets.hash_sync(content, length?)` | Compute hash synchronously |
| `rs.assets.write_hashed(content, path, options?)` | Write file with content-hashed filename (async) |
| `rs.assets.register(original, hashed)` | Manually register original → hashed path mapping |
| `rs.assets.get_path(path)` | Get hashed path for original (or original if not found) |
| `rs.assets.manifest()` | Get table of all path mappings |
| `rs.assets.clear()` | Clear the manifest |

**PWA (rs.pwa):**

| Function | Description |
|----------|-------------|
| `rs.pwa.manifest(options)` | Generate web app manifest.json (async) |
| `rs.pwa.service_worker(options)` | Generate service worker sw.js with caching strategies (async) |

**SEO (rs.seo):**

| Function | Description |
|----------|-------------|
| `rs.seo.sitemap(options)` | Generate XML sitemap (async) |
| `rs.seo.robots(options)` | Generate robots.txt (async) |

**Tera Filter:**

Use `| asset` in templates to resolve hashed asset paths:

```html
<link rel="stylesheet" href="{{ '/styles/main.css' | asset }}">
<script src="{{ '/js/editor.js' | asset }}"></script>
```

**Text Processing (rs.text):**

| Function | Description |
|----------|-------------|
| `rs.text.slugify(text)` | Convert text to URL-friendly slug |
| `rs.text.word_count(text)` | Count words in text |
| `rs.text.reading_time(text, wpm?)` | Calculate reading time in minutes |
| `rs.text.truncate(text, len, suffix?)` | Truncate text with optional suffix |
| `rs.text.url_encode(str)` | URL encode a string |
| `rs.text.url_decode(str)` | URL decode a string |

**Path Utilities (rs.path):**

| Function | Description |
|----------|-------------|
| `rs.path.join(...)` | Join path segments |
| `rs.path.basename(path)` | Get file name from path |
| `rs.path.dirname(path)` | Get directory from path |
| `rs.path.extension(path)` | Get file extension |

**Hash Functions (rs.hash):**

| Function | Description |
|----------|-------------|
| `rs.hash.content(content)` | Hash content (xxHash64) |
| `rs.hash.file(path)` | Hash file contents |

**Collection Operations (rs.ops):**

| Function | Description |
|----------|-------------|
| `rs.ops.filter(items, fn)` | Filter items where fn returns true |
| `rs.ops.sort(items, fn)` | Sort items using comparator |
| `rs.ops.map(items, fn)` | Transform each item |
| `rs.ops.find(items, fn)` | Find first item where fn returns true |
| `rs.ops.group_by(items, key_fn)` | Group items by key |
| `rs.ops.unique(items)` | Remove duplicates |
| `rs.ops.reverse(items)` | Reverse array order |
| `rs.ops.take(items, n)` | Take first n items |
| `rs.ops.skip(items, n)` | Skip first n items |
| `rs.ops.keys(table)` | Get all keys from a table |
| `rs.ops.values(table)` | Get all values from a table |
| `rs.ops.reduce(items, init, fn)` | Reduce items to single value |

Parallel collection operations:
| Function | Description |
|----------|-------------|
| `rs.ops.par.map(items, fn, ctx?)` | Transform items in parallel (pass context explicitly) |
| `rs.ops.par.filter(items, fn, ctx?)` | Filter items in parallel (pass context explicitly) |

**Environment (rs.env):**

| Function | Description |
|----------|-------------|
| `rs.env.get(name, default?)` | Get environment variable with optional default |

**Logging (rs.log):**

| Function | Description |
|----------|-------------|
| `rs.log.trace(...)` | Log at trace level |
| `rs.log.debug(...)` | Log at debug level |
| `rs.log.info(...)` | Log at info level |
| `rs.log.warn(...)` | Log at warn level |
| `rs.log.error(...)` | Log at error level |
| `rs.log.print(...)` | Print to output |

**Git Information (rs.git):**

| Function | Description |
|----------|-------------|
| `rs.git.info(path?)` | Get git info for repo or file (hash, branch, author, timestamp, dirty) |
| `rs.git.is_ignored(path)` | Check if path is gitignored |

**Note:** All file operations respect the sandbox setting and are tracked for incremental builds. Paths can be relative (resolved from project root) or absolute.

#### Coroutine Helpers

Coroutine-based cooperative multitasking with a cleaner API:

```lua
local rs = require("rs-web")

-- Create and run tasks
local task1 = rs.coro.task(function()
  local data = rs.data.load_json("file1.json")
  rs.coro.yield()  -- cooperative yield
  return data
end)

-- Run single task to completion
local result = rs.coro.await(task1)

-- Run multiple tasks (interleaved execution)
local results = rs.coro.all({task1, task2, task3})

-- Race: return first completed
local winner, index = rs.coro.race({task1, task2})
```

#### Parallel Processing

True parallel execution using Rust's rayon thread pool. Each module has a `.par` submodule:

```lua
local rs = require("rs-web")

-- Load multiple JSON files in parallel (I/O parallelism)
local configs = rs.data.par.load_json({
  "content/problems/two-sum/config.json",
  "content/problems/reverse-string/config.json",
})

-- Load multiple YAML files in parallel
local data = rs.data.par.load_yaml({"a.yaml", "b.yaml", "c.yaml"})

-- Read multiple files in parallel
local contents = rs.fs.par.read({"a.txt", "b.txt", "c.txt"})

-- Parse frontmatter from multiple files in parallel
local posts = rs.data.par.load_frontmatter({
  "content/blog/post1.md",
  "content/blog/post2.md",
})
-- Returns: { { frontmatter = {...}, content = "...", raw = "..." }, ... }

-- Check multiple files exist in parallel
local exists = rs.fs.par.exists({"a.txt", "b.txt"})

-- Create directories in parallel
rs.fs.par.create_dirs({"dist/a", "dist/b", "dist/c"})

-- Copy files in parallel (sources, destinations)
rs.fs.par.copy(
  {"src/a.txt", "src/b.txt"},
  {"dist/a.txt", "dist/b.txt"}
)

-- Convert images in parallel
rs.image.par.convert(
  {"img/a.jpg", "img/b.jpg"},
  {"dist/a.webp", "dist/b.webp"},
  { quality = 85 }
)

-- TRUE PARALLEL map/filter (runs on rayon thread pool)
-- IMPORTANT: Upvalues are NOT captured. Pass context explicitly as 3rd argument.
local multiplier = 10
local doubled = rs.ops.par.map(items, function(x, ctx)
  return x * ctx.multiplier
end, { multiplier = multiplier })

local threshold = 5
local above = rs.ops.par.filter(items, function(x, ctx)
  return x > ctx.threshold
end, { threshold = threshold })

-- Simple functions without upvalues work directly
local squared = rs.ops.par.map(items, function(x) return x * x end)
local evens = rs.ops.par.filter(items, function(x) return x % 2 == 0 end)

-- Sequential operations (for non-serializable items)
local result = rs.ops.map(items, function(x) return x.name end)
local filtered = rs.ops.filter(items, function(x) return x.active end)

-- Reduce (sequential)
local sum = rs.ops.reduce(items, 0, function(acc, x) return acc + x end)
```

#### Async I/O (Tokio)

True async I/O backed by Tokio. All functions return handles - await with `rs.async.await()`:

```lua
-- Spawn concurrent fetches
local t1 = rs.async.fetch("https://api.example.com/a", { cache = true })
local t2 = rs.async.fetch_bytes("https://fonts.example.com/font.woff2", { cache = true })
local t3 = rs.fonts.download_google_font("Lexend", { fonts_dir = "dist/fonts", css_path = "dist/fonts.css" })

-- Await all together
local results = rs.async.await_all({t1, t2, t3})

-- File operations (also return handles)
local write_task = rs.async.write_file("output.txt", "content")
local copy_task = rs.async.copy_file("src.txt", "dst.txt")
local dir_task = rs.async.create_dir("new/nested/dir")
rs.async.await_all({write_task, copy_task, dir_task})

-- Blocking fetch (returns response directly, no await needed)
local response = rs.async.fetch_sync("https://api.example.com/data")
```

### Configuration Sections

| Section | Key Settings |
|---------|--------------|
| `site` | title, description, base_url, author (required) |
| `seo` | twitter_handle, default_og_image |
| `build` | output_dir |
| `paths` | templates |

## Frontmatter

Post frontmatter options (YAML or TOML):

```yaml
---
title: "Post Title"           # Required
description: "Description"    # Optional
date: 2024-01-15              # Optional (YAML date or string)
tags: ["tag1", "tag2"]        # Optional
draft: false                  # Optional (default: false, excluded from build)
image: "/static/post.png"     # Optional: OG image
template: "custom.html"       # Optional: Override template
slug: "custom-slug"           # Optional: Override URL slug
permalink: "/custom/url/"     # Optional: Full URL override
encrypted: false              # Optional: Encrypt entire post
password: "post-secret"       # Optional: Post-specific password
---
```

## Content Types

### Markdown Files (.md)

Standard markdown files processed through the markdown pipeline.

### HTML Files (.html)

HTML files with Tera templating support. Can use extends, includes, and all Tera features:

```html
+++
title = "Custom Page"
date = 2024-01-15
+++
{% extends "base.html" %}

{% block content %}
<div class="custom">
  <h1>{{ page.data.post.title }}</h1>
  <p>By {{ site.author }}</p>
</div>
{% endblock %}
```

## Partial Encryption

### Markdown

Use `:::encrypted` blocks for partial content encryption:

```markdown
Public content here.

:::encrypted
This content is encrypted with the global/post password.
:::

:::encrypted password="custom"
This block has its own password.
:::
```

### HTML

Use `<encrypted>` tags in HTML files:

```html
<p>Public content here.</p>

<encrypted>
  <p>This content is encrypted.</p>
</encrypted>

<encrypted password="custom">
  <p>This block has its own password.</p>
</encrypted>
```

## Template Variables

### Home Template (home.html)

- `site` - Site config (title, description, base_url, author)
- `page` - Page info (title, description, url, image)
- `sections` - All sections with posts (`sections.blog.posts`)
- `content` - Rendered markdown content

### Post Template (post.html)

- `site` - Site config
- `post` - Post info (title, url, date, tags, reading_time, etc.)
- `page` - Page info for head.html compatibility
- `content` - Rendered markdown content
- `backlinks` - Posts linking to this post (url, title, section)
- `graph` - Local graph data (nodes, edges) for visualization

### Graph Template (graph.html)

- `site` - Site config
- `page` - Page info
- `graph` - Full graph data (nodes, edges)

## License

MIT License. See [LICENSE](LICENSE) for details.
