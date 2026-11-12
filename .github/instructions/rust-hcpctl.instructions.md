---
name: 'Rust hcpctl Conventions'
description: 'Rust coding conventions, clap patterns, and code organization rules for hcpctl'
applyTo: 'src/**/*.rs'
---

# Rust & Clap Conventions for hcpctl

## Clap Best Practices

- Use idiomatic clap derive features:
  - `default_value_if` with `ArgPredicate::IsPresent` for conditional defaults
  - `after_help` for documentation at the bottom of short and long help (`AFTER_LONG_HELP` in `src/cli/mod.rs`); use `after_long_help` for long help only
  - `value_enum` for enums
  - `visible_alias` for kubectl-style short resource names
  - `verbatim_doc_comment` to preserve formatting in help text
- All `use` imports at top of file, never inside functions
- Proper error handling with `Result` and the project's custom `TfeError` type

## Code Organization (per resource module)

- `models.rs` - Data structures: DTOs, query params, response types, serde attrs, `TfeResource` + `PaginatedResponse` impls. Example: `src/hcp/workspaces/models.rs`
- `api.rs` - HTTP/API logic as `impl TfeClient` methods. Example: `src/hcp/workspaces/api.rs`
- `commands.rs` - CLI command handlers, user-facing orchestration. Example: `src/hcp/workspaces/commands.rs`
- `mod.rs` - Public exports only, no logic. Example: `src/hcp/workspaces/mod.rs`
- `resolver.rs` - Optional: shared resolution logic (by ID or name). Examples: `src/hcp/workspaces/resolver.rs`, `src/hcp/projects/resolver.rs`
- `set_api.rs` - Optional: write/update API methods (separated from read API). Example: `src/hcp/workspaces/set_api.rs`
- `set_commands.rs` - Optional: command handlers for set/update operations. Example: `src/hcp/workspaces/set_commands.rs`
- `log_utils.rs` - Optional: log streaming utilities. Example: `src/hcp/runs/log_utils.rs`

## Module Locality

Functions for a resource MUST live in that resource's module, not in the calling command module.

- `resolve_project()` → `src/hcp/projects/`, NOT `src/cli/set.rs`
- `resolve_workspace()` → `src/hcp/workspaces/`, NOT `src/cli/get.rs`
- Calling modules import and call these functions, they never reimplement them locally

## API Efficiency

- **ALWAYS analyze API documentation first** before implementing any API-related feature
- **Server-side filtering** — use API query parameters (`search[name]`, `filter[*]`, `q=`)
- Never fetch all data and filter locally when API supports filtering
- Respect pagination — use `fetch_all_pages` helper
- Minimize API calls — parallel fetch with `fetch_from_organizations`

## Pattern Checklist (verify before implementing)

- Output formatting (`src/output/workspaces.rs`): copy `comfy_table`, `SerializableX` struct, `escape_csv`
- Paginated API list (`src/hcp/client.rs`): use `fetch_all_pages::<T, R>()` helper
- Models (`src/hcp/workspaces/models.rs`, `src/hcp/traits.rs`): serde renames, `TfeResource` impl, generic `ApiListResponse<T>`
- Commands (`src/hcp/workspaces/commands.rs`): spinner usage, filter logic, output delegation
- Multi-org fetch (`src/hcp/helpers.rs`): `fetch_from_organizations`, `collect_org_results`
- Query options (`src/hcp/workspaces/models.rs`): `WorkspaceQuery` struct pattern
- Resource resolution (`src/hcp/workspaces/resolver.rs`): resolve by ID or name pattern
- Set/update API (`src/hcp/workspaces/set_api.rs`): separate write API methods from read
- Log streaming (`src/hcp/runs/log_utils.rs`): log parsing and streaming utilities

## Extensibility

- Design for easy addition of new resources/commands
- Consistent patterns across all resource types
- Use `TfeResource` trait for common behavior (id, name, matches)
- Prefer the generic `ApiListResponse<T>` (`src/hcp/traits.rs`) for list endpoints; use a custom response when the endpoint's shape requires it. Any response passed to `fetch_all_pages` must implement `PaginatedResponse`.

## Command Design Philosophy

- **`set <resource>`** is the generic "modify resource properties" verb — each new setting becomes a flag on the existing `set ws|prj` command (e.g. `--prj` today, future: `--execution-mode`, `--working-dir`, etc.)
- **Separate subresources** (like TFE tag-bindings) that have their own CRUD lifecycle get their own top-level resource under each verb: `set|get|delete tag ws|prj`. This keeps argument signatures clean and avoids overloading `set ws` with unrelated operations.
- Reference: `src/hcp/tags/` for subresource-as-command pattern, `src/hcp/workspaces/set_commands.rs` for property-as-flag pattern.
