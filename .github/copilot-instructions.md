# hcpctl - AI Development Guidelines

A kubectl-style CLI for HCP Terraform / Terraform Enterprise, written in Rust with clap.

## Tech Stack

- Language: Rust (edition 2021)
- CLI framework: clap 4 (derive) + clap-markdown
- HTTP: reqwest 0.13 (rustls) + tokio async
- Output: comfy-table 7.2, serde_json, serde_yml 0.0.12
- Serialization: serde, base64 0.22
- Utilities: dialoguer 0.12, indicatif 0.18, dirs 6, urlencoding, md-5, futures 0.3, chrono 0.4
- Testing: wiremock 0.6, assert_cmd, predicates, tempfile
- CI: GitHub Actions (release-please + cross-compilation)

## Project Structure

```
src/
  cli/           - CLI argument definitions (clap derive)
    common.rs      - OutputFormat enum
    delete.rs      - Delete subcommand args
    download.rs    - Download subcommand args
    enums.rs       - Shared enums (sort fields, subresources)
    get.rs         - Get subcommand args (all resources)
    invite.rs      - Invite subcommand args
    logs.rs        - Logs subcommand args
    purge.rs       - Purge subcommand args
    set.rs         - Set subcommand args
    tag.rs         - Tag subcommand args (set/get/delete tag ws|prj)
    watch.rs       - Watch subcommand args
  config.rs      - Configuration constants
  error.rs       - Custom error types (TfeError)
  hcp/           - Core logic, one submodule per resource
    client.rs      - TfeClient HTTP client + fetch_all_pages
    credentials.rs - Token resolution (CLI, env, file)
    host.rs        - Host resolution (CLI, env, file)
    helpers.rs     - Multi-org parallel fetch utilities
    traits.rs      - TfeResource, PaginatedResponse traits
    configuration_versions/ - Config version download (models, api, commands)
    logs/          - Log streaming (commands)
    oauth_clients/ - OAuth/VCS connections (models, api, commands)
    org_memberships/ - Organization members (models, api, commands)
    organizations/ - Organizations (models, api, commands)
    projects/      - Projects (models, api, commands, resolver)
    runs/          - Runs (models, api, commands, log_utils)
    state/         - State purge (models, api, commands)
    tags/          - Tag bindings for ws/prj (models, api, commands)
    teams/         - Teams (models, api, commands)
    watch/         - Workspace watching (commands)
    workspaces/    - Workspaces (models, api, commands, resolver, set_api, set_commands)
  output/        - Formatters per resource (table/csv/json/yaml)
  ui/            - Spinners (indicatif), confirmation prompts (dialoguer)
  update/        - Self-update checker and command
tests/
  cli_tests.rs   - Integration tests (binary execution)
```

## Top Rules (always apply)

1. Pattern reuse is mandatory - before writing new code, find how existing resources implement the same feature and copy the structure exactly.
2. kubectl style - hcpctl get ws like kubectl get pods, -o yaml/json/table/csv, --subresource, aliases.
3. No code duplication - one function with optional params, use query structs, extract helpers, use traits.
4. Code review mindset - verify: idiomatic Rust/clap? duplicates? right file? efficient API usage? consistent? extensible?
5. Tests always - every feature has tests, run cargo test and cargo clippy -- -D warnings after changes.
6. Module locality - functions for a resource belong in that resource's module. E.g. resolve_project goes in hcp/projects/, not in the calling command module like set/.

## Key Conventions

- Resource aliases: ws/workspace/workspaces, prj/prjs/project/projects, org/orgs/organization/organizations, oc/oauth-client/oauth-clients, run/runs, team/teams, org-member/org-members/orgmember/orgmembers, tag/tags
- Output formats: table (default), json, yaml, csv
- Commands: get, delete, purge, download, logs, watch, invite, set, update
- `set` command philosophy: `set <resource>` modifies specific settings of that resource (e.g. `set ws` assigns to project, future: execution-mode, working-dir, etc.). It is the generic "modify resource properties" verb — new settings will be added as flags over time.
- Tag bindings are a separate TFE API subresource, so they use `set|get|delete tag ws|prj` rather than flags on `set ws|prj`. This keeps tag CRUD (add/list/remove key-value pairs) cleanly separated from resource property modifications.
- Global flags: -H/--host, -t/--token, -b/--batch, --no-header, -l/--log-level
- Per-resource flags: --org, -f/--filter, -o/--output, -s/--sort, -r/--reverse
- Environment variables: HCP_TOKEN, TFC_TOKEN, TFE_TOKEN, TFE_HOSTNAME
