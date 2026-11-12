---
name: 'Test Conventions'
description: 'Testing patterns, wiremock setup, and test organization rules'
applyTo: 'src/**/*.rs,tests/**/*.rs'
---

# Testing Conventions for hcpctl

## Test Layers

- **Unit tests** — `#[cfg(test)] mod tests` in each source
  file. Built-in + serde_json. Models, helpers, pure logic.
- **API mock tests** — `#[cfg(test)] mod tests` in `api.rs`
  files. wiremock + tokio::test. HTTP calls with mock server.
- **Integration tests** — `tests/cli_tests.rs`.
  assert_cmd + predicates. Binary execution, help text,
  error UX.

## Mandatory Rules

- **Maximum code coverage is the goal** — every feature
  should have corresponding tests
- **Always create and update tests** when implementing
  new features or fixing bugs
- Run `cargo test` after every change
- Run `cargo clippy -- -D warnings` to ensure no warnings
- Test edge cases: empty inputs, invalid data, error conditions (404, 500)

## Wiremock Test Pattern (MANDATORY for api.rs)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_get_resource() {
        let mock_server = MockServer::start().await;

        // Path is relative — NO /api/v2 prefix!
        Mock::given(method("GET"))
            .and(path("/organizations/my-org/teams"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [/* ... */],
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

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client.get_teams("my-org").await.unwrap();
        assert_eq!(result.len(), 2);
    }
}
```

### Key Rules

1. `TfeClient::test_client()` takes `mock_server.uri()`
   as base_url — never create local
   `create_test_client` helpers
2. Mock paths are WITHOUT `/api/v2` prefix
   (e.g., `/organizations/...` not
   `/api/v2/organizations/...`)
3. Always include pagination meta in list responses
4. Test error cases (404, 500) separately
5. Test pagination with multiple pages

## Pagination Helper (MANDATORY for list endpoints)

```rust
// In api.rs — generic ApiListResponse<T> (src/hcp/traits.rs); no per-resource response struct needed
pub async fn get_teams(&self, org: &str) -> Result<Vec<Team>> {
    let path = format!("/{}/{}/{}", api::ORGANIZATIONS, org, api::TEAMS);
    let error_context = format!("teams for organization '{}'", org);
    self.fetch_all_pages::<Team, ApiListResponse<Team>>(&path, &error_context).await
}
```

## Current Test Coverage

Counts are a snapshot; verify with `cargo test --lib -- --list` and `cargo test --test cli_tests -- --list`.

- **753 unit tests** (`cargo test --lib`)
- **97 integration tests** in `tests/cli_tests.rs`
- **No unit tests** in `hcp/{organizations,org_memberships,oauth_clients,projects,teams,tags}/commands.rs` — orchestration gap (consider e2e tests)
