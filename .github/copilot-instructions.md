# AI Development Guidelines for hcpctl

Read and follow these guidelines when working on this project.

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

### Current Test Coverage (225 tests total)

**Unit tests (204):**

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

**Integration tests (21):**

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
