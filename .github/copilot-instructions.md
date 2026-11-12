# hcpctl - AI Development Guidelines

A kubectl-style CLI for HCP Terraform / Terraform Enterprise, written in Rust with clap.

## Tech Stack

- Language: Rust (edition 2021)
- CLI framework: clap 4 (derive) + clap-markdown
- HTTP: reqwest 0.13 (rustls) + tokio async
- Output: comfy-table 8.0, serde_json, serde_yml 0.0.13
- Serialization: serde, base64 0.23
- Utilities: dialoguer 0.12, indicatif 0.18, dirs 7.0, urlencoding, md-5, futures 0.3, chrono 0.4
- Logging: log + env_logger
- Testing: wiremock 0.6, assert_cmd, predicates, tempfile
- CI: GitHub Actions (release-please + cross-compilation)

## Project Structure

- `src/cli/`: clap arguments. `common.rs` defines `OutputFormat`; `enums.rs` has shared sort and subresource enums. Subcommands: `context.rs`, `delete.rs`, `download.rs`, `get.rs`, `invite.rs`, `logs.rs`, `purge.rs`, `set.rs`, `tag.rs` (set/get/delete tag ws|prj), `team_access.rs` (get team-access), `watch.rs`.
- `src/config.rs`: constants; `src/context/`: named context store, resolution and commands; `src/error.rs`: `TfeError`; `src/lib.rs`: library and re-exports; `src/main.rs`: binary.
- `src/hcp/`: core logic. `client.rs`: `TfeClient` and `fetch_all_pages`; `credentials.rs`: CLI/env/file token resolution; `host.rs`: CLI/env/file host resolution; `helpers.rs`: multi-org parallel fetch; `traits.rs`: `TfeResource`, `PaginatedResponse`.
- Resource modules: `configuration_versions/` (config download), `oauth_clients/` (OAuth/VCS), `org_memberships/` (members), `organizations/`, `projects/`, `runs/`, `state/` (purge), `tags/` (ws/prj bindings), `team_projects/` (access), `teams/`, `workspaces/`: models, API and commands. `projects/` and `workspaces/` also have resolvers; `workspaces/` has `set_api` and `set_commands`; `runs/` has `log_utils`.
- `src/hcp/logs/` and `src/hcp/watch/`: commands for log streaming and workspace watching.
- `src/output/`: per-resource table/CSV/JSON/YAML formatters; `src/ui/`: spinners and confirmation prompts; `src/update/`: self-update checker and command.
- `tests/cli_tests.rs`: binary integration tests.

## Top Rules (always apply)

1. Pattern reuse is mandatory - before writing new code, find how existing resources implement the same feature and copy the structure exactly.
2. kubectl style - hcpctl get ws like kubectl get pods, -o yaml/json/table/csv, --subresource, aliases.
3. No code duplication - one function with optional params, use query structs, extract helpers, use traits.
4. Code review mindset - verify: idiomatic Rust/clap? duplicates? right file? efficient API usage? consistent? extensible?
5. Tests always - every feature has tests, run cargo test and cargo clippy -- -D warnings after changes.
6. Module locality - functions for a resource belong in that resource's module. E.g. resolve_project goes in hcp/projects/, not in the calling command module like set/.
7. README Features table - after adding a new command, resource, or capability, update the `## Features` table in `README.md` to reflect the change. Keep entries sorted by command, then by resource name.

## Key Conventions

- Resource aliases: ws/workspace/workspaces, prj/prjs/project/projects, org/orgs/organization/organizations, oc/oauth-client/oauth-clients, run/runs, team/teams, team-access/team-accesses/teamaccess/ta, org-member/org-members/orgmember/orgmembers, tag/tags
- Output formats: table (default), json, yaml, csv
- Commands: get, delete, purge, download, logs (alias: log), watch, invite, set, config (contexts), update
- `set` command philosophy: `set <resource>` modifies specific settings of that resource (e.g. `set ws` assigns to project, future: execution-mode, working-dir, etc.). It is the generic "modify resource properties" verb — new settings will be added as flags over time.
- Tag bindings are a separate TFE API subresource, so they use `set|get|delete tag ws|prj` rather than flags on `set ws|prj`. This keeps tag CRUD (add/list/remove key-value pairs) cleanly separated from resource property modifications.
- Global flags: -c/--context, -H/--host, -t/--token, -b/--batch, --no-header, -l/--log-level
- Per-resource flags: --org, -f/--filter, -o/--output, -s/--sort, -r/--reverse
- Environment variables: HCP_TOKEN, TFC_TOKEN, TFE_TOKEN, TFE_HOSTNAME
