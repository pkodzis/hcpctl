//! Logs command arguments

use clap::Parser;
use std::path::PathBuf;

/// Parse a positive integer (digits only, at least 1)
fn parse_positive_count(value: &str) -> Result<usize, String> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("'{}' is not a positive integer", value));
    }
    match value.parse::<usize>() {
        Ok(0) => Err("count must be at least 1".to_string()),
        Ok(n) => Ok(n),
        Err(_) => Err(format!("'{}' is too large", value)),
    }
}

/// Arguments for 'logs' command
#[derive(Parser, Debug)]
pub struct LogsArgs {
    /// Run ID (run-xxx), workspace ID (ws-xxx), or workspace name
    ///
    ///   - run-xxx  directly fetches logs for that run (not allowed with --runs)
    ///   - ws-xxx   fetches logs for workspace's current run
    ///   - name     workspace name, fetches current run (organization from
    ///     --org, the active context, or auto-discovery)
    #[arg(verbatim_doc_comment)]
    pub target: String,

    /// Organization name (otherwise use active context or auto-discovery)
    #[arg(short = 'O', long)]
    pub org: Option<String>,

    /// Show apply log instead of plan log (default: plan)
    #[arg(short = 'a', long, default_value_t = false)]
    pub apply: bool,

    /// Follow log output in real-time until completion (like tail -f)
    #[arg(short = 'f', long, default_value_t = false)]
    pub follow: bool,

    /// Output raw log without parsing (default: extract @message from JSON lines)
    #[arg(long, default_value_t = false)]
    pub raw: bool,

    /// Bulk mode: save logs of the newest COUNT workspace runs (by creation time)
    ///
    /// Files go to --output-dir (default: current directory). Runs are numbered oldest first among the selected
    /// runs. Saves plan logs (or apply logs with --apply); use --all-phases for both.
    #[arg(long, value_name = "COUNT", value_parser = parse_positive_count, conflicts_with = "follow")]
    pub runs: Option<usize>,

    /// With --runs: download both plan and apply logs of each run
    #[arg(
        long,
        default_value_t = false,
        requires = "runs",
        conflicts_with = "apply"
    )]
    pub all_phases: bool,

    /// With --runs: existing directory to write
    /// <workspace>-<number>-<run-id>-<plan|apply>.txt files into
    #[arg(long, value_name = "DIR", requires = "runs", default_value = ".")]
    pub output_dir: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cli;

    #[test]
    fn bulk_flag_combinations() {
        for flags in [
            vec![],
            vec!["--all-phases"],
            vec!["--apply"],
            vec!["--raw"],
            vec!["--apply", "--raw"],
        ] {
            let mut args = vec![
                "hcpctl",
                "logs",
                "workspace",
                "--runs",
                "3",
                "--output-dir",
                ".",
            ];
            args.extend(flags);
            assert!(Cli::try_parse_from(args).is_ok());
        }
        assert!(Cli::try_parse_from(["hcpctl", "logs", "workspace", "--runs", "3"]).is_ok());
        for flags in [
            vec!["--output-dir", "."],
            vec!["--all-phases"],
            vec![
                "--runs",
                "3",
                "--output-dir",
                ".",
                "--all-phases",
                "--apply",
            ],
            vec!["--runs", "3", "--output-dir", ".", "--follow"],
        ] {
            let mut args = vec!["hcpctl", "logs", "workspace"];
            args.extend(flags);
            assert!(Cli::try_parse_from(args).is_err());
        }
        for count in [
            "0",
            "-1",
            "abc",
            "",
            "+1",
            "1.5",
            "999999999999999999999999999999999999999999",
        ] {
            assert!(Cli::try_parse_from([
                "hcpctl",
                "logs",
                "workspace",
                "--runs",
                count,
                "--output-dir",
                "."
            ])
            .is_err());
        }
    }
}
