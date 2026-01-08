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
    ///   - ALWAYS requires interactive confirmation (--batch is ignored)
    ///   - Requires exact workspace ID (ws-xxx), NOT workspace name
    ///   - Workspace is locked during the entire operation
    ///   - If upload fails, workspace is still unlocked
    ///   - Original state lineage is preserved for consistency
    ///
    /// USE CASES:
    ///   - Cleaning up a workspace before deletion
    ///   - Resetting state after manual infrastructure changes
    ///   - Preparing for re-import of resources
    ///   - Removing orphaned resources from state
    ///
    /// WARNING:
    ///   This operation is IRREVERSIBLE without manual state recovery.
    ///   Cloud resources will continue to exist but will no longer be
    ///   tracked by Terraform.
    #[command(verbatim_doc_comment)]
    State(PurgeStateArgs),
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
}
