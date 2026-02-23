---
name: add-resource
description: 'Step-by-step guide for adding a new TFE API resource to hcpctl. Use when implementing a new resource type like teams, policies, variables, registry modules, etc. Covers the full lifecycle: models, API, commands, output, CLI args, tests, and wiring.'
---

# Add a New Resource to hcpctl

This skill guides you through adding a complete new resource to hcpctl, following the exact patterns used by existing resources (workspaces, projects, teams, etc.).

## Prerequisites

Before starting, identify:

- **Resource name** (e.g., `variables`)
- **API endpoint** (e.g., `/api/v2/workspaces/:id/vars`)
- **Short alias** (e.g., `var`)
- **Whether it's org-scoped, workspace-scoped, or both**

## Step 1: Create the Resource Module

Create `src/hcp/<resource>/` with 4 files:

### 1.1 `models.rs` — Data Structures

Reference: [src/hcp/teams/models.rs](../../src/hcp/teams/models.rs)

```rust
use serde::Deserialize;
use crate::hcp::traits::{PaginatedResponse, TfeResource};
use crate::hcp::PaginationMeta;

#[derive(Deserialize, Debug)]
pub struct ResourcesResponse {
    pub data: Vec<Resource>,
    pub meta: Option<PaginationMeta>,
}

impl PaginatedResponse<Resource> for ResourcesResponse {
    fn into_data(self) -> Vec<Resource> { self.data }
    fn meta(&self) -> Option<&PaginationMeta> { self.meta.as_ref() }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Resource {
    pub id: String,
    pub attributes: ResourceAttributes,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ResourceAttributes {
    pub name: String,
    // Use #[serde(rename = "kebab-case")] for API field names
}

impl TfeResource for Resource {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { &self.attributes.name }
}
```

### 1.2 `api.rs` — API Operations

Reference: [src/hcp/teams/api.rs](../../src/hcp/teams/api.rs)

```rust
use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::TfeClient;
use super::models::{Resource, ResourcesResponse};

impl TfeClient {
    pub async fn get_resources(&self, org: &str) -> Result<Vec<Resource>> {
        let path = format!("/{}/{}/{}", api::ORGANIZATIONS, org, "resources");
        let error_context = format!("resources for organization '{}'", org);
        self.fetch_all_pages::<Resource, ResourcesResponse>(&path, &error_context).await
    }
}
```

### 1.3 `commands.rs` — Command Handler

Reference: [src/hcp/teams/commands.rs](../../src/hcp/teams/commands.rs)

Pattern: spinner → fetch → filter → output

### 1.4 `mod.rs` — Public Exports

```rust
mod api;
mod commands;
mod models;

pub use commands::run_resource_command;
pub use models::{Resource, ResourceAttributes};
```

## Step 2: Add Output Formatter

Create `src/output/<resource>.rs`:

Reference: [src/output/teams.rs](../../src/output/teams.rs)

Pattern: `SerializableResource` struct + `output_table/csv/json/yaml` functions using comfy_table.

Register in `src/output/mod.rs`.

## Step 3: Add CLI Arguments

In `src/cli/get.rs`:

- Add variant to `GetResource` enum with `visible_alias`
- Add `ResourceArgs` struct with standard fields: `name`, `org`, `filter`, `output`

Register in `src/cli/mod.rs` re-exports.

## Step 4: Wire Up

1. `src/hcp/mod.rs` — add `pub mod resource;` and re-exports
2. `src/lib.rs` — add to `pub use hcp::...` and `pub use cli::...`
3. `src/main.rs` — add `Command::Get { resource: GetResource::Resource(_) }` arm

## Step 5: Write Tests

1. **Model tests** in `models.rs` — deserialization, trait impls, helper methods
2. **API tests** in `api.rs` — wiremock mocks for success, 404, empty results
3. **Output tests** in `src/output/<resource>.rs` — serialization, empty input
4. **CLI tests** in `tests/cli_tests.rs` — help text, aliases, flag validation

## Step 6: Verify

```bash
cargo test
cargo clippy -- -D warnings
```

All 500+ tests must pass with zero warnings.
