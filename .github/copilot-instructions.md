# AI Development Guidelines for hcpctl

Read and follow these guidelines when working on this project.

## 0. CRITICAL: Pattern Reuse is MANDATORY

**This is the most important rule. Violating it creates technical debt and wastes time.**

Before writing ANY new code:

1. **STOP and analyze existing codebase** - Search for similar functionality in existing modules
2. **Find the pattern** - Look at how other resources (workspaces, projects, organizations, oauth_clients) implement the same feature
3. **Copy the structure exactly** - File layout, function signatures, output formatting, test structure
4. **DO NOT invent new patterns** - If workspaces uses `comfy_table` for output, teams MUST use `comfy_table`
5. **DO NOT use `println!` with manual formatting** when `comfy_table` exists
6. **DO NOT create inline output functions** when `src/output/` module exists

### Pattern Checklist (verify before implementing):

| Feature | Where to look | What to copy |
|---------|---------------|--------------|
| Output formatting | `src/output/workspaces.rs` | `comfy_table`, `SerializableX` struct, `escape_csv` |
| Paginated API list | `src/hcp/client.rs` | Use `fetch_all_pages<T, R>()` helper |
| Models | `src/hcp/workspaces/models.rs` | Serde attributes, `TfeResource` trait impl, `PaginatedResponse` impl |
| Commands | `src/hcp/workspaces/commands.rs` | Spinner usage, filter logic, output call |
| Tests | `src/hcp/workspaces/api.rs` tests | `wiremock` setup, `create_test_client`, mock structure |

### Pagination Helper (MANDATORY for list endpoints)

Use `TfeClient::fetch_all_pages<T, R>()` for all paginated list endpoints:

```rust
// In models.rs - implement PaginatedResponse trait
impl PaginatedResponse<Team> for TeamsResponse {
    fn into_data(self) -> Vec<Team> { self.data }
    fn meta(&self) -> Option<&PaginationMeta> { self.meta.as_ref() }
}

// In api.rs - use the helper (replaces 50+ lines of pagination loop)
pub async fn get_teams(&self, org: &str) -> Result<Vec<Team>> {
    let path = format!("/{}/{}/{}", api::ORGANIZATIONS, org, api::TEAMS);
    let error_context = format!("teams for organization '{}'", org);

    self.fetch_all_pages::<Team, TeamsResponse>(&path, &error_context).await
}

// With query params:
pub async fn get_workspaces(&self, org: &str, query: WorkspaceQuery<'_>) -> Result<Vec<Workspace>> {
    let mut path = format!("/{}/{}/{}", api::ORGANIZATIONS, org, api::WORKSPACES);
    if let Some(s) = query.search {
        path.push_str(&format!("?search[name]={}", urlencoding::encode(s)));
    }
    self.fetch_all_pages::<Workspace, WorkspacesResponse>(&path, &error_context).await
}
```

**If you're about to write 10+ lines of code without checking existing patterns first, you're doing it wrong.**

## 1. CLI Design - kubectl Style

- **Syntax must mirror kubectl** as closely as possible
- Flag names, behavior, and conventions should match kubectl patterns
- Examples:
  - `hcpctl get ws` like `kubectl get pods`
  - `--subresource` like `kubectl get pod X -o jsonpath='{.status}'`
  - `-o yaml/json/table` for output formats

## 2. Rust & Clap Best Practices

- Use idiomatic clap features:
  - `default_value_if` with `ArgPredicate::IsPresent` for conditional defaults
  - `after_long_help` for documentation sections at bottom of help
  - `value_enum` for enums
- All `use` imports at top of file, never inside functions
- Proper error handling with `Result` and custom error types

## 3. Code Organization

- **models.rs** - Data structures (DTOs, query params, response types)
- **api.rs** - HTTP/API logic, `impl TfeClient` methods
- **commands.rs** - CLI command handlers, user-facing logic
- **mod.rs** - Public exports only

## 4. No Code Duplication (DRY)

- One function with optional parameters instead of multiple variants
- Use structs for query options (e.g., `WorkspaceQuery`) instead of many function args
- Extract common patterns into helpers
- If copying code, stop and refactor
- **When adding new code, analyze existing codebase for similar patterns**:
  - Is there already a function/struct doing something similar?
  - Would a trait make sense to unify behavior across types?
  - Can this be generalized for reuse by other resources?

## 5. API Efficiency

- **ALWAYS analyze API documentation first** - Before implementing any API-related feature:
  1. Read and understand the API documentation
  2. Identify available endpoints, query parameters, and filters
  3. Understand response structure and pagination
  4. Plan implementation based on API capabilities
