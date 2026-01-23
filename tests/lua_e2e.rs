//! End-to-end tests for Lua API functions
//!
//! Tests all Lua API functions through a comprehensive config.lua file
//! that exercises the complete rs-web Lua API.

use std::process::Command;
use tempfile::TempDir;

/// Get the path to the rs-web binary
fn get_binary_path() -> String {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test binary name
    path.pop(); // Remove deps
    path.push("rs-web");
    path.to_string_lossy().to_string()
}

/// Setup a test project with the given config.lua content
fn setup_project(config_lua: &str) -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_dir = temp_dir.path().join("test-project");

    // Create directory structure
    std::fs::create_dir_all(project_dir.join("templates")).unwrap();
    std::fs::create_dir_all(project_dir.join("site")).unwrap();
    std::fs::create_dir_all(project_dir.join("static")).unwrap();
    std::fs::create_dir_all(project_dir.join("data")).unwrap();

    // Write config.lua
    std::fs::write(project_dir.join("config.lua"), config_lua).unwrap();

    // Create base template
    let base_template = r#"<!DOCTYPE html>
<html>
<head><title>{% block title %}{{ site.title }}{% endblock %}</title></head>
<body>{% block content %}{% endblock %}</body>
</html>"#;
    std::fs::write(project_dir.join("templates/base.html"), base_template).unwrap();

    // Create page template
    let page_template = r#"{% extends "base.html" %}
{% block title %}{{ page.title }}{% endblock %}
{% block content %}
<article>
<h1>{{ page.title }}</h1>
{{ content | safe }}
{% if page.extra %}
<div class="extra">{{ page.extra | safe }}</div>
{% endif %}
</article>
{% endblock %}"#;
    std::fs::write(project_dir.join("templates/page.html"), page_template).unwrap();

    (temp_dir, project_dir)
}

