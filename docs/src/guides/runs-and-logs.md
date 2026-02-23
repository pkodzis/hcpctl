# Runs and Logs

Monitoring and managing Terraform runs is a common task. `hcpctl` provides commands to view runs, stream logs, and cancel stuck runs.

## Viewing Runs

To list active runs (runs that are not in a final state like `applied` or `errored`) across your organization:

```bash
hcpctl get run --org my-org
```

To view runs for a specific workspace:

```bash
hcpctl get run --workspace my-workspace --org my-org
```

## Streaming Logs

You can stream the logs of a specific run, or the current run of a workspace.

To stream logs for a specific run ID:
```bash
hcpctl logs run-1234567890abcdef
```

To stream logs for the current run of a workspace:
```bash
hcpctl logs --workspace my-workspace --org my-org
```

By default, this streams the `plan` logs. To stream the `apply` logs, use the `-a` or `--apply` flag:
```bash
hcpctl logs --workspace my-workspace --apply --org my-org
```

## Watching a Workspace

If you are waiting for a workspace to trigger a run (e.g., from a VCS webhook), you can use the `watch` command. This will continuously monitor the workspace and automatically stream logs whenever a new run starts.

```bash
hcpctl watch ws my-workspace --org my-org
```

This is incredibly useful for CI/CD pipelines or when you've just pushed a commit and want to see the Terraform output immediately.

## Purging Runs

Sometimes a workspace gets stuck because of a queued run that is waiting for confirmation, or a run that has hung. You can use `purge run` to cancel or discard all pending runs blocking a workspace.

```bash
hcpctl purge run my-workspace --org my-org
```

This will:
1. Find all pending runs (queued, planning, etc.).
2. Ask for confirmation.
3. Cancel actively executing runs and discard queued runs.

You can use `--dry-run` to see what would be cancelled without actually doing it.
