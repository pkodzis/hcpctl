//! Integration tests for CLI functionality
//!
//! Testing kubectl-style command structure:
//! - hcpctl get org [NAME]
//! - hcpctl get prj [NAME] --org ORG
//! - hcpctl get ws [NAME] --org ORG

use std::process::Command;

/// Get path to compiled binary
fn hcpctl_bin() -> &'static std::path::Path {
    assert_cmd::cargo::cargo_bin!("hcpctl")
}

/// Test that help flag works and shows expected content
#[test]
fn test_help_flag() {
    let output = Command::new(hcpctl_bin()).arg("--help").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify key CLI subcommands are documented (kubectl-style 'get')
    assert!(stdout.contains("get"), "Should document get subcommand");
    assert!(stdout.contains("--token"), "Should document --token option");
    assert!(stdout.contains("--host"), "Should document --host option");
    assert!(stdout.contains("-t"), "Should have short -t for token");
}

/// Test that 'get' subcommand shows resources
#[test]
fn test_get_help_flag() {
    let output = Command::new(hcpctl_bin())
        .args(["get", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify resources are listed
    assert!(stdout.contains("org"), "Should document org resource");
    assert!(stdout.contains("prj"), "Should document prj resource");
    assert!(stdout.contains("ws"), "Should document ws resource");
}

/// Test that 'get ws' subcommand help shows expected options
#[test]
fn test_ws_help_flag() {
    let output = Command::new(hcpctl_bin())
        .args(["get", "ws", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify key ws options are documented
    assert!(
        stdout.contains("--output") || stdout.contains("-o"),
        "Should document --output option"
    );
    assert!(
        stdout.contains("--filter"),
        "Should document --filter option"
    );
    assert!(stdout.contains("--sort"), "Should document --sort option");
    assert!(stdout.contains("-f"), "Should have short -f for filter");
}

/// Test that 'get prj' subcommand help shows expected options
#[test]
fn test_prj_help_flag() {
    let output = Command::new(hcpctl_bin())
        .args(["get", "prj", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("--org"), "Should document --org option");
    assert!(
        stdout.contains("--output") || stdout.contains("-o"),
        "Should document --output option"
    );
}

/// Test that 'get org' subcommand help shows expected options
#[test]
fn test_org_help_flag() {
    let output = Command::new(hcpctl_bin())
        .args(["get", "org", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("NAME") || stdout.contains("name"),
        "Should document optional NAME argument"
    );
}

/// Test that version flag works
#[test]
fn test_version_flag() {
    let output = Command::new(hcpctl_bin())
        .arg("--version")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hcpctl"));
}

/// Test invalid output format argument is rejected for 'get ws'
#[test]
fn test_invalid_format_rejected() {
    let output = Command::new(hcpctl_bin())
        .args(["get", "ws", "--output", "xml"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("xml") || stderr.contains("invalid") || stderr.contains("possible values"),
        "Should mention the invalid value"
    );
}

/// Test invalid sort field is rejected for 'get ws'
#[test]
fn test_invalid_sort_rejected() {
    let output = Command::new(hcpctl_bin())
        .args(["get", "ws", "--sort", "invalid-field"])
        .output()
        .unwrap();

    assert!(!output.status.success());
}

/// Test that missing token shows helpful error (not crash) for 'get org'
#[test]
fn test_missing_token_shows_help() {
    let output = Command::new(hcpctl_bin())
        .args(["--host", "nonexistent.example.com", "get", "org"])
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
        stderr.contains("hcpctl --token") || stderr.contains("terraform login"),
        "Error should suggest how to provide token"
    );
}

/// Test that 'get prj' shows optional org in help
#[test]
fn test_prj_help_shows_optional_org() {
    let output = Command::new(hcpctl_bin())
        .args(["get", "prj", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // org should be optional (shown as --org)
    assert!(
        stdout.contains("--org"),
        "--org option should be documented"
    );
}

/// Test aliases work via 'get' command (orgs, prjs, workspaces)
#[test]
fn test_aliases() {
    // Test 'get orgs' alias
    let output = Command::new(hcpctl_bin())
        .args(["get", "orgs", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success(), "'get orgs' alias should work");

    // Test 'get organizations' alias
    let output = Command::new(hcpctl_bin())
        .args(["get", "organizations", "--help"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "'get organizations' alias should work"
    );

    // Test 'get prjs' alias
    let output = Command::new(hcpctl_bin())
        .args(["get", "prjs", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success(), "'get prjs' alias should work");

    // Test 'get projects' alias
    let output = Command::new(hcpctl_bin())
        .args(["get", "projects", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success(), "'get projects' alias should work");

    // Test 'get workspace' alias
    let output = Command::new(hcpctl_bin())
        .args(["get", "workspace", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success(), "'get workspace' alias should work");

    // Test 'get workspaces' alias
    let output = Command::new(hcpctl_bin())
        .args(["get", "workspaces", "--help"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "'get workspaces' alias should work"
    );
}

/// Test that 'get oc' (OAuth clients) alias works
#[test]
fn test_oauth_client_alias() {
    let output = Command::new(hcpctl_bin())
        .args(["get", "oc", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success(), "'get oc' should work");

    let output = Command::new(hcpctl_bin())
        .args(["get", "oauth-clients", "--help"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "'get oauth-clients' alias should work"
    );
}

/// Test that output formats are documented
#[test]
fn test_output_formats_documented() {
    let output = Command::new(hcpctl_bin())
        .args(["get", "ws", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("table"), "Should document table format");
    assert!(stdout.contains("json"), "Should document json format");
    assert!(stdout.contains("yaml"), "Should document yaml format");
    assert!(stdout.contains("csv"), "Should document csv format");
}

/// Test that --batch flag is documented
#[test]
fn test_batch_flag_documented() {
    let output = Command::new(hcpctl_bin()).arg("--help").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("--batch") || stdout.contains("-b"),
        "Should document --batch option"
    );
}

/// Test that --no-header flag is documented
#[test]
fn test_no_header_flag_documented() {
    let output = Command::new(hcpctl_bin()).arg("--help").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("--no-header"),
        "Should document --no-header option"
    );
}

/// Test that sort options are documented for ws
#[test]
fn test_ws_sort_options_documented() {
    let output = Command::new(hcpctl_bin())
        .args(["get", "ws", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("--sort"), "Should document --sort option");
    assert!(stdout.contains("name"), "Should list name sort field");
}

/// Test that project filter is documented for ws
#[test]
fn test_ws_project_filter_documented() {
    let output = Command::new(hcpctl_bin())
        .args(["get", "ws", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("--prj") || stdout.contains("-p"),
        "Should document --prj option"
    );
}

/// Test that group-by-prj flag is documented
#[test]
fn test_ws_group_by_prj_documented() {
    let output = Command::new(hcpctl_bin())
        .args(["get", "ws", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("--group-by-prj"),
        "Should document --group-by-prj option"
    );
}

/// Test that prj workspace info flags are documented
#[test]
fn test_prj_workspace_flags_documented() {
    let output = Command::new(hcpctl_bin())
        .args(["get", "prj", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("--with-ws"), "Should document --with-ws");
}

/// Test invalid subcommand is rejected
#[test]
fn test_invalid_subcommand_rejected() {
    let output = Command::new(hcpctl_bin())
        .args(["get", "invalid-resource"])
        .output()
        .unwrap();

    assert!(!output.status.success());
}

/// Test that global options come before subcommand
#[test]
fn test_global_options_before_subcommand() {
    let output = Command::new(hcpctl_bin())
        .args(["--batch", "--no-header", "get", "org", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
}
