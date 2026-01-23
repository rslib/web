//! End-to-end tests for CLI commands
//!
//! Tests the `rs-web new` and `rs-web build` commands to ensure
//! the complete workflow works correctly.

use std::process::Command;
use tempfile::TempDir;

/// Get the path to the rs-web binary
fn get_binary_path() -> String {
    // When running tests, cargo builds the binary in target/debug or target/release
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test binary name
    path.pop(); // Remove deps
    path.push("rs-web");
    path.to_string_lossy().to_string()
}

/// Test the `rs-web new` command creates a valid project structure
#[test]
fn test_new_command_creates_project() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_name = "test-site";

    let output = Command::new(get_binary_path())
        .args(["new", project_name, "-d", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "rs-web new failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let project_dir = temp_dir.path().join(project_name);

    // Check that all expected files and directories are created
    assert!(project_dir.exists(), "Project directory should exist");
    assert!(
        project_dir.join("config.lua").exists(),
        "config.lua should exist"
    );
    assert!(
        project_dir.join("templates").exists(),
        "templates directory should exist"
    );
    assert!(
        project_dir.join("templates/base.html").exists(),
        "base.html template should exist"
    );
    assert!(
        project_dir.join("templates/page.html").exists(),
        "page.html template should exist"
    );
    assert!(
        project_dir.join("site").exists(),
        "site directory should exist"
    );
    assert!(
        project_dir.join("site/index.md").exists(),
        "index.md should exist"
    );
    assert!(
        project_dir.join("static").exists(),
        "static directory should exist"
    );
    assert!(
        project_dir.join(".types").exists(),
        ".types directory should exist"
    );
    assert!(
        project_dir.join(".types/rs-web.lua").exists(),
        "Lua type definitions should exist"
    );
    assert!(
        project_dir.join(".luarc.json").exists(),
        ".luarc.json should exist"
    );
    assert!(
        project_dir.join(".gitignore").exists(),
        ".gitignore should exist"
    );
}

/// Test that `rs-web new` with --force works on existing directories
#[test]
fn test_new_command_force_existing_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_name = "existing-site";
    let project_dir = temp_dir.path().join(project_name);

    // Create the directory first
    std::fs::create_dir_all(&project_dir).expect("Failed to create project dir");

    // Create a file that should NOT be overwritten
    let custom_file = project_dir.join("custom.txt");
    std::fs::write(&custom_file, "custom content").expect("Failed to write custom file");

    // Run new with --force
    let output = Command::new(get_binary_path())
        .args([
            "new",
            project_name,
            "-d",
            temp_dir.path().to_str().unwrap(),
            "--force",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "rs-web new --force failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check custom file is preserved
    assert!(custom_file.exists(), "Custom file should be preserved");
    assert_eq!(
        std::fs::read_to_string(&custom_file).unwrap(),
        "custom content",
        "Custom file content should be unchanged"
    );

    // Check new project files exist
    assert!(
        project_dir.join("config.lua").exists(),
        "config.lua should exist"
    );
}

/// Test that `rs-web new` fails on existing directory without --force
#[test]
fn test_new_command_fails_on_existing_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_name = "existing-site";
    let project_dir = temp_dir.path().join(project_name);

    // Create the directory first
    std::fs::create_dir_all(&project_dir).expect("Failed to create project dir");

    // Run new without --force
    let output = Command::new(get_binary_path())
        .args(["new", project_name, "-d", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to execute command");

    assert!(
        !output.status.success(),
        "rs-web new should fail on existing directory without --force"
    );
}

/// Test the `rs-web build` command on a newly created project
#[test]
fn test_build_command_basic() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_name = "build-test-site";

    // First, create a new project
    let output = Command::new(get_binary_path())
        .args(["new", project_name, "-d", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to execute new command");

    assert!(
        output.status.success(),
        "rs-web new failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let project_dir = temp_dir.path().join(project_name);

    // Now build the project
    let output = Command::new(get_binary_path())
        .args(["build", "-d", project_dir.to_str().unwrap()])
        .output()
        .expect("Failed to execute build command");

    assert!(
        output.status.success(),
        "rs-web build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check output directory exists
    let output_dir = project_dir.join("dist");
    assert!(output_dir.exists(), "Output directory should exist");

    // Check index.html is generated
    let index_html = output_dir.join("index.html");
    assert!(index_html.exists(), "index.html should be generated");

    // Check the content of index.html
    let content = std::fs::read_to_string(&index_html).expect("Failed to read index.html");
    assert!(
        content.contains("Home"),
        "index.html should contain the page title"
    );
    assert!(
        content.contains("Welcome"),
        "index.html should contain the welcome message"
    );
}

/// Test the `rs-web build` with custom output directory
#[test]
fn test_build_command_custom_output() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_name = "custom-output-test";

    // Create a new project
    let output = Command::new(get_binary_path())
        .args(["new", project_name, "-d", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to execute new command");

    assert!(output.status.success());

    let project_dir = temp_dir.path().join(project_name);
    let custom_output = temp_dir.path().join("custom-dist");

    // Build with custom output directory
    let output = Command::new(get_binary_path())
        .args([
            "build",
            "-d",
            project_dir.to_str().unwrap(),
            "-o",
            custom_output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute build command");

    assert!(
        output.status.success(),
        "rs-web build with custom output failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check custom output directory exists
    assert!(
        custom_output.exists(),
        "Custom output directory should exist"
    );
    assert!(
        custom_output.join("index.html").exists(),
        "index.html should be in custom output"
    );
}

/// Test building a project with multiple pages
#[test]
fn test_build_command_multiple_pages() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_name = "multi-page-test";

    // Create a new project
    Command::new(get_binary_path())
        .args(["new", project_name, "-d", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to execute new command");

    let project_dir = temp_dir.path().join(project_name);

    // Create an about page
    let about_md = r#"---
title: About
---

This is the about page.
"#;
    std::fs::write(project_dir.join("site/about.md"), about_md).expect("Failed to write about.md");

    // Update config.lua to include the about page
    let config = r#"local rs = require("rs-web")

return {
  site = {
    title = "Multi-page Test",
    description = "A test site with multiple pages",
    base_url = "http://localhost:3000",
    author = "Test Author",
  },

  pages = function(ctx)
    local index = rs.data.load_frontmatter("site/index.md")
    local about = rs.data.load_frontmatter("site/about.md")

    return {
      {
        path = "/",
        template = "page.html",
        title = index.title,
        content = rs.markdown.render(index.content),
      },
      {
        path = "/about/",
        template = "page.html",
        title = about.title,
        content = rs.markdown.render(about.content),
      },
    }
  end,
}
"#;
    std::fs::write(project_dir.join("config.lua"), config).expect("Failed to write config.lua");

    // Build the project
    let output = Command::new(get_binary_path())
        .args(["build", "-d", project_dir.to_str().unwrap()])
        .output()
        .expect("Failed to execute build command");

    assert!(
        output.status.success(),
        "rs-web build with multiple pages failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output_dir = project_dir.join("dist");

    // Check both pages are generated
    assert!(
        output_dir.join("index.html").exists(),
        "index.html should exist"
    );
    assert!(
        output_dir.join("about/index.html").exists(),
        "about/index.html should exist"
    );

    // Verify about page content
    let about_html =
        std::fs::read_to_string(output_dir.join("about/index.html")).expect("Failed to read about");
    assert!(about_html.contains("About"), "About page should have title");
    assert!(
        about_html.contains("about page"),
        "About page should have content"
    );
}

/// Test the `rs-web types` command
#[test]
fn test_types_command_lua() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let output = Command::new(get_binary_path())
        .args(["types", "--lua", "-o", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to execute types command");

    assert!(
        output.status.success(),
        "rs-web types --lua failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let lua_file = temp_dir.path().join("rs-web.lua");
    assert!(lua_file.exists(), "rs-web.lua should be generated");

    let content = std::fs::read_to_string(&lua_file).expect("Failed to read lua file");
    assert!(
        content.contains("---@class"),
        "Should contain EmmyLua annotations"
    );
}

/// Test the `rs-web types` command for markdown
#[test]
fn test_types_command_markdown() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let output = Command::new(get_binary_path())
        .args([
            "types",
            "--markdown",
            "-o",
            temp_dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute types command");

    assert!(
        output.status.success(),
        "rs-web types --markdown failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let md_file = temp_dir.path().join("LUA_API.md");
    assert!(md_file.exists(), "LUA_API.md should be generated");

    let content = std::fs::read_to_string(&md_file).expect("Failed to read markdown file");
    assert!(content.contains("# "), "Should contain markdown headers");
}

/// Test error handling for invalid project directory
#[test]
fn test_build_command_invalid_directory() {
    let output = Command::new(get_binary_path())
        .args(["build", "-d", "/nonexistent/path/to/project"])
        .output()
        .expect("Failed to execute build command");

    assert!(
        !output.status.success(),
        "rs-web build should fail for nonexistent directory"
    );
}

/// Test error handling for invalid config.lua
#[test]
fn test_build_command_invalid_config() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_name = "invalid-config-test";

    // Create a new project
    Command::new(get_binary_path())
        .args(["new", project_name, "-d", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to execute new command");

    let project_dir = temp_dir.path().join(project_name);

    // Write invalid Lua config
    std::fs::write(
        project_dir.join("config.lua"),
        "this is not valid lua syntax {{{{",
    )
    .expect("Failed to write invalid config");

    // Build should fail
    let output = Command::new(get_binary_path())
        .args(["build", "-d", project_dir.to_str().unwrap()])
        .output()
        .expect("Failed to execute build command");

    assert!(
        !output.status.success(),
        "rs-web build should fail with invalid config.lua"
    );
}
