//! Integration tests for CLI functionality

use std::process::Command;

/// Get path to compiled binary
fn hcp_cli_bin() -> &'static std::path::Path {
    assert_cmd::cargo::cargo_bin!("hcp-cli")
}

/// Test that help flag works
#[test]
fn test_help_flag() {
    let output = Command::new(hcp_cli_bin())
        .arg("--help")
        .output()
        .unwrap();
    
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("List and explore TFE workspaces"));
}

/// Test that version flag works
#[test]
fn test_version_flag() {
    let output = Command::new(hcp_cli_bin())
        .arg("--version")
        .output()
        .unwrap();
    
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hcp-cli"));
}

/// Test invalid format argument
#[test]
fn test_invalid_format() {
    let output = Command::new(hcp_cli_bin())
        .args(["--format", "invalid"])
        .output()
        .unwrap();
    
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid"));
}

/// Test valid format arguments are accepted (program runs, argument parsing succeeds)
#[test]
fn test_valid_format_table() {
    let output = Command::new(hcp_cli_bin())
        .args(["--format", "table", "-o", "test"])
        .output()
        .unwrap();
    
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Org")); // Table header
}

/// Test valid format arguments are accepted
#[test]
fn test_valid_format_json() {
    let output = Command::new(hcp_cli_bin())
        .args(["--format", "json", "-o", "test"])
        .output()
        .unwrap();
    
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[")); // JSON array
}

/// Test valid format arguments are accepted  
#[test]
fn test_valid_format_csv() {
    let output = Command::new(hcp_cli_bin())
        .args(["--format", "csv", "-o", "test"])
        .output()
        .unwrap();
    
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("org,workspace_name")); // CSV header
}