- **Server-side filtering** - Use API query parameters (`search[name]`, `filter[*]`, `q=`)
- Never fetch all data and filter locally when API supports filtering
- Respect pagination
- Minimize API calls

## 6. Extensibility

- Design for easy addition of new resources/commands
- Consistent patterns across all resource types (ws, prj, org, oauth)
- Traits for common behavior (`TfeResource`)

## 7. Code Review Mindset

Before proposing any solution, verify:

1. Is this idiomatic Rust/clap?
2. Does it duplicate existing code?
3. Are data models in the right place?
4. Does it use API efficiently?
5. Is it consistent with kubectl and rest of codebase?
6. Is it ready for future expansion?

**Challenge suboptimal solutions. Propose better patterns.**

## 8. Testing

- **Maximum code coverage is the goal** - every feature should have corresponding tests
- **Always create and update tests** when implementing new features or fixing bugs
- Follow existing test patterns in the codebase
- Unit tests for models and helpers
- Integration tests for CLI parsing (in `tests/cli_tests.rs`)
- **HTTP mocking with `wiremock`** for API tests
- Test edge cases: empty inputs, invalid data, error conditions
- All tests must pass before completing a task
- Run `cargo test` after every change
- Run `cargo clippy` to ensure no warnings

### Wiremock Test Pattern (MANDATORY)

When testing API methods, use this exact pattern:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // IMPORTANT: Create client with mock server URI as base_url
    // DO NOT include /api/v2 prefix in mock paths - client adds it automatically
    fn create_test_client(base_url: &str) -> TfeClient {
        TfeClient::with_base_url(
            "test-token".to_string(),
            "mock.terraform.io".to_string(),
            base_url.to_string(),  // This is the mock server URI
        )
    }

    #[tokio::test]
    async fn test_get_resource() {
        let mock_server = MockServer::start().await;

        // Path is relative - NO /api/v2 prefix!
        Mock::given(method("GET"))
            .and(path("/organizations/my-org/teams"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [...],
                "meta": {
                    "pagination": {
                        "current-page": 1,
                        "total-pages": 1,
                        "total-count": 2
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let result = client.get_teams("my-org").await.unwrap();

        assert_eq!(result.len(), 2);
    }
}
```

**Key rules:**
1. `create_test_client` takes `mock_server.uri()` as base_url
2. Mock paths are WITHOUT `/api/v2` prefix (e.g., `/organizations/...` not `/api/v2/organizations/...`)
3. Always include pagination meta in list responses
4. Test error cases (404, 500) separately
5. Test pagination with multiple pages

### Current Test Coverage (350+ tests total)

**Unit tests (318+):**

- `cli/mod.rs` - CLI parsing, flags, output formats
- `config.rs` - Configuration constants
- `error.rs` - Error types and conversions
- `hcp/client.rs` - HTTP client, base_url
- `hcp/credentials.rs` - Token resolver
- `hcp/host.rs` - Host resolver
- `hcp/helpers.rs` - Multi-org fetch utilities
- `hcp/traits.rs` - TfeResource trait
- `hcp/*/models.rs` - All resource models with deserialization tests
- `hcp/*/api.rs` - API calls with wiremock HTTP mocking
- `output/*.rs` - Output formatters

**Integration tests (32):**

- `tests/cli_tests.rs` - Binary execution, help, aliases, error handling

**Not yet covered (require refactoring):**

- `hcp/*/commands.rs` - Command handlers (complex orchestration, consider e2e tests)

## 9. Current Project Structure

```
src/
├── cli/           # CLI argument definitions (clap)
├── config/        # Configuration constants
├── error/         # Error types
├── hcp/           # Core logic
│   ├── client.rs      # TfeClient HTTP client
│   ├── credentials.rs # Token resolution
│   ├── host.rs        # Host resolution
│   ├── helpers.rs     # Common utilities
│   ├── traits.rs      # TfeResource trait
│   ├── workspaces/    # models.rs, api.rs, commands.rs
│   ├── projects/
│   ├── organizations/
│   └── oauth_clients/
├── output/        # Formatters (table, json, yaml, csv)
└── ui/            # Spinners, user feedback
```

## 10. Key Conventions

- Resource aliases: `ws`, `prj`, `org`, `oauth`
- Output formats: `table` (default), `json`, `yaml`, `csv`
- Flags: `-o/--org`, `-f/--filter`, `-o/--output`, `--no-header`, `--batch`
- Environment variables: `TFE_TOKEN`, `TFE_HOST`, `TFE_ORG`
