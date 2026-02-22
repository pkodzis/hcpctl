//! Delete command resource definitions and arguments

use clap::{Parser, Subcommand};

/// Resource types for the 'delete' command
#[derive(Subcommand, Debug)]
pub enum DeleteResource {
    /// Delete organization member (remove from organization)
    #[command(
        visible_alias = "org-members",
        visible_alias = "orgmember",
        visible_alias = "orgmembers"
    )]
    OrgMember(DeleteOrgMemberArgs),

    /// Delete tag bindings from a workspace or project
    #[command(visible_alias = "tags")]
    Tag {
        #[command(subcommand)]
        resource: super::tag::DeleteTagResource,
    },
}

/// Arguments for 'delete org-member' subcommand
#[derive(Parser, Debug)]
pub struct DeleteOrgMemberArgs {
    /// Membership ID (ou-xxx) or email address to delete
    ///
    ///   ou-xxx   Membership ID - deletes directly
    ///   email    Email address - requires --org to identify the membership
    #[arg(verbatim_doc_comment)]
    pub id: String,

    /// Organization name (required when argument is an email)
    #[arg(long = "org")]
    pub org: Option<String>,

    /// Skip confirmation prompt
    #[arg(short = 'y', long, default_value_t = false)]
    pub yes: bool,
}
