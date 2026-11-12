# Runs and Logs

Monitoring and managing Terraform runs is a common task. `hcpctl` provides commands to view runs, stream logs, and cancel stuck runs.

## Viewing Runs

To list active runs (runs that are not in a final state like `applied` or `errored`) across your organization:

```bash
hcpctl get run --org my-org
```

To view runs for a specific workspace:

```bash
hcpctl get run --ws ws-1234567890abcdef
```

`--ws` accepts a workspace ID (`ws-...`).

You can also filter by status and workspace names in org scope:

```bash
hcpctl get run --org my-org --status planning,applying
hcpctl get run --org my-org --workspace-names app-prod,network-prod
```

## Bulk Download of Workspace Logs

Use `hcpctl logs my-workspace --runs 3 --all-phases --output-dir ./out` to
save both plan and apply logs of the newest three runs. The directory must already
exist and be writable; hcpctl does not create it. Workspace names and `ws-...` IDs
are supported. Organization resolution uses `--org`, the active context, or
auto-discovery, just as single-workspace commands do.

Bulk target disambiguation is explicit: a `ws-...` target is tried as an ID
first, then as a name only if the ID lookup returns 404. For `run-...` targets,
an exact workspace name in `--org` (or the active context's organization) wins
even if an accessible run has the same ID. If no scoped name matches, an
accessible run ID is rejected. Without organization scope, accessible run IDs
take precedence over auto-discovered names; supply `--org` to select a colliding
workspace name. Authentication and server errors are not treated as absence.
These rules apply only to bulk mode; legacy single-log routing is unchanged.

| Option | Combination rules |
|--------|-------------------|
| `--runs COUNT` | Positive integer; writes to `--output-dir` (default: current directory); incompatible with `--follow` and a run-ID target. |
| `--output-dir DIR` | Requires `--runs`; defaults to `.`; saves plan logs by default, or apply logs with `--apply`. |
| `--all-phases` | Requires `--runs`; saves both phases independently; incompatible with `--apply`. |
| `--raw` | Allowed; preserves original log text instead of extracting JSON `@message` lines. |

Selection includes all statuses, including errored and canceled runs, and all
seven documented operations (including speculative and action-only runs). If
fewer runs exist than requested, all are selected. An empty history is an error.
Selection uses creation time, **not the workspace's current run**: `--runs 1`
can therefore select a different run from `hcpctl logs my-workspace`.

Files are named `<workspace-name>-<number>-<run-id>-<plan|apply>.txt`, using the
actual workspace name even for an ID target. Selected runs are numbered oldest
first, with run ID as the deterministic tie-breaker for identical timestamps.
Numbers have at least two digits (`01`, `99`, `100`); selecting the newest three
of twenty runs numbers those three `01`, `02`, `03`. Unsafe or overlong filenames
are rejected before downloads.

Each success prints `Saved <absolute path>` to stdout. Existing files are
atomically overwritten only after complete retrieval, writing, flushing and
syncing. Failed retrievals or writes preserve existing files and remove temporary
files. New files have private permissions (0600 on Unix).

Ctrl-C (and SIGTERM on Unix) cancels pending requests and stops new downloads.
Active filesystem writes are joined before exit: they either finish an atomic
replacement or remove their temporary file. Cancellation returns a nonzero
status. SIGKILL, power loss, and other uncatchable termination cannot guarantee
temporary-file cleanup. Retry/fallback diagnostics use fallible stderr writes;
a broken reporter does not abandon active downloads or writes. Diagnostics may
be emitted when the next chronological worker completes or preflight ends.
On Unix, pipe and terminal reports use nonblocking readiness-driven writes;
an unread full pipe does not prevent cancellation or worker cleanup. Final
errors and update notifications are best effort with a one-second deadline.
Regular-file output and non-Unix reporting can still be held by an OS-level
blocking write; started blocking filesystem operations cannot be aborted safely.

Unavailable phases (including null log URLs, empty responses and archivist 404s),
retrieval errors and write errors are reported on stderr with the affected run
and phase. Other phases and runs continue. The exit status is nonzero if any
requested log could not be saved; in-progress or plan-only runs can therefore
produce a nonzero result with `--all-phases` even when their plan was saved.

### API usage and compatibility

Bulk mode reads **all workspace history pages**, 100 runs per page, with no
timeframe or status restriction. The workspace endpoint documents that omitting
timeframe returns all runs since creation. This avoids assuming page sort order,
UTC year-filter boundaries, or an accurate local clock. History requests,
including retries, are spaced at least two seconds apart. Very large histories
can consequently take time even when a small count is requested.

The list requests include the requested plan/apply metadata to avoid per-run
lookups. Usable included log URLs are reused; missing metadata, null URLs of any
age, or metadata older than 30 seconds is refreshed through phase lookups before
declaring a phase unavailable. If list includes are
rejected (400/422), hcpctl probes without includes while retaining the complete
operation filter. Rejection of that filter fails explicitly rather than silently
dropping run types. Detectable changes in pagination counts or duplicate IDs
cause a rescan, up to three passes, then an error with no logs downloaded. The
API provides no stable snapshot, so undetectable concurrent changes remain a
limitation.

Downloads use at most four concurrent run workers, with phases sequential within
each run. Reports remain chronological. All bulk API calls, including workspace
resolution and target classification, share a 20-requests-per-second limiter and
a rate-limit pause gate. Blob downloads are unauthenticated and not subject to
the API send limiter. Signed URLs are treated as secrets.

429 responses honor integer `Retry-After`, then decimal `x-ratelimit-reset`
(rounded up), otherwise 60 seconds, with a one-second minimum. Retries are
bounded at 20 and ten minutes of cumulative server-requested waits per request;
larger waits fail immediately rather than being shortened. A 429 pauses workers
and refreshes the blob URL before retrying. Blob 403s refresh once. Transient
502/503/504 responses use five retries with 1, 2, 4, 8, 16-second backoff. Existing
single-run log and follow behavior does not opt in to these bulk retries.

If both target lookups fail, the error explains that a 404 can mean absent or
unauthorized. Target precedence follows the scoped collision rules above.
Use `--org` or a context to avoid organization discovery.

## Streaming Logs

You can view logs of a specific run, or logs from the current run of a workspace.

To stream logs for a specific run ID:
```bash
hcpctl logs run-1234567890abcdef
```

To stream logs for the current run of a workspace (name or `ws-...`):
```bash
hcpctl logs my-workspace --org my-org
```

By default, this streams the `plan` logs. To stream the `apply` logs, use the `-a` or `--apply` flag:
```bash
hcpctl logs my-workspace --apply --org my-org
```

To follow output in real time, add `-f` / `--follow`:

```bash
hcpctl logs run-1234567890abcdef --follow
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

## Related guides

- [Managing Workspaces and Projects](workspaces.md)
- [Authentication and Contexts](authentication.md)
