//! CLI argument parsing

use clap::{Parser, ValueEnum};

use crate::config::defaults;

/// TFE Workspace Lister CLI
#[derive(Parser, Debug)]
#[command(name = "hcp-cli")]
#[command(version = "0.2.0")]
#[command(about = "List and explore TFE workspaces", long_about = None)]
pub struct Cli {
    /// Organization name (if not specified, lists all accessible organizations)
    #[arg(short, long)]
    pub org: Option<String>,

    /// Filter workspaces by name (substring match)
    #[arg(short, long)]
    pub filter: Option<String>,

    /// TFE host URL
    #[arg(short = 'H', long, default_value = defaults::HOST)]
    pub host: String,

    /// API token (overrides env vars and credentials file)
    #[arg(short = 't', long)]
    pub token: Option<String>,

    /// Log level (error, warn, info, debug, trace)
    #[arg(short, long, default_value = defaults::LOG_LEVEL)]
    pub log_level: String,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,

    /// Sort results by field
    #[arg(short, long, value_enum, default_value_t = SortField::Name)]
    pub sort: SortField,

    /// Disable grouping by organization (sort all results together)
    #[arg(long, default_value_t = false)]
    pub no_group: bool,

    /// Reverse sort order (descending)
    #[arg(short = 'r', long, default_value_t = false)]
    pub reverse: bool,
}

/// Output format options
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// ASCII table (default)
    Table,
    /// Comma-separated values
    Csv,
    /// JSON array
    Json,
}

/// Sort field options
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SortField {
    /// Sort by workspace name (default)
    Name,
    /// Sort by resource count
    Resources,
    /// Sort by last update time
    UpdatedAt,
    /// Sort by Terraform version
    TfVersion,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Table => write!(f, "table"),
            OutputFormat::Csv => write!(f, "csv"),
            OutputFormat::Json => write!(f, "json"),
        }
    }
}

impl std::fmt::Display for SortField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SortField::Name => write!(f, "name"),
            SortField::Resources => write!(f, "resources"),
            SortField::UpdatedAt => write!(f, "updated-at"),
            SortField::TfVersion => write!(f, "tf-version"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_display() {
        assert_eq!(OutputFormat::Table.to_string(), "table");
        assert_eq!(OutputFormat::Csv.to_string(), "csv");
        assert_eq!(OutputFormat::Json.to_string(), "json");
    }

    #[test]
    fn test_cli_default_values() {
        let cli = Cli::parse_from(["hcp-cli"]);
        assert_eq!(cli.host, defaults::HOST);
        assert_eq!(cli.log_level, defaults::LOG_LEVEL);
        assert_eq!(cli.format, OutputFormat::Table);
        assert_eq!(cli.sort, SortField::Name);
        assert!(!cli.no_group);
        assert!(!cli.reverse);
        assert!(cli.org.is_none());
        assert!(cli.filter.is_none());
    }

    #[test]
    fn test_cli_with_org() {
        let cli = Cli::parse_from(["hcp-cli", "-o", "my-org"]);
        assert_eq!(cli.org, Some("my-org".to_string()));
    }

    #[test]
    fn test_cli_with_filter() {
        let cli = Cli::parse_from(["hcp-cli", "-f", "dev"]);
        assert_eq!(cli.filter, Some("dev".to_string()));
    }

    #[test]
    fn test_cli_with_format() {
        let cli = Cli::parse_from(["hcp-cli", "--format", "json"]);
        assert_eq!(cli.format, OutputFormat::Json);
    }

    #[test]
    fn test_cli_with_sort() {
        let cli = Cli::parse_from(["hcp-cli", "-s", "resources"]);
        assert_eq!(cli.sort, SortField::Resources);
    }

    #[test]
    fn test_cli_with_sort_and_reverse() {
        let cli = Cli::parse_from(["hcp-cli", "-s", "updated-at", "-r"]);
        assert_eq!(cli.sort, SortField::UpdatedAt);
        assert!(cli.reverse);
    }

    #[test]
    fn test_cli_with_no_group() {
        let cli = Cli::parse_from(["hcp-cli", "--no-group"]);
        assert!(cli.no_group);
    }

    #[test]
    fn test_cli_all_options() {
        let cli = Cli::parse_from([
            "hcp-cli",
            "-o",
            "my-org",
            "-f",
            "prod",
            "-H",
            "custom.host.com",
            "-l",
            "debug",
            "--format",
            "csv",
            "-s",
            "tf-version",
            "-r",
            "--no-group",
        ]);

        assert_eq!(cli.org, Some("my-org".to_string()));
        assert_eq!(cli.filter, Some("prod".to_string()));
        assert_eq!(cli.host, "custom.host.com");
        assert_eq!(cli.log_level, "debug");
        assert_eq!(cli.format, OutputFormat::Csv);
        assert_eq!(cli.sort, SortField::TfVersion);
        assert!(cli.reverse);
        assert!(cli.no_group);
    }
}
