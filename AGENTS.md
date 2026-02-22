# hcpctl — Agent Instructions

A kubectl-style CLI for HCP Terraform / Terraform Enterprise, written in Rust (edition 2021) with clap 4 (derive).

## Build & Validate

```bash
# Build
cargo build

# Run all tests (491 unit + 57 integration = 548 total)
cargo test

# Lint — must produce zero warnings
cargo clippy

# Always run both after ANY change
cargo test && cargo clippy
```

No additional setup is needed — `cargo` handles everything. There are no environment variables, Docker, or external services required for building and testing.

## Project Layout

```
src/
  cli/           - CLI argument definitions (clap derive)
  config.rs      - Configuration constants
  error.rs       - Custom error types (TfeError)
  hcp/           - Core logic, one submodule per resource
    client.rs      - TfeClient HTTP client + fetch_all_pages
    credentials.rs - Token resolution
    host.rs        - Host resolution
    helpers.rs     - Multi-org parallel fetch utilities
    traits.rs      - TfeResource, PaginatedResponse traits
    configuration_versions/
    logs/
    oauth_clients/
    org_memberships/
    organizations/
    projects/
    runs/
    state/
    tags/
    teams/
    watch/
    workspaces/
  output/        - Formatters per resource (table/csv/json/yaml)
  ui/            - Spinners, confirmation prompts
  update/        - Self-update checker
tests/
  cli_tests.rs   - Integration tests (binary execution)
```

Each resource module in `hcp/` follows the same structure:

| File | Purpose |
|------|---------|
| `models.rs` | Data structures, serde, `TfeResource` + `PaginatedResponse` impls |
| `api.rs` | HTTP/API logic as `impl TfeClient` methods |
| `commands.rs` | CLI command handlers |
| `mod.rs` | Public exports only |
| `resolver.rs` | Optional: resolution by ID or name |
| `set_api.rs` | Optional: write/update API methods |
| `set_commands.rs` | Optional: set/update command handlers |
| `log_utils.rs` | Optional: log streaming utilities |

## Command Design

- **`set <resource>`** is the generic "modify resource properties" verb. `set ws` currently assigns workspace to project; future additions will add more flags (execution-mode, working-dir, etc.). Each new setting becomes a flag on `set ws|prj`.
- **Tag bindings** are a separate TFE API subresource with their own CRUD, so they are modeled as `set|get|delete tag ws|prj` rather than flags on `set ws`. This cleanly separates tag operations (add/list/remove key-value pairs) from resource property modifications.

## Key Rules for Making Changes

1. **Pattern reuse is mandatory** — before writing new code, find how existing resources implement the same feature and copy the structure exactly. Reference implementation: `src/hcp/workspaces/`.
2. **Module locality** — functions for a resource belong in that resource's module (`hcp/<resource>/`), never in the calling command module.
3. **Use `fetch_all_pages`** for paginated endpoints — never write manual pagination loops.
4. **Use `comfy_table`** for table output — never `println!` with manual formatting.
5. **Use `wiremock`** for API tests — never hit real endpoints. Mock paths are WITHOUT `/api/v2` prefix.
6. **Tests always** — every feature needs unit tests in the source file and integration tests in `tests/cli_tests.rs`.
7. **No code duplication** — use query structs, extract helpers, use traits.

## Wiring a New Resource

When adding a new resource, these files must be updated:

1. `src/hcp/<resource>/` — create `models.rs`, `api.rs`, `commands.rs`, `mod.rs`
2. `src/output/<resource>.rs` — output formatter
3. `src/cli/get.rs` — add `GetResource` variant with `visible_alias`
4. `src/cli/mod.rs` — add re-exports
5. `src/hcp/mod.rs` — add `pub mod` and re-exports
6. `src/lib.rs` — add to `pub use` blocks
7. `src/main.rs` — add match arm in command dispatch

## CI

Single workflow: `.github/workflows/release.yml` — release-please + cross-compilation for 6 targets (linux amd64/musl/arm64, macOS amd64/arm64, Windows amd64). Not triggered by changes to `scripts/`, `*.md`, or `docs/`.

## Trust These Instructions

These instructions reflect the current state of the codebase. Only search the codebase if information here is incomplete or found to be in error.