/// Run build and return (success, stdout, stderr)
fn run_build(project_dir: &std::path::Path) -> (bool, String, String) {
    let output = Command::new(get_binary_path())
        .args(["build", "-d", project_dir.to_str().unwrap()])
        .output()
        .expect("Failed to execute build command");

    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

/// Test rs.fs module functions (read, write, exists, glob, list)
#[test]
fn test_lua_fs_module() {
    let config = r#"local rs = require("rs-web")

return {
  site = {
    title = "FS Test",
    base_url = "http://localhost:3000",
  },

  pages = function(ctx)
    -- Test fs.read
    local content = rs.fs.read("site/test.md")
    assert(content ~= nil, "fs.read should read file")
    assert(content:find("Test Content") ~= nil, "fs.read content should match")

    -- Test fs.exists
    assert(rs.fs.exists("site/test.md") == true, "fs.exists should return true for existing file")
    assert(rs.fs.exists("nonexistent.md") == false, "fs.exists should return false for missing file")

    -- Test fs.glob
    local md_files = rs.fs.glob("site/*.md")
    assert(#md_files >= 1, "fs.glob should find markdown files")
    assert(md_files[1].ext == "md", "fs.glob should return file info with ext")

    -- Test fs.list
    local files = rs.fs.list("site", "*.md")
    assert(#files >= 1, "fs.list should find files")

    -- Test fs.write (write to dist)
    local write_success = rs.fs.write("dist/test-write.txt", "written content")
    assert(write_success, "fs.write should succeed")

    return {
      { path = "/", template = "page.html", title = "FS Test", content = "<p>FS tests passed</p>" },
    }
  end,
}
"#;

    let (_temp, project_dir) = setup_project(config);

    // Create test markdown file
    std::fs::write(
        project_dir.join("site/test.md"),
        "# Test Content\n\nHello World",
    )
    .unwrap();

    let (success, _, stderr) = run_build(&project_dir);
    assert!(success, "Build should succeed: {}", stderr);

    // Verify fs.write worked
    assert!(project_dir.join("dist/test-write.txt").exists());
    assert_eq!(
        std::fs::read_to_string(project_dir.join("dist/test-write.txt")).unwrap(),
        "written content"
    );
}

/// Test rs.data module functions (load_json, load_yaml, load_toml, load_frontmatter, from_json, to_json)
#[test]
fn test_lua_data_module() {
    let config = r#"local rs = require("rs-web")

return {
  site = {
    title = "Data Test",
    base_url = "http://localhost:3000",
  },

  pages = function(ctx)
    -- Test load_json
    local json_data = rs.data.load_json("data/test.json")
    assert(json_data ~= nil, "load_json should load JSON file")
    assert(json_data.name == "test", "load_json should parse correctly")
    assert(json_data.count == 42, "load_json should parse numbers")

    -- Test load_yaml
    local yaml_data = rs.data.load_yaml("data/test.yaml")
    assert(yaml_data ~= nil, "load_yaml should load YAML file")
    assert(yaml_data.title == "YAML Test", "load_yaml should parse correctly")

    -- Test load_toml
    local toml_data = rs.data.load_toml("data/test.toml")
    assert(toml_data ~= nil, "load_toml should load TOML file")
    assert(toml_data.package.name == "test-pkg", "load_toml should parse nested tables")

    -- Test load_frontmatter
    local fm = rs.data.load_frontmatter("site/post.md")
    assert(fm ~= nil, "load_frontmatter should load file")
    assert(fm.title == "My Post", "load_frontmatter should parse title")
    assert(fm.content:find("post content") ~= nil, "load_frontmatter should have content")

    -- Test from_json / to_json
    local obj = { x = 1, y = 2 }
    local json_str = rs.data.to_json(obj)
    local parsed = rs.data.from_json(json_str)
    assert(parsed.x == 1, "from_json/to_json roundtrip should work")

    -- Test from_yaml / to_yaml
    local yaml_str = rs.data.to_yaml(obj)
    local parsed_yaml = rs.data.from_yaml(yaml_str)
    assert(parsed_yaml.x == 1, "from_yaml/to_yaml roundtrip should work")

    -- Test from_toml / to_toml
    local toml_str = rs.data.to_toml(obj)
    local parsed_toml = rs.data.from_toml(toml_str)
    assert(parsed_toml.x == 1, "from_toml/to_toml roundtrip should work")

    return {
      { path = "/", template = "page.html", title = "Data Test", content = "<p>Data tests passed</p>" },
    }
  end,
}
"#;

    let (_temp, project_dir) = setup_project(config);

    // Create test data files
    std::fs::write(
        project_dir.join("data/test.json"),
        r#"{"name": "test", "count": 42}"#,
    )
    .unwrap();

    std::fs::write(
        project_dir.join("data/test.yaml"),
        "title: YAML Test\nvalue: 100",
    )
    .unwrap();

    std::fs::write(
        project_dir.join("data/test.toml"),
        "[package]\nname = \"test-pkg\"",
    )
    .unwrap();

    std::fs::write(
        project_dir.join("site/post.md"),
        "---\ntitle: My Post\ndate: 2024-01-15\n---\n\nThis is the post content.",
    )
    .unwrap();

    let (success, _, stderr) = run_build(&project_dir);
    assert!(success, "Build should succeed: {}", stderr);
}

/// Test rs.text module functions (slugify, word_count, reading_time, truncate, url_encode)
#[test]
fn test_lua_text_module() {
    let config = r#"local rs = require("rs-web")

return {
  site = {
    title = "Text Test",
    base_url = "http://localhost:3000",
  },

  pages = function(ctx)
    -- Test slugify
    local slug = rs.text.slugify("Hello World! This is a Test")
    assert(slug == "hello-world-this-is-a-test", "slugify should work: got " .. slug)

    -- Test word_count
    local count = rs.text.word_count("one two three four five")
    assert(count == 5, "word_count should count words: got " .. count)

    -- Test reading_time
    local time = rs.text.reading_time(string.rep("word ", 400))
    assert(time == 2, "reading_time should calculate minutes: got " .. time)

    -- Test truncate
    local truncated = rs.text.truncate("Hello World", 8)
    assert(truncated == "Hello...", "truncate should work: got " .. truncated)

    -- Test url_encode/decode
    local encoded = rs.text.url_encode("hello world?foo=bar")
    assert(encoded:find("%%20") or encoded:find("+"), "url_encode should encode spaces")
    local decoded = rs.text.url_decode(encoded)
    assert(decoded == "hello world?foo=bar", "url_decode should decode: got " .. decoded)

    return {
      {
        path = "/",
        template = "page.html",
        title = "Text Test",
        content = "<p>Text tests passed</p>",
        extra = "Slug: " .. slug,
      },
    }
  end,
}
"#;

    let (_temp, project_dir) = setup_project(config);

    let (success, _, stderr) = run_build(&project_dir);
    assert!(success, "Build should succeed: {}", stderr);
}

/// Test rs.markdown module (render, extract_links, extract_images)
#[test]
fn test_lua_markdown_module() {
    let config = r##"local rs = require("rs-web")

return {
  site = {
    title = "Markdown Test",
    base_url = "http://localhost:3000",
  },

  pages = function(ctx)
    -- Test markdown.render
    local md = [[# Hello

This is **bold** and *italic*.

- Item 1
- Item 2]]
    local html = rs.markdown.render(md)
    assert(html:find("<h1") ~= nil, "should render headings")
    assert(html:find("<strong>bold</strong>") ~= nil, "should render bold")
    assert(html:find("<em>italic</em>") ~= nil, "should render italic")
    assert(html:find("<ul>") ~= nil, "should render lists")

    -- Test external links get target="_blank"
    local link_md = "Check out [Google](https://google.com)"
    local link_html = rs.markdown.render(link_md)
    assert(link_html:find([[target="_blank"]]) ~= nil, "external links should have target=_blank")

    -- Test extract_links
    local links_md = "Link to [foo](/foo) and [bar](https://bar.com)"
    local links = rs.markdown.extract_links(links_md)
    assert(#links == 2, "should extract 2 links")

    -- Test extract_images
    local img_md = "Image: ![alt text](/images/test.png)"
    local images = rs.markdown.extract_images(img_md)
    assert(#images == 1, "should extract 1 image")
    assert(images[1] == "/images/test.png", "should have correct image path")

    return {
      { path = "/", template = "page.html", title = "Markdown", content = html },
    }
  end,
}
"##;

    let (_temp, project_dir) = setup_project(config);

    let (success, _, stderr) = run_build(&project_dir);
    assert!(success, "Build should succeed: {}", stderr);

    // Verify the output
    let index_html = std::fs::read_to_string(project_dir.join("dist/index.html")).unwrap();
    assert!(index_html.contains("<h1"));
    assert!(index_html.contains("<strong>"));
}

/// Test rs.date module (format, parse, rss_format)
#[test]
fn test_lua_date_module() {
    let config = r#"local rs = require("rs-web")

return {
  site = {
    title = "Date Test",
    base_url = "http://localhost:3000",
  },

  pages = function(ctx)
    -- Test date.format
    local formatted = rs.date.format("2024-01-15", "%B %d, %Y")
    assert(formatted == "January 15, 2024", "date.format should work: got " .. tostring(formatted))

    -- Test date.parse
    local parsed = rs.date.parse("2024-01-15")
    assert(parsed ~= nil, "date.parse should work")
    assert(parsed.year == 2024, "date.parse year should be 2024")
    assert(parsed.month == 1, "date.parse month should be 1")
    assert(parsed.day == 15, "date.parse day should be 15")

    -- Test date.rss_format
    local rss = rs.date.rss_format("2024-01-15")
    assert(rss:find("2024") ~= nil, "rss_format should include year")
    assert(rss:find("Jan") ~= nil, "rss_format should include month abbrev")

    return {
      {
        path = "/",
        template = "page.html",
        title = "Date Test",
        content = "<p>Date: " .. formatted .. "</p>",
      },
    }
  end,
}
"#;

    let (_temp, project_dir) = setup_project(config);

    let (success, _, stderr) = run_build(&project_dir);
    assert!(success, "Build should succeed: {}", stderr);
}

/// Test rs.path module (join, basename, dirname, extension)
#[test]
fn test_lua_path_module() {
    let config = r#"local rs = require("rs-web")

return {
  site = {
    title = "Path Test",
    base_url = "http://localhost:3000",
  },

  pages = function(ctx)
    -- Test path.join
    local joined = rs.path.join("foo", "bar", "baz.txt")
    assert(joined:find("foo") ~= nil, "path.join should include first part")
    assert(joined:find("baz.txt") ~= nil, "path.join should include filename")

    -- Test path.basename
    local base = rs.path.basename("/path/to/file.txt")
    assert(base == "file.txt", "path.basename should return filename: got " .. base)

    -- Test path.dirname
    local dir = rs.path.dirname("/path/to/file.txt")
    assert(dir == "/path/to", "path.dirname should return directory: got " .. dir)

    -- Test path.extension
    local ext = rs.path.extension("/path/to/file.txt")
    assert(ext == "txt", "path.extension should return extension: got " .. ext)

    return {
      { path = "/", template = "page.html", title = "Path Test", content = "<p>Path tests passed</p>" },
    }
  end,
}
"#;

    let (_temp, project_dir) = setup_project(config);

    let (success, _, stderr) = run_build(&project_dir);
    assert!(success, "Build should succeed: {}", stderr);
}

/// Test rs.hash module (content)
#[test]
fn test_lua_hash_module() {
    let config = r#"local rs = require("rs-web")

return {
  site = {
    title = "Hash Test",
    base_url = "http://localhost:3000",
  },

  pages = function(ctx)
    -- Test hash.content
    local hash1 = rs.hash.content("hello world")
    local hash2 = rs.hash.content("hello world")
    local hash3 = rs.hash.content("different content")

    assert(hash1 == hash2, "same content should produce same hash")
    assert(hash1 ~= hash3, "different content should produce different hash")
    assert(#hash1 > 0, "hash should not be empty")

    return {
      { path = "/", template = "page.html", title = "Hash Test", content = "<p>Hash: " .. hash1 .. "</p>" },
    }
  end,
}
"#;

    let (_temp, project_dir) = setup_project(config);

    let (success, _, stderr) = run_build(&project_dir);
    assert!(success, "Build should succeed: {}", stderr);
}

/// Test rs.html module (to_text, strip_tags, extract_links, extract_images)
#[test]
fn test_lua_html_module() {
    let config = r##"local rs = require("rs-web")

return {
  site = {
    title = "HTML Test",
    base_url = "http://localhost:3000",
  },

  pages = function(ctx)
    local html = [[<div><p>Hello World</p><a href="/foo">Link</a></div>]]

    -- Test html.to_text (returns formatted plain text)
    local text = rs.html.to_text(html)
    assert(text:find("Hello") ~= nil, "html.to_text should extract text")
    assert(text:find("<p>") == nil, "html.to_text should not contain tags")

    -- Test html.strip_tags
    local stripped = rs.html.strip_tags(html)
    assert(stripped:find("Hello") ~= nil, "strip_tags should keep text")
    assert(stripped:find("<div>") == nil, "strip_tags should remove tags")

    -- Test html.extract_links
    local links = rs.html.extract_links(html)
    assert(#links >= 1, "extract_links should find links")
    assert(links[1] == "/foo", "extract_links should get href")

    -- Test html.extract_images
    local img_html = [[<img src="/test.png" alt="test">]]
    local images = rs.html.extract_images(img_html)
    assert(#images >= 1, "extract_images should find images")
    assert(images[1] == "/test.png", "extract_images should get src")

    return {
      { path = "/", template = "page.html", title = "HTML Test", content = "<p>HTML tests passed</p>" },
    }
  end,
}
"##;

    let (_temp, project_dir) = setup_project(config);

    let (success, _, stderr) = run_build(&project_dir);
    assert!(success, "Build should succeed: {}", stderr);
}

/// Test rs.ops module (map, filter, sort, find, group_by, unique, reverse, take, skip, reduce)
#[test]
fn test_lua_ops_module() {
    let config = r#"local rs = require("rs-web")

return {
  site = {
    title = "Ops Test",
    base_url = "http://localhost:3000",
  },

  pages = function(ctx)
    local items = {1, 2, 3, 4, 5}

    -- Test ops.map
    local doubled = rs.ops.map(items, function(x) return x * 2 end)
    assert(doubled[1] == 2, "ops.map should transform items")
    assert(doubled[3] == 6, "ops.map should transform all items")

    -- Test ops.filter
    local evens = rs.ops.filter(items, function(x) return x % 2 == 0 end)
    assert(#evens == 2, "ops.filter should filter items")
    assert(evens[1] == 2, "ops.filter should keep matching items")

    -- Test ops.sort
    local unsorted = {3, 1, 4, 1, 5, 9}
    local sorted = rs.ops.sort(unsorted, function(a, b) return a < b end)
    assert(sorted[1] == 1, "ops.sort should sort items")
    assert(sorted[6] == 9, "ops.sort should sort in order")

    -- Test ops.find
    local found = rs.ops.find(items, function(x) return x > 3 end)
    assert(found == 4, "ops.find should find first matching item")

    -- Test ops.group_by
    local words = {{word = "apple", len = 5}, {word = "banana", len = 6}, {word = "cherry", len = 6}}
    local grouped = rs.ops.group_by(words, function(w) return tostring(w.len) end)
    assert(#grouped["5"] == 1, "ops.group_by should group by key")
    assert(#grouped["6"] == 2, "ops.group_by should group multiple items")

    -- Test ops.unique
    local dups = {1, 2, 2, 3, 3, 3}
    local unique = rs.ops.unique(dups)
    assert(#unique == 3, "ops.unique should remove duplicates")

    -- Test ops.reverse
    local reversed = rs.ops.reverse(items)
    assert(reversed[1] == 5, "ops.reverse should reverse array")
    assert(reversed[5] == 1, "ops.reverse should reverse all items")

    -- Test ops.take
    local taken = rs.ops.take(items, 3)
    assert(#taken == 3, "ops.take should take n items")
    assert(taken[3] == 3, "ops.take should take first n items")

    -- Test ops.skip
    local skipped = rs.ops.skip(items, 2)
    assert(#skipped == 3, "ops.skip should skip n items")
    assert(skipped[1] == 3, "ops.skip should skip first n items")

    -- Test ops.reduce
    local sum = rs.ops.reduce(items, 0, function(acc, x) return acc + x end)
    assert(sum == 15, "ops.reduce should reduce to single value")

    -- Test ops.keys and ops.values
    local obj = {a = 1, b = 2, c = 3}
    local keys = rs.ops.keys(obj)
    local values = rs.ops.values(obj)
    assert(#keys == 3, "ops.keys should return all keys")
    assert(#values == 3, "ops.values should return all values")

    return {
      { path = "/", template = "page.html", title = "Ops Test", content = "<p>Ops tests passed</p>" },
    }
  end,
}
"#;

    let (_temp, project_dir) = setup_project(config);

    let (success, _, stderr) = run_build(&project_dir);
    assert!(success, "Build should succeed: {}", stderr);
}

/// Test rs.highlight module (code syntax highlighting)
#[test]
fn test_lua_highlight_module() {
    let config = r#"local rs = require("rs-web")

return {
  site = {
    title = "Highlight Test",
    base_url = "http://localhost:3000",
  },

  pages = function(ctx)
    local code = [[function hello() {
  console.log("Hello, World!");
}]]

    -- Test highlight.highlight_sync (synchronous version)
    local highlighted = rs.highlight.highlight_sync(code, "javascript")
    assert(highlighted:find("<span") ~= nil, "highlighted code should have spans")
    assert(highlighted:find("function") ~= nil, "highlighted code should contain code text")

    -- Test highlight.syntaxes
    local syntaxes = rs.highlight.syntaxes()
    assert(#syntaxes > 0, "should have syntaxes")

    -- Test highlight.themes
    local themes = rs.highlight.themes()
    assert(#themes > 0, "should have themes")

    return {
      { path = "/", template = "page.html", title = "Highlight", content = "<pre>" .. highlighted .. "</pre>" },
    }
  end,
}
"#;

    let (_temp, project_dir) = setup_project(config);

    let (success, _, stderr) = run_build(&project_dir);
    assert!(success, "Build should succeed: {}", stderr);
}

/// Test rs.env module (get with optional default)
#[test]
fn test_lua_env_module() {
    let config = r#"local rs = require("rs-web")

return {
  site = {
    title = "Env Test",
    base_url = "http://localhost:3000",
  },

  pages = function(ctx)
    -- Test env.get on known variable
    local path = rs.env.get("PATH")
    -- PATH may or may not exist, so just test it doesn't error

    -- Test env.get on missing variable returns nil
    local missing = rs.env.get("DEFINITELY_NOT_SET_XYZ123")
    assert(missing == nil, "env.get should return nil for missing vars without default")

    -- Test env.get with default value
    local with_default = rs.env.get("DEFINITELY_NOT_SET_XYZ123", "my_default")
    assert(with_default == "my_default", "env.get should return default for missing vars")

    return {
      { path = "/", template = "page.html", title = "Env Test", content = "<p>Env tests passed</p>" },
    }
  end,
}
"#;

    let (_temp, project_dir) = setup_project(config);

    let (success, _, stderr) = run_build(&project_dir);
    assert!(success, "Build should succeed: {}", stderr);
}

/// Test data() and pages() hooks integration
#[test]
fn test_lua_data_pages_hooks() {
    let config = r#"local rs = require("rs-web")

return {
  site = {
    title = "Hooks Test",
    base_url = "http://localhost:3000",
  },

  data = function()
    -- Return global data that pages can use
    local posts = {}
    for i = 1, 3 do
      table.insert(posts, {
        title = "Post " .. i,
        slug = "post-" .. i,
        content = "Content for post " .. i,
      })
    end
    return {
      posts = posts,
      site_author = "Test Author",
    }
  end,

  pages = function(ctx)
    -- Verify data is passed correctly via ctx.data
    assert(ctx.data.site_author == "Test Author", "data should be passed to pages via ctx.data")
    assert(#ctx.data.posts == 3, "posts should be in ctx.data")

    local pages = {
      { path = "/", template = "page.html", title = "Home", content = "<p>Welcome</p>" },
    }

    -- Generate a page for each post
    for _, post in ipairs(ctx.data.posts) do
      table.insert(pages, {
        path = "/" .. post.slug .. "/",
        template = "page.html",
        title = post.title,
        content = "<p>" .. post.content .. "</p>",
      })
    end

    return pages
  end,
}
"#;

    let (_temp, project_dir) = setup_project(config);

    let (success, _, stderr) = run_build(&project_dir);
    assert!(success, "Build should succeed: {}", stderr);

    // Verify all pages were created
    assert!(project_dir.join("dist/index.html").exists());
    assert!(project_dir.join("dist/post-1/index.html").exists());
    assert!(project_dir.join("dist/post-2/index.html").exists());
    assert!(project_dir.join("dist/post-3/index.html").exists());
}

/// Test before_build and after_build hooks
#[test]
fn test_lua_build_hooks() {
    let config = r#"local rs = require("rs-web")

return {
  site = {
    title = "Build Hooks Test",
    base_url = "http://localhost:3000",
  },

  hooks = {
    before_build = function()
      -- Write a marker file to verify before_build ran
      -- dist/ should already exist after clean()
      rs.fs.write("dist/before-marker.txt", "before_build executed")
    end,

    after_build = function()
      -- Write a marker file to verify after_build ran
      rs.fs.write("dist/after-marker.txt", "after_build executed")
    end,
  },

  pages = function(ctx)
    return {
      { path = "/", template = "page.html", title = "Test", content = "<p>Test</p>" },
    }
  end,
}
"#;

    let (_temp, project_dir) = setup_project(config);

    let (success, _, stderr) = run_build(&project_dir);
    assert!(success, "Build should succeed: {}", stderr);

    // Verify hooks ran
    assert!(project_dir.join("dist/before-marker.txt").exists());
    assert!(project_dir.join("dist/after-marker.txt").exists());
}

/// Comprehensive test exercising many Lua API functions together
#[test]
fn test_lua_comprehensive_site() {
    let config = r#"local rs = require("rs-web")

return {
  site = {
    title = "Comprehensive Test Site",
    description = "A site that tests all Lua APIs",
    base_url = "http://localhost:3000",
    author = "Test Author",
  },

  data = function()
    -- Load all posts from markdown files
    local post_files = rs.fs.glob("site/posts/*.md")

    -- Sort by filename
    post_files = rs.ops.sort(post_files, function(a, b)
      return a.name < b.name
    end)

    -- Load and process posts
    local posts = rs.ops.map(post_files, function(file)
      local fm = rs.data.load_frontmatter(file.path)
      return {
        title = fm.title,
        date = fm.date,
        slug = rs.text.slugify(fm.title),
        content = fm.content,
        word_count = rs.text.word_count(fm.content),
        reading_time = rs.text.reading_time(fm.content),
      }
    end)

    return {
      posts = posts,
      config = rs.data.load_json("data/config.json"),
    }
  end,

  pages = function(ctx)
    local pages = {}
    local data = ctx.data

    -- Home page with list of posts
    local post_list = "<ul>"
    for _, post in ipairs(data.posts) do
      post_list = post_list .. "<li><a href='/" .. post.slug .. "/'>" .. post.title .. "</a></li>"
    end
    post_list = post_list .. "</ul>"

    table.insert(pages, {
      path = "/",
      template = "page.html",
      title = "Home",
      content = "<h2>Posts</h2>" .. post_list,
    })

    -- Individual post pages
    for _, post in ipairs(data.posts) do
      local html = rs.markdown.render(post.content)
      local meta = "<p>Reading time: " .. post.reading_time .. " min (" .. post.word_count .. " words)</p>"

      table.insert(pages, {
        path = "/" .. post.slug .. "/",
        template = "page.html",
        title = post.title,
        content = html,
        extra = meta,
      })
    end

    return pages
  end,
}
"#;

    let (_temp, project_dir) = setup_project(config);

    // Create posts directory
    std::fs::create_dir_all(project_dir.join("site/posts")).unwrap();

    // Create test posts
    std::fs::write(
        project_dir.join("site/posts/01-first.md"),
        "---\ntitle: First Post\ndate: 2024-01-01\n---\n\nThis is the first post content.",
    )
    .unwrap();

    std::fs::write(
        project_dir.join("site/posts/02-second.md"),
        "---\ntitle: Second Post\ndate: 2024-01-15\n---\n\nThis is the second post with more content for reading time.",
    )
    .unwrap();

    // Create config.json
    std::fs::write(
        project_dir.join("data/config.json"),
        r#"{"theme": "default", "features": ["posts", "pages"]}"#,
    )
    .unwrap();

    let (success, _, stderr) = run_build(&project_dir);
    assert!(success, "Build should succeed: {}", stderr);

    // Verify pages were created
    assert!(project_dir.join("dist/index.html").exists());
    assert!(project_dir.join("dist/first-post/index.html").exists());
    assert!(project_dir.join("dist/second-post/index.html").exists());

    // Verify content
    let index = std::fs::read_to_string(project_dir.join("dist/index.html")).unwrap();
    assert!(index.contains("First Post"));
    assert!(index.contains("Second Post"));

    let post1 = std::fs::read_to_string(project_dir.join("dist/first-post/index.html")).unwrap();
    assert!(post1.contains("first post content"));
}
