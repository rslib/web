# rs-web - Static Site Generator

A fast, opinionated static site generator built in Rust with support for:

- Markdown processing with syntax highlighting, external link handling
- Content encryption (full post or partial `:::encrypted` blocks)
- HTML content files with Tera templating support
- Link graph with backlinks and visualization (Obsidian-style)
- RSS feed generation with section filtering
- Parallel processing for fast builds

## Installation

```bash
cargo install rs-web
```

Or with Nix:

```bash
nix run github:rslib/web#rs-web
```

## Quick Start

```bash
# Build the site
rs-web build

# Build to custom output directory
rs-web build --output public

# Watch for changes and rebuild incrementally
rs-web build --watch
```

## Incremental Builds

When using `--watch`, rs-web tracks file dependencies automatically and only rebuilds what's necessary:

- **Dependency tracking**: All file reads/writes in Lua are tracked automatically
- **Smart rebuilds**: Only rebuilds when tracked dependencies change
- **Memoization**: Transform functions like `render_markdown` cache results
- **Cache persistence**: Dependencies are cached in `.rs-web-cache/deps.bin`

```bash
# Watch mode with smart incremental builds
rs-web build --watch

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
      { path = "/about/", template = "page.html", title = "About" },
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

Available in `config.lua`:

**File Operations:**

| Function | Description |
|----------|-------------|
| `read_file(path)` | Read file contents, returns nil if not found |
| `write_file(path, content)` | Write content to file, returns true/false |
| `copy_file(src, dest)` | Copy file (binary-safe), returns true/false |
| `file_exists(path)` | Check if file exists |
| `list_files(path, pattern?)` | List files matching glob pattern |
| `list_dirs(path)` | List subdirectories |
| `load_json(path)` | Load and parse JSON file |
| `load_yaml(path)` | Load and parse YAML file |
| `load_toml(path)` | Load and parse TOML file |
| `read_frontmatter(path)` | Extract frontmatter and content from markdown |

**Content Processing:**

| Function | Description |
|----------|-------------|
| `render_markdown(content, transform_fn?)` | Convert markdown to HTML with optional transform |
| `html_to_text(html)` | Convert HTML to plain text |
| `rss_date(date_string)` | Format date for RSS (RFC 2822) |

**Image Processing:**

| Function | Description |
|----------|-------------|
| `image_dimensions(path)` | Get image width and height |
| `image_resize(input, output, options)` | Resize image (options: width, height?, quality?) |
| `image_convert(input, output, options?)` | Convert image format (options: format?, quality?) |
| `image_optimize(input, output, options?)` | Optimize/compress image (options: quality?) |

**Asset Building:**

| Function | Description |
|----------|-------------|
| `build_css(pattern, output, options?)` | Build and concatenate CSS files |

**Text Processing:**

| Function | Description |
|----------|-------------|
| `slugify(text)` | Convert text to URL-friendly slug |
| `word_count(text)` | Count words in text |
| `reading_time(text, wpm?)` | Calculate reading time in minutes |
| `truncate(text, len, suffix?)` | Truncate text with optional suffix |
| `strip_tags(html)` | Remove HTML tags |
| `format_date(date, format)` | Format a date string |
| `parse_date(str)` | Parse date string to table {year, month, day} |
| `hash(content)` | Hash content (xxHash64) |
| `hash_file(path)` | Hash file contents |
| `url_encode(str)` | URL encode a string |
| `url_decode(str)` | URL decode a string |

**Path Utilities:**

| Function | Description |
|----------|-------------|
| `join_path(...)` | Join path segments |
| `basename(path)` | Get file name from path |
| `dirname(path)` | Get directory from path |
| `extension(path)` | Get file extension |

**Collections:**

| Function | Description |
|----------|-------------|
| `filter(items, fn)` | Filter items where fn returns true |
| `sort(items, fn)` | Sort items using comparator |
| `map(items, fn)` | Transform each item |
| `find(items, fn)` | Find first item where fn returns true |
| `group_by(items, key_fn)` | Group items by key |
| `unique(items)` | Remove duplicates |
| `reverse(items)` | Reverse array order |
| `take(items, n)` | Take first n items |
| `skip(items, n)` | Skip first n items |
| `keys(table)` | Get all keys from a table |
| `values(table)` | Get all values from a table |

**Environment:**

| Function | Description |
|----------|-------------|
| `env(name)` | Get environment variable |
| `print(...)` | Log output to build log |

**Note:** All file operations respect the sandbox setting and are tracked for incremental builds. Paths can be relative (resolved from project root) or absolute.

#### Async/Await Helpers

Coroutine-based cooperative multitasking with a cleaner API:

```lua
-- Create and run tasks
local task1 = async.task(function()
  local data = load_json("file1.json")
  async.yield()  -- cooperative yield
  return data
end)

-- Run single task to completion
local result = async.await(task1)

-- Run multiple tasks (interleaved execution)
local results = async.all({task1, task2, task3})

-- Race: return first completed
local winner, index = async.race({task1, task2})
```

#### Parallel Processing

True parallel execution using Rust's rayon thread pool:

```lua
-- Load multiple JSON files in parallel (I/O parallelism)
local configs = parallel.load_json({
  "content/problems/two-sum/config.json",
  "content/problems/reverse-string/config.json",
})

-- Load multiple YAML files in parallel
local data = parallel.load_yaml({"a.yaml", "b.yaml", "c.yaml"})

-- Read multiple files in parallel
local contents = parallel.read_files({"a.txt", "b.txt", "c.txt"})

-- Parse frontmatter from multiple files in parallel
local posts = parallel.read_frontmatter({
  "content/blog/post1.md",
  "content/blog/post2.md",
})
-- Returns: { { frontmatter = {...}, content = "...", raw = "..." }, ... }

-- Check multiple files exist in parallel
local exists = parallel.file_exists({"a.txt", "b.txt"})

-- Functional helpers (sequential but convenient)
local doubled = parallel.map(items, function(x) return x * 2 end)
local evens = parallel.filter(items, function(x) return x % 2 == 0 end)
local sum = parallel.reduce(items, 0, function(acc, x) return acc + x end)
```

### Configuration Sections

| Section | Key Settings |
|---------|--------------|
| `site` | title, description, base_url, author (required) |
| `seo` | twitter_handle, default_og_image |
| `build` | output_dir |
| `paths` | styles, static_files, templates |
| `encryption` | password_command or password (SITE_PASSWORD env has priority) |

## Root Pages

Markdown files at the content root (besides the home page) are processed as standalone pages. For example, `404.md` becomes `404.html`:

```yaml
---
title: "404 - Page Not Found"
template: "error.html"
---

# Page Not Found
The page you're looking for doesn't exist.
```

Default excluded files (disable with `exclude_defaults = false`):
- README.md, LICENSE.md, CHANGELOG.md, CONTRIBUTING.md, CODE_OF_CONDUCT.md
- Hidden files (starting with `.`)

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
  <h1>{{ post.title }}</h1>
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
