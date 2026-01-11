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

/// Test that 'logs' subcommand help shows expected options
#[test]
fn test_logs_help_flag() {
    let output = Command::new(hcpctl_bin())
        .args(["logs", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify key options are documented
    assert!(stdout.contains("--apply"), "Should document --apply option");
    assert!(
        stdout.contains("--follow"),
        "Should document --follow option"
    );
    assert!(stdout.contains("--raw"), "Should document --raw option");
    assert!(stdout.contains("--org"), "Should document --org option");
    assert!(stdout.contains("-f"), "Should have short -f for follow");
    assert!(stdout.contains("-a"), "Should have short -a for apply");
}

/// Test that 'log' alias works
#[test]
fn test_logs_alias() {
    let output = Command::new(hcpctl_bin())
        .args(["log", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("--follow"),
        "Should document --follow option"
    );
}

/// Test that logs target types are documented
#[test]
fn test_logs_target_types_documented() {
    let output = Command::new(hcpctl_bin())
        .args(["logs", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("run-"), "Should document run ID format");
    assert!(
        stdout.contains("ws-"),
        "Should document workspace ID format"
    );
    assert!(
        stdout.contains("workspace name"),
        "Should document workspace name"
    );
}

/// Test that main help shows logs command
#[test]
fn test_main_help_shows_logs() {
    let output = Command::new(hcpctl_bin()).arg("--help").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("logs"), "Should show logs command");
    assert!(stdout.contains("log]"), "Should show log alias");
}

// =========================================================================
// Watch command tests
// =========================================================================

/// Test that 'watch' subcommand help shows expected resources
#[test]
fn test_watch_help_flag() {
    let output = Command::new(hcpctl_bin())
        .args(["watch", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify ws resource is documented
    assert!(stdout.contains("ws"), "Should document ws resource");
}

/// Test that 'watch ws' subcommand help shows expected options
#[test]
fn test_watch_ws_help_flag() {
    let output = Command::new(hcpctl_bin())
        .args(["watch", "ws", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify key options are documented
    assert!(
        stdout.contains("--no-prefix"),
        "Should document --no-prefix option"
    );
    assert!(
        stdout.contains("--interval"),
        "Should document --interval option"
    );
    assert!(stdout.contains("--apply"), "Should document --apply option");
    assert!(stdout.contains("--raw"), "Should document --raw option");
    assert!(stdout.contains("--org"), "Should document --org option");
    assert!(stdout.contains("-i"), "Should have short -i for interval");
    assert!(stdout.contains("-a"), "Should have short -a for apply");
}

/// Test that 'watch workspace' alias works
#[test]
fn test_watch_ws_alias() {
    let output = Command::new(hcpctl_bin())
        .args(["watch", "workspace", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("--interval"),
        "Should document --interval option"
    );
}

/// Test that watch ws target types are documented
#[test]
fn test_watch_ws_target_types_documented() {
    let output = Command::new(hcpctl_bin())
        .args(["watch", "ws", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("ws-"),
        "Should document workspace ID format"
    );
    assert!(
        stdout.contains("Workspace name") || stdout.contains("name"),
        "Should document workspace name option"
    );
}

/// Test that main help shows watch command
#[test]
fn test_main_help_shows_watch() {
    let output = Command::new(hcpctl_bin()).arg("--help").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("watch"), "Should show watch command");
}

/// Test that watch ws default interval is 3
#[test]
fn test_watch_ws_default_interval() {
    let output = Command::new(hcpctl_bin())
        .args(["watch", "ws", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("3") || stdout.contains("default"),
        "Should show default interval"
    );
}

/// Test that watch ws no-prefix default is documented
#[test]
fn test_watch_ws_prefix_default() {
    let output = Command::new(hcpctl_bin())
        .args(["watch", "ws", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Help should mention that prefix is enabled by default (no-prefix disables it)
    assert!(
        stdout.contains("Disable") || stdout.contains("prefix"),
        "Should document prefix behavior"
    );
}

// =============================================================================
// Delete org-member tests
// =============================================================================

/// Test that 'delete org-member' requires ID argument
#[test]
fn test_delete_org_member_requires_id() {
    let output = Command::new(hcpctl_bin())
        .args(["delete", "org-member"])
        .output()
        .unwrap();

    // Should fail - ID is required
    assert!(
        !output.status.success(),
        "delete org-member without ID should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("required") || stderr.contains("ID"),
        "Error should mention required argument"
    );
}

/// Test that 'delete org-member' help shows expected options
#[test]
fn test_delete_org_member_help() {
    let output = Command::new(hcpctl_bin())
        .args(["delete", "org-member", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should document the ID/email argument
    assert!(
        stdout.contains("ou-xxx") || stdout.contains("Membership ID"),
        "Should document membership ID format"
    );
    assert!(
        stdout.contains("email") || stdout.contains("Email"),
        "Should document email option"
    );
    assert!(stdout.contains("--org"), "Should document --org option");
    assert!(
        stdout.contains("-y") || stdout.contains("--yes"),
        "Should document skip confirmation option"
    );
}

/// Test that 'get org-member' help shows expected options
#[test]
fn test_get_org_member_help() {
    let output = Command::new(hcpctl_bin())
        .args(["get", "org-member", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("--org"), "Should document --org option");
    assert!(
        stdout.contains("--filter") || stdout.contains("-f"),
        "Should document filter option"
    );
    assert!(
        stdout.contains("--status"),
        "Should document status filter option"
    );
}

/// Test that 'invite' command help shows expected options
#[test]
fn test_invite_help() {
    let output = Command::new(hcpctl_bin())
        .args(["invite", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("--org"), "Should document --org option");
    assert!(
        stdout.contains("EMAIL") || stdout.contains("email"),
        "Should document email argument"
    );
    assert!(stdout.contains("--teams"), "Should document teams option");
}

// === Purge command tests ===

/// Test that 'purge' subcommand shows resources
#[test]
fn test_purge_help_flag() {
    let output = Command::new(hcpctl_bin())
        .args(["purge", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify resources are listed
    assert!(stdout.contains("state"), "Should document state resource");
    assert!(
        stdout.contains("IRREVERSIBLE") || stdout.contains("confirmation"),
        "Should warn about destructive nature"
    );
}

/// Test that 'purge state' subcommand help shows expected options
#[test]
fn test_purge_state_help_flag() {
    let output = Command::new(hcpctl_bin())
        .args(["purge", "state", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify key options are documented
    assert!(
        stdout.contains("ws-") || stdout.contains("workspace"),
        "Should document workspace ID requirement"
    );
    assert!(
        stdout.contains("WORKSPACE_ID") || stdout.contains("workspace-id"),
        "Should show workspace-id argument"
    );
}

/// Test that 'purge state' requires workspace ID argument
#[test]
fn test_purge_state_requires_workspace_id() {
    let output = Command::new(hcpctl_bin())
        .args(["purge", "state"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("WORKSPACE_ID") || stderr.contains("required"),
        "Should indicate workspace ID is required"
    );
}

/// Test that 'purge run' subcommand help shows expected options
#[test]
fn test_purge_run_help_flag() {
    let output = Command::new(hcpctl_bin())
        .args(["purge", "run", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify key options are documented
    assert!(
        stdout.contains("workspace") || stdout.contains("WORKSPACE"),
        "Should document workspace argument"
    );
    assert!(stdout.contains("--org"), "Should document --org option");
    assert!(
        stdout.contains("--dry-run"),
        "Should document --dry-run option"
    );
}

/// Test that 'purge run' requires workspace argument
#[test]
fn test_purge_run_requires_workspace() {
    let output = Command::new(hcpctl_bin())
        .args(["purge", "run"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("WORKSPACE") || stderr.contains("required"),
        "Should indicate workspace is required"
    );
}

/// Test that 'purge runs' alias works
#[test]
fn test_purge_runs_alias() {
    let output = Command::new(hcpctl_bin())
        .args(["purge", "runs", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Cancel") || stdout.contains("discard") || stdout.contains("pending"),
        "Should show runs help when using 'runs' alias"
    );
}

/// Test that 'purge state' is documented in main help
#[test]
fn test_main_help_shows_purge() {
    let output = Command::new(hcpctl_bin()).arg("--help").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("purge") || stdout.contains("Purge"),
        "Should show purge command in main help"
    );
}
