# Command-Line Help for `hcpctl`

This document contains the help content for the `hcpctl` command-line program.

**Command Overview:**

* [`hcpctl`↴](#hcpctl)
* [`hcpctl get`↴](#hcpctl-get)
* [`hcpctl get org`↴](#hcpctl-get-org)
* [`hcpctl get prj`↴](#hcpctl-get-prj)
* [`hcpctl get ws`↴](#hcpctl-get-ws)
* [`hcpctl get oc`↴](#hcpctl-get-oc)
* [`hcpctl get run`↴](#hcpctl-get-run)
* [`hcpctl get team`↴](#hcpctl-get-team)
* [`hcpctl get org-member`↴](#hcpctl-get-org-member)
* [`hcpctl delete`↴](#hcpctl-delete)
* [`hcpctl delete org-member`↴](#hcpctl-delete-org-member)
* [`hcpctl purge`↴](#hcpctl-purge)
* [`hcpctl purge state`↴](#hcpctl-purge-state)
* [`hcpctl purge run`↴](#hcpctl-purge-run)
* [`hcpctl logs`↴](#hcpctl-logs)
* [`hcpctl watch`↴](#hcpctl-watch)
* [`hcpctl watch ws`↴](#hcpctl-watch-ws)
* [`hcpctl invite`↴](#hcpctl-invite)
* [`hcpctl update`↴](#hcpctl-update)

## `hcpctl`

Explore HCP Terraform / Terraform Enterprise resources

**Usage:** `hcpctl [OPTIONS] <COMMAND>`

HOST RESOLUTION:
  The host is resolved in the following order (first match wins):
  1. CLI argument (-H, --host)
  2. Environment variable: TFE_HOSTNAME
  3. Credentials file (~/.terraform.d/credentials.tfrc.json):
     - If 1 host configured: use it automatically
     - If multiple hosts: interactive selection (or error in batch mode)

TOKEN RESOLUTION:
  The API token is resolved in the following order (first match wins):
  1. CLI argument (-t, --token)
  2. Environment variables (in order): HCP_TOKEN, TFC_TOKEN, TFE_TOKEN
  3. Credentials file (~/.terraform.d/credentials.tfrc.json)
     Token is read from the entry matching the resolved host.

EXAMPLES:
  - hcpctl get org                     List all organizations
  - hcpctl get ws --org myorg          List workspaces in organization
  - hcpctl get ws myws --org myorg     Get workspace details
  - hcpctl -H app.terraform.io get ws  Use specific host

###### **Subcommands:**

* `get` — Get resources (organizations, projects, workspaces)
* `delete` — Delete resources
* `purge` — Purge resources (destructive operations with mandatory confirmation)
* `logs` — View logs for a run (plan or apply)
* `watch` — Watch resources for changes
* `invite` — Invite a user to an organization
* `update` — Update hcpctl to the latest version

###### **Options:**

* `-H`, `--host <HOST>` — TFE/HCP host URL (falls back to TFE_HOSTNAME env var or credentials file)
* `-t`, `--token <TOKEN>` — API token (overrides env vars and credentials file)
* `-l`, `--log-level <LOG_LEVEL>` — Log level (error, warn, info, debug, trace)

  Default value: `warn`
* `-b`, `--batch` — Batch mode - no interactive prompts, no spinners

  Default value: `false`
* `--no-header` — Omit header row in table/CSV output

  Default value: `false`



## `hcpctl get`

Get resources (organizations, projects, workspaces)

**Usage:** `hcpctl get <COMMAND>`

###### **Subcommands:**

* `org` — Get organizations
* `prj` — Get projects
* `ws` — Get workspaces
* `oc` — Get OAuth clients (VCS connections)
* `run` — Get runs (active runs by default - non_final states)
* `team` — Get teams in an organization
* `org-member` — Get organization members



## `hcpctl get org`

Get organizations

**Usage:** `hcpctl get org [OPTIONS] [NAME]`

**Command Aliases:** `orgs`, `organization`, `organizations`

###### **Arguments:**

* `<NAME>` — Organization name (if specified, shows details for that organization)

###### **Options:**

* `-f`, `--filter <FILTER>` — Filter organizations by name (substring match)
* `-o`, `--output <OUTPUT>` — Output format

  Default value: `table`

  Possible values:
  - `table`:
    ASCII table (default)
  - `csv`:
    Comma-separated values
  - `json`:
    JSON array
  - `yaml`:
    YAML format




## `hcpctl get prj`

Get projects

**Usage:** `hcpctl get prj [OPTIONS] [NAME]`

**Command Aliases:** `prjs`, `project`, `projects`

###### **Arguments:**

* `<NAME>` — Project name or ID (if specified, shows details for that project)

###### **Options:**

* `--org <ORG>` — Organization name (required for single project, optional for list)
* `-f`, `--filter <FILTER>` — Filter projects by name (substring match)
* `-o`, `--output <OUTPUT>` — Output format

  Default value: `table`

  Possible values:
  - `table`:
    ASCII table (default)
  - `csv`:
    Comma-separated values
  - `json`:
    JSON array
  - `yaml`:
    YAML format

* `-s`, `--sort <SORT>` — Sort results by field

  Default value: `name`

  Possible values:
  - `name`:
    Sort by project name (default)
  - `workspaces`:
    Sort by workspace count

* `-r`, `--reverse` — Reverse sort order (descending)

  Default value: `false`
* `--no-group-org` — Disable grouping by organization

  Default value: `false`
* `--with-ws` — Include workspace information (count, names, IDs)

  Default value: `false`
* `--with-ws-names` — Show workspace names column (implies --with-ws)

  Default value: `false`
* `--with-ws-ids` — Show workspace IDs column (implies --with-ws)

  Default value: `false`
* `--with-ws-details` — Show workspaces as "name (id)" format (implies --with-ws)

  Default value: `false`



## `hcpctl get ws`

Get workspaces

**Usage:** `hcpctl get ws [OPTIONS] [NAME]`

**Command Aliases:** `workspace`, `workspaces`

###### **Arguments:**

* `<NAME>` — Workspace name or ID (if specified, shows details for that workspace)

###### **Options:**

* `--org <ORG>` — Organization name (required for single workspace, optional for list)
* `-p`, `--prj <PRJ>` — Filter by project (name or ID)
* `-f`, `--filter <FILTER>` — Filter workspaces by name (substring match)
* `-o`, `--output <OUTPUT>` — Output format (defaults to yaml when --subresource is used)

  Default value: `table`

  Possible values:
  - `table`:
    ASCII table (default)
  - `csv`:
    Comma-separated values
  - `json`:
    JSON array
  - `yaml`:
    YAML format

* `-s`, `--sort <SORT>` — Sort results by field

  Default value: `name`

  Possible values:
  - `name`:
    Sort by workspace name (default)
  - `resources`:
    Sort by resource count
  - `updated-at`:
    Sort by last update time
  - `tf-version`:
    Sort by Terraform version

* `-r`, `--reverse` — Reverse sort order (descending)

  Default value: `false`
* `--no-group-org` — Disable grouping by organization

  Default value: `false`
* `--group-by-prj` — Enable grouping by project (can be combined with org grouping)

  Default value: `false`
* `--subresource <SUBRESOURCE>` — Fetch a related subresource (run=current-run, state=current-state-version, config=current-configuration-version, assessment=current-assessment-result). Only works with single workspace lookup and JSON/YAML output

  Possible values:
  - `run`:
    Current run (current-run)
  - `state`:
    Current state version (current-state-version)
  - `config`:
    Current configuration version (current-configuration-version)
  - `assessment`:
    Current assessment result (current-assessment-result)




## `hcpctl get oc`

Get OAuth clients (VCS connections)

**Usage:** `hcpctl get oc [OPTIONS] [NAME]`

**Command Aliases:** `oauth-client`, `oauth-clients`, `oauthclient`, `oauthclients`

###### **Arguments:**

* `<NAME>` — OAuth client name or ID (if specified, shows details for that client)

###### **Options:**

* `--org <ORG>` — Organization name (required for single client, optional for list)
* `-f`, `--filter <FILTER>` — Filter OAuth clients by name (substring match)
* `-o`, `--output <OUTPUT>` — Output format

  Default value: `table`

  Possible values:
  - `table`:
    ASCII table (default)
  - `csv`:
    Comma-separated values
  - `json`:
    JSON array
  - `yaml`:
    YAML format




## `hcpctl get run`

Get runs (active runs by default - non_final states)

**Usage:** `hcpctl get run [OPTIONS] [NAME]`

**Command Alias:** `runs`

NOTE: This command shows only active (non-final) runs.
Use --status to filter by specific non-final statuses (e.g. planning,applying).
Completed runs (applied, errored, canceled) are not shown.

###### **Arguments:**

* `<NAME>` — Run ID (if specified, shows details for that run)

###### **Options:**

* `--org <ORG>` — Organization name (lists runs across org workspaces)
* `--ws <WS>` — Workspace ID (lists runs for specific workspace, must start with ws-)
* `--workspace-names <WORKSPACE_NAMES>` — Filter by workspace names (comma-separated, only with --org)
* `--status <STATUS>` — Filter by specific non-final run statuses (comma-separated). Valid values: pending, fetching, queuing, plan_queued, planning, planned, cost_estimating, cost_estimated, policy_checking, policy_override, policy_soft_failed, policy_checked, confirmed, post_plan_running, post_plan_completed, applying, apply_queued
* `-o`, `--output <OUTPUT>` — Output format

  Default value: `table`

  Possible values:
  - `table`:
    ASCII table (default)
  - `csv`:
    Comma-separated values
  - `json`:
    JSON array
  - `yaml`:
    YAML format

* `--subresource <SUBRESOURCE>` — Fetch a related subresource (events, plan, apply). Requires run ID

  Possible values:
  - `events`:
    Run events (run-events)
  - `plan`:
    Plan details with log access
  - `apply`:
    Apply details with log access

* `--get-log` — Download and display the full log (requires --subresource plan or apply)

  Default value: `false`
* `--tail-log` — Tail the log in real-time until completion (requires --subresource plan or apply)

  Default value: `false`
* `--raw` — Output raw log without parsing (default: extract @message from JSON lines)

  Default value: `false`
* `-s`, `--sort <SORT>` — Sort results by field (default: created-at, newest first)

  Default value: `created-at`

  Possible values:
  - `created-at`:
    Sort by creation time (default: newest first)
  - `status`:
    Sort by status
  - `ws-id`:
    Sort by workspace ID

* `-r`, `--reverse` — Reverse sort order

  Default value: `false`
* `-y`, `--yes` — Skip confirmation prompt when results exceed 100

  Default value: `false`



## `hcpctl get team`

Get teams in an organization

**Usage:** `hcpctl get team [OPTIONS] [NAME]`

**Command Alias:** `teams`

###### **Arguments:**

* `<NAME>` — Team name or ID (if specified, shows details for that team)

###### **Options:**

* `--org <ORG>` — Organization name (required)
* `-f`, `--filter <FILTER>` — Filter teams by name (substring match)
* `-o`, `--output <OUTPUT>` — Output format

  Default value: `table`

  Possible values:
  - `table`:
    ASCII table (default)
  - `csv`:
    Comma-separated values
  - `json`:
    JSON array
  - `yaml`:
    YAML format




## `hcpctl get org-member`

Get organization members

**Usage:** `hcpctl get org-member [OPTIONS] [ID]`

**Command Aliases:** `org-members`, `orgmember`, `orgmembers`

###### **Arguments:**

* `<ID>` — Membership ID (ou-xxx) - if specified, shows details for that membership

###### **Options:**

* `--org <ORG>` — Organization name (if not specified, lists members from all organizations)
* `-f`, `--filter <FILTER>` — Filter by email (substring match)
* `--status <STATUS>` — Filter by status (active, invited)
* `-o`, `--output <OUTPUT>` — Output format

  Default value: `table`

  Possible values:
  - `table`:
    ASCII table (default)
  - `csv`:
    Comma-separated values
  - `json`:
    JSON array
  - `yaml`:
    YAML format




## `hcpctl delete`

Delete resources

**Usage:** `hcpctl delete <COMMAND>`

###### **Subcommands:**

* `org-member` — Delete organization member (remove from organization)



## `hcpctl delete org-member`

Delete organization member (remove from organization)

**Usage:** `hcpctl delete org-member [OPTIONS] <ID>`

**Command Aliases:** `org-members`, `orgmember`, `orgmembers`

###### **Arguments:**

* `<ID>` — Membership ID (ou-xxx) or email address to delete

     ou-xxx   Membership ID - deletes directly
     email    Email address - requires --org to identify the membership

###### **Options:**

* `--org <ORG>` — Organization name (required when argument is an email)
* `-y`, `--yes` — Skip confirmation prompt

  Default value: `false`



## `hcpctl purge`

Purge resources (destructive operations with mandatory confirmation)

These operations are IRREVERSIBLE and always require interactive confirmation.
The --batch flag is ignored for purge commands.

**Usage:** `hcpctl purge <COMMAND>`

###### **Subcommands:**

* `state` — Purge all resources from a workspace's Terraform state
* `run` — Cancel/discard pending runs blocking a workspace



## `hcpctl purge state`

Purge all resources from a workspace's Terraform state

This is a DESTRUCTIVE operation that removes all resources from the state.
The actual infrastructure will NOT be destroyed, but Terraform will
"forget" about the resources, making them orphaned.

PROCEDURE:
  1. Fetches workspace info and validates it exists
  2. Fetches current state version metadata
  3. Displays warning and requires confirmation (type workspace ID)
  4. LOCKS the workspace to prevent concurrent modifications
  5. Downloads the current Terraform state file
  6. Creates a new empty state (preserving lineage, incrementing serial)
  7. Uploads the empty state as a new state version
  8. UNLOCKS the workspace (always, even on error)

SAFETY:
  - ALWAYS requires interactive confirmation (--batch is ignored)
  - Requires exact workspace ID (ws-xxx), NOT workspace name
  - Workspace is locked during the entire operation
  - If upload fails, workspace is still unlocked
  - Original state lineage is preserved for consistency

USE CASES:
  - Cleaning up a workspace before deletion
  - Resetting state after manual infrastructure changes
  - Preparing for re-import of resources
  - Removing orphaned resources from state

WARNING:
  This operation is IRREVERSIBLE without manual state recovery.
  Cloud resources will continue to exist but will no longer be
  tracked by Terraform.

**Usage:** `hcpctl purge state <WORKSPACE_ID>`

###### **Arguments:**

* `<WORKSPACE_ID>` — Workspace ID (ws-xxx) to purge state from

   Must be the exact workspace ID, not the workspace name.
   You can find the workspace ID using: hcpctl get ws NAME --org ORG -o json



## `hcpctl purge run`

Cancel/discard pending runs blocking a workspace

Cancels or discards all pending runs that are blocking a workspace,
including the current run holding the workspace lock if applicable.

PROCEDURE:
  1. Resolves workspace by name or ID (auto-discovers organization)
  2. Fetches all pending runs and current run
  3. Displays summary table with run details
  4. Requires user confirmation
  5. Processes runs: pending first (newest→oldest), then current run
  6. Uses appropriate action (cancel/discard) based on run state

ACTIONS:
  - cancel: Interrupts actively executing run (planning/applying)
  - discard: Skips run waiting for confirmation or priority

USE CASES:
  - Clearing stacked pending runs from CI/CD
  - Unblocking workspace stuck on failed/abandoned run
  - Cleaning up runs before workspace maintenance

NOTES:
  - Use --dry-run to preview without making changes
  - Workspace name can be used (auto-discovers organization)
  - Workspace ID (ws-xxx) can also be used directly

**Usage:** `hcpctl purge run [OPTIONS] <WORKSPACE>`

**Command Alias:** `runs`

###### **Arguments:**

* `<WORKSPACE>` — Workspace name or ID (ws-xxx) to purge runs from

   Can be either:
   - Workspace name (e.g., "my-workspace") - requires --org or auto-discovery
   - Workspace ID (e.g., "ws-abc123") - organization auto-detected

###### **Options:**

* `-o`, `--org <ORG>` — Organization name (auto-detected if not provided)
* `--dry-run` — Preview what would be canceled without making changes



## `hcpctl logs`

View logs for a run (plan or apply)

Target can be:
  run-xxx  Run ID - directly fetches logs for that run
  ws-xxx   Workspace ID - fetches current-run logs
  name     Workspace name - fetches current-run logs (requires --org)

**Usage:** `hcpctl logs [OPTIONS] <TARGET>`

**Command Alias:** `log`

###### **Arguments:**

* `<TARGET>` — Run ID (run-xxx), workspace ID (ws-xxx), or workspace name

     run-xxx  directly fetches logs for that run
     ws-xxx   fetches logs for workspace's current run
     name     workspace name, fetches current run (requires --org)

###### **Options:**

* `-O`, `--org <ORG>` — Organization name (required when target is a workspace name)
* `-a`, `--apply` — Show apply log instead of plan log (default: plan)

  Default value: `false`
* `-f`, `--follow` — Follow log output in real-time until completion (like tail -f)

  Default value: `false`
* `--raw` — Output raw log without parsing (default: extract @message from JSON lines)

  Default value: `false`



## `hcpctl watch`

Watch resources for changes

**Usage:** `hcpctl watch <COMMAND>`

###### **Subcommands:**

* `ws` — Watch a workspace for new runs and stream their logs



## `hcpctl watch ws`

Watch a workspace for new runs and stream their logs

Continuously monitors a workspace for new runs. When a new run starts,
automatically streams its logs until completion, then watches for the
next run. Logs are prefixed with [run-xxx] by default.

**Usage:** `hcpctl watch ws [OPTIONS] <TARGET>`

**Command Alias:** `workspace`

###### **Arguments:**

* `<TARGET>` — Workspace ID (ws-xxx) or workspace name

     ws-xxx   Workspace ID - watches directly
     name     Workspace name - requires --org or auto-discovery

###### **Options:**

* `-O`, `--org <ORG>` — Organization name (optional - will search all orgs if not specified)
* `-a`, `--apply` — Show apply logs instead of plan logs (default: plan)

  Default value: `false`
* `--no-prefix` — Disable [run-xxx] prefix on log output (default: prefix enabled)

  Default value: `false`
* `-i`, `--interval <INTERVAL>` — Poll interval in seconds (default: 3)

  Default value: `3`
* `--raw` — Output raw log without parsing (default: extract @message from JSON lines)

  Default value: `false`



## `hcpctl invite`

Invite a user to an organization

**Usage:** `hcpctl invite [OPTIONS] --email <EMAIL> --org <ORG>`

###### **Options:**

* `--email <EMAIL>` — Email address of user to invite
* `--org <ORG>` — Organization name to invite user to
* `--teams <TEAMS>` — Team ID(s) to add user to (comma-separated, e.g. team-xxx,team-yyy)
* `-o`, `--output <OUTPUT>` — Output format

  Default value: `table`

  Possible values:
  - `table`:
    ASCII table (default)
  - `csv`:
    Comma-separated values
  - `json`:
    JSON array
  - `yaml`:
    YAML format




## `hcpctl update`

Update hcpctl to the latest version

**Usage:** `hcpctl update`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

