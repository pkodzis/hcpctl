//! Sort and subresource enums for CLI commands

use clap::ValueEnum;

/// Sort field options for projects
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PrjSortField {
    /// Sort by project name (default)
    Name,
    /// Sort by workspace count
    Workspaces,
}

impl std::fmt::Display for PrjSortField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrjSortField::Name => write!(f, "name"),
            PrjSortField::Workspaces => write!(f, "workspaces"),
        }
    }
}

/// Sort field options for workspaces
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum WsSortField {
    /// Sort by workspace name (default)
    Name,
    /// Sort by resource count
    Resources,
    /// Sort by last update time
    UpdatedAt,
    /// Sort by Terraform version
    TfVersion,
}

impl std::fmt::Display for WsSortField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WsSortField::Name => write!(f, "name"),
            WsSortField::Resources => write!(f, "resources"),
            WsSortField::UpdatedAt => write!(f, "updated-at"),
            WsSortField::TfVersion => write!(f, "tf-version"),
        }
    }
}

/// Sort field options for runs
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum RunSortField {
    /// Sort by creation time (default: newest first)
    #[default]
    CreatedAt,
    /// Sort by status
    Status,
    /// Sort by workspace ID
    #[value(name = "ws-id")]
    WsId,
}

impl std::fmt::Display for RunSortField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunSortField::CreatedAt => write!(f, "created-at"),
            RunSortField::Status => write!(f, "status"),
            RunSortField::WsId => write!(f, "ws-id"),
        }
    }
}

/// Run subresources that can be fetched
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RunSubresource {
    /// Run events (run-events)
    Events,
    /// Plan details with log access
    Plan,
    /// Apply details with log access
    Apply,
}

/// Workspace subresources that can be fetched
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum WsSubresource {
    /// Current run (current-run)
    Run,
    /// Current state version (current-state-version)
    State,
    /// Current configuration version (current-configuration-version)
    Config,
    /// Current assessment result (current-assessment-result)
    Assessment,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_sort_field_display() {
        assert_eq!(WsSortField::Name.to_string(), "name");
        assert_eq!(WsSortField::Resources.to_string(), "resources");
        assert_eq!(WsSortField::UpdatedAt.to_string(), "updated-at");
        assert_eq!(WsSortField::TfVersion.to_string(), "tf-version");
    }

    #[test]
    fn test_prj_sort_field_display() {
        assert_eq!(PrjSortField::Name.to_string(), "name");
        assert_eq!(PrjSortField::Workspaces.to_string(), "workspaces");
    }

    #[test]
    fn test_run_sort_field_display() {
        assert_eq!(RunSortField::CreatedAt.to_string(), "created-at");
        assert_eq!(RunSortField::Status.to_string(), "status");
        assert_eq!(RunSortField::WsId.to_string(), "ws-id");
    }
}
