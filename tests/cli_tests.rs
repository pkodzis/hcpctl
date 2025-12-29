//! Integration tests for CLI functionality

use std::process::Command;

/// Get path to compiled binary
fn hcp_cli_bin() -> &'static std::path::Path {
    assert_cmd::cargo::cargo_bin!("hcp-cli")
}

/// Test that help flag works and shows expected content
#[test]
fn test_help_flag() {
    let output = Command::new(hcp_cli_bin()).arg("--help").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify key CLI options are documented
    assert!(stdout.contains("--token"), "Should document --token option");
    assert!(stdout.contains("--org"), "Should document --org option");
    assert!(
        stdout.contains("--format"),
        "Should document --format option"
    );
    assert!(stdout.contains("--host"), "Should document --host option");
    assert!(stdout.contains("-t"), "Should have short -t for token");
    assert!(stdout.contains("-o"), "Should have short -o for org");
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

/// Test invalid format argument is rejected
#[test]
fn test_invalid_format_rejected() {
    let output = Command::new(hcp_cli_bin())
        .args(["--format", "xml"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("xml") || stderr.contains("invalid"),
        "Should mention the invalid value"
    );
}

/// Test invalid sort field is rejected
#[test]
fn test_invalid_sort_rejected() {
    let output = Command::new(hcp_cli_bin())
        .args(["--sort", "invalid-field"])
        .output()
        .unwrap();

    assert!(!output.status.success());
}

/// Test that missing token shows helpful error (not crash)
#[test]
fn test_missing_token_shows_help() {
    let output = Command::new(hcp_cli_bin())
        .args(["--host", "nonexistent.example.com"])
        .env_remove("HCP_TOKEN")
        .env_remove("TFC_TOKEN")
        .env_remove("TFE_TOKEN")
        .output()
        .unwrap();

    // Should fail but with helpful message
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("token") || stderr.contains("TOKEN"),
        "Error should mention token"
    );
    assert!(
        stderr.contains("hcp-cli --token") || stderr.contains("terraform login"),
        "Error should suggest how to provide token"
    );
}
