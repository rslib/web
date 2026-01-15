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

Configure via `config.lua` (recommended) or `config.toml`:

### Lua Configuration (config.lua)

Lua configuration provides more power with computed data, dynamic pages, and build hooks:

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
    minify_css = true,
  },

  -- Computed data available in templates as {{ computed.tags }}
  computed = {
    tags = function(sections)
      -- Process sections and return data for templates
      return { ... }
    end,
  },

  -- Generate dynamic pages (e.g., /tags/array/, /tags/string/)
  computed_pages = function(sections)
    return {
      { path = "/tags/array/", template = "tag.html", title = "Array", data = {...} },
    }
  end,

  -- Custom Tera filters: {{ value | my_filter }}
  filters = {
    shout = function(s) return s:upper() .. "!" end,
  },

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

| Function | Description |
|----------|-------------|
| `read_file(path)` | Read file contents, returns nil if not found |
| `write_file(path, content)` | Write content to file, returns true/false |
| `file_exists(path)` | Check if file exists |
| `list_files(path, pattern?)` | List files matching glob pattern |
| `list_dirs(path)` | List subdirectories |
| `load_json(path)` | Load and parse JSON file |
| `env(name)` | Get environment variable |
| `print(...)` | Log output to build log |

**Note:** All file operations respect the sandbox setting. Paths can be relative (resolved from project root) or absolute.

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

-- Read multiple files in parallel
local contents = parallel.read_files({"a.txt", "b.txt", "c.txt"})

-- Check multiple files exist in parallel
local exists = parallel.file_exists({"a.txt", "b.txt"})

-- Functional helpers (sequential but convenient)
local doubled = parallel.map(items, function(x) return x * 2 end)
local evens = parallel.filter(items, function(x) return x % 2 == 0 end)
local sum = parallel.reduce(items, 0, function(acc, x) return acc + x end)
```

### TOML Configuration (config.toml)

### Required Settings

```toml
[site]
title = "My Site"                    # Site title
description = "Site description"     # Site description
base_url = "https://example.com"     # Base URL (no trailing slash)
author = "Your Name"                 # Author name

[seo]
twitter_handle = "@username"         # Optional: Twitter handle
default_og_image = "/static/og.png"  # Optional: Default OG image

[build]
output_dir = "dist"                  # Output directory
minify_css = true                    # Default: true

[images]
quality = 85.0                       # WebP quality (default: 85.0)
scale_factor = 1.0                   # Image scale (default: 1.0)
```

### Optional Settings (have defaults)

```toml
[paths]
content = "content"                  # Content directory (default: "content")
styles = "styles"                    # Styles directory (default: "styles")
static_files = "static"              # Static files (default: "static")
templates = "templates"              # Templates (default: "templates")
home = "index.md"                    # Home page file (default: "index.md")
exclude = ["drafts", "^temp.*"]      # Regex patterns to exclude files/dirs (default: [])
exclude_defaults = true              # Exclude README.md, LICENSE.md, etc. (default: true)
respect_gitignore = true             # Respect .gitignore (default: true)

[highlight]
names = ["John Doe", "Jane Doe"]     # Names to highlight (default: [])
class = "me"                         # CSS class for highlights (default: "me")

[templates]
blog = "post.html"                   # Section -> template mapping
projects = "project.html"            # (default: uses {section}.html or post.html)

[permalinks]
blog = "/:year/:month/:slug/"        # Section -> URL pattern
projects = "/:slug/"                 # Placeholders: :year :month :day :slug :title :section

[encryption]
password_command = "pass show site"  # Command to get password (optional)
password = "secret"                  # Raw password (optional, less secure)
                                     # Priority: SITE_PASSWORD env > command > password

[graph]
enabled = true                       # Enable graph generation (default: true)
template = "graph.html"              # Graph page template (default: "graph.html")
path = "graph"                       # URL path (default: "graph" -> /graph/)

[rss]
enabled = true                       # Enable RSS generation (default: true)
filename = "rss.xml"                 # Output filename (default: "rss.xml")
sections = ["blog"]                  # Sections to include (default: [] = all)
limit = 20                           # Max items (default: 20)
exclude_encrypted_blocks = false     # Exclude posts with :::encrypted (default: false)
```

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
