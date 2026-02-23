//! Purge command resource definitions and arguments

use clap::{Parser, Subcommand};

/// Resource types for the 'purge' command
#[derive(Subcommand, Debug)]
pub enum PurgeResource {
    /// Purge all resources from a workspace's Terraform state
    ///
    /// This is a DESTRUCTIVE operation that removes all resources from the state.
    /// The actual infrastructure will NOT be destroyed, but Terraform will
    /// "forget" about the resources, making them orphaned.
    ///
    /// PROCEDURE:
    ///
    ///   1. Fetches workspace info and validates it exists
    ///   2. Fetches current state version metadata
    ///   3. Displays warning and requires confirmation (type workspace ID)
    ///   4. LOCKS the workspace to prevent concurrent modifications
    ///   5. Downloads the current Terraform state file
    ///   6. Creates a new empty state (preserving lineage, incrementing serial)
    ///   7. Uploads the empty state as a new state version
    ///   8. UNLOCKS the workspace (always, even on error)
    ///
    /// SAFETY:
    ///
    ///   - Requires interactive confirmation by default (--batch is ignored)
    ///   - Requires exact workspace ID (ws-xxx), NOT workspace name
    ///   - Workspace is locked during the entire operation
    ///   - If upload fails, workspace is still unlocked
    ///   - Original state lineage is preserved for consistency
    ///
    /// USE CASES:
    ///
    ///   - Cleaning up a workspace before deletion
    ///   - Resetting state after manual infrastructure changes
    ///   - Preparing for re-import of resources
    ///   - Removing orphaned resources from state
    ///
    /// WARNING:
    ///
    ///   - This operation is IRREVERSIBLE without manual state recovery.
    ///   - Cloud resources will continue to exist but will no longer be
    ///     tracked by Terraform.
    #[command(verbatim_doc_comment)]
    State(PurgeStateArgs),

    /// Cancel/discard pending runs blocking a workspace
    ///
    /// Cancels or discards all pending runs that are blocking a workspace,
    /// including the current run holding the workspace lock if applicable.
    ///
    /// PROCEDURE:
    ///
    ///   1. Resolves workspace by name or ID (auto-discovers organization)
    ///   2. Fetches all pending runs and current run
    ///   3. Displays summary table with run details
    ///   4. Requires user confirmation
    ///   5. Processes runs: pending first (newest→oldest), then current run
    ///   6. Uses appropriate action (cancel/discard) based on run state
    ///
    /// ACTIONS:
    ///
    ///   - cancel: Interrupts actively executing run (planning/applying)
    ///   - discard: Skips run waiting for confirmation or priority
    ///
    /// USE CASES:
    ///
    ///   - Clearing stacked pending runs from CI/CD
    ///   - Unblocking workspace stuck on failed/abandoned run
    ///   - Cleaning up runs before workspace maintenance
    ///
    /// NOTES:
    ///
    ///   - Use --dry-run to preview without making changes
    ///   - Workspace name can be used (auto-discovers organization)
    ///   - Workspace ID (ws-xxx) can also be used directly
    #[command(verbatim_doc_comment, visible_alias = "runs")]
    Run(PurgeRunArgs),
}

/// Arguments for 'purge state' subcommand
#[derive(Parser, Debug)]
pub struct PurgeStateArgs {
    /// Workspace ID (ws-xxx) to purge state from
    ///
    /// Must be the exact workspace ID, not the workspace name.
    /// You can find the workspace ID using: hcpctl get ws NAME --org ORG -o json
    #[arg(verbatim_doc_comment)]
    pub workspace_id: String,

    /// Batch mode - no interactive prompts, no spinners
    #[arg(long)]
    pub my_resume_is_updated: bool,
}

/// Arguments for 'purge run' subcommand
#[derive(Parser, Debug)]
pub struct PurgeRunArgs {
    /// Workspace name or ID (ws-xxx) to purge runs from
    ///
    /// Can be either:
    /// - Workspace name (e.g., "my-workspace") - requires --org or auto-discovery
    /// - Workspace ID (e.g., "ws-abc123") - organization auto-detected
    #[arg(verbatim_doc_comment)]
    pub workspace: String,

    /// Organization name (auto-detected if not provided)
    #[arg(short, long)]
    pub org: Option<String>,

    /// Preview what would be canceled without making changes
    #[arg(long)]
    pub dry_run: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_purge_resource_subcommands() {
        // Verify PurgeResource can be used as a subcommand
        #[derive(Parser)]
        struct TestCli {
            #[command(subcommand)]
            resource: PurgeResource,
        }

        // This should not panic - validates the structure
        TestCli::command().debug_assert();
    }

    #[test]
    fn test_purge_state_args_parsing() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(subcommand)]
            resource: PurgeResource,
        }

        let cli = TestCli::parse_from(["test", "state", "ws-abc123"]);
        match cli.resource {
            PurgeResource::State(args) => {
                assert_eq!(args.workspace_id, "ws-abc123");
            }
            _ => panic!("Expected State variant"),
        }
    }

    #[test]
    fn test_purge_state_requires_workspace_id() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(subcommand)]
            resource: PurgeResource,
        }

        // Missing workspace_id should fail
        let result = TestCli::try_parse_from(["test", "state"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_purge_run_args_parsing() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(subcommand)]
            resource: PurgeResource,
        }

        // Test with workspace name only
        let cli = TestCli::parse_from(["test", "run", "my-workspace"]);
        match cli.resource {
            PurgeResource::Run(args) => {
                assert_eq!(args.workspace, "my-workspace");
                assert!(args.org.is_none());
                assert!(!args.dry_run);
            }
            _ => panic!("Expected Run variant"),
        }
    }

    #[test]
    fn test_purge_run_with_org_and_dry_run() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(subcommand)]
            resource: PurgeResource,
        }

        let cli = TestCli::parse_from([
            "test",
            "run",
            "my-workspace",
            "--org",
            "my-org",
            "--dry-run",
        ]);
        match cli.resource {
            PurgeResource::Run(args) => {
                assert_eq!(args.workspace, "my-workspace");
                assert_eq!(args.org, Some("my-org".to_string()));
                assert!(args.dry_run);
            }
            _ => panic!("Expected Run variant"),
        }
    }

    #[test]
    fn test_purge_run_alias_runs() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(subcommand)]
            resource: PurgeResource,
        }

        // Test alias 'runs' works
        let cli = TestCli::parse_from(["test", "runs", "ws-abc123"]);
        match cli.resource {
            PurgeResource::Run(args) => {
                assert_eq!(args.workspace, "ws-abc123");
            }
            _ => panic!("Expected Run variant"),
        }
    }

    #[test]
    fn test_purge_run_requires_workspace() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(subcommand)]
            resource: PurgeResource,
        }

        // Missing workspace should fail
        let result = TestCli::try_parse_from(["test", "run"]);
        assert!(result.is_err());
    }
}
