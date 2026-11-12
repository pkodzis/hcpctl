---
description: 'Audit the hcpctl project and update all Copilot infrastructure files to match current state'
---

# Sync Copilot Infrastructure with Project Reality

The project may have drifted from what the Copilot configuration files describe.
Your job is to audit the actual codebase and update every Copilot file so they accurately reflect the current state.

## Files to audit and update

- [copilot-instructions.md](../copilot-instructions.md): tech stack versions, project structure, top rules, key conventions, resource aliases, flags
- [rust-hcpctl.instructions.md](../instructions/rust-hcpctl.instructions.md): clap patterns actually used, code organization list, pattern checklist references, API efficiency rules
- [testing.instructions.md](../instructions/testing.instructions.md): test counts (unit + integration), coverage gaps, wiremock patterns, test layer descriptions
- [github-actions.instructions.md](../instructions/github-actions.instructions.md): CI workflow structure, cross-compilation targets, release-please config
- [add-resource SKILL.md](../skills/add-resource/SKILL.md): step-by-step accuracy, code templates, file references
- [design.agent.md](../agents/design.agent.md): design output format, workflow steps, design principles
- [critic.agent.md](../agents/critic.agent.md): review checklist, severity rules, APPROVE/REVISE verdict format
- [review.agent.md](../agents/review.agent.md): review checklist items, tool restrictions
- [implement.agent.md](../agents/implement.agent.md): file placement rules, workflow steps
- [test.agent.md](../agents/test.agent.md): coverage gaps, test patterns, priority order
- [feature.agent.md](../agents/feature.agent.md): coordinator pipeline phases, design/critic loop, retry policy, rules, subagent allowlist
- Other agents: `api-perf`, `code_review*`, `guide`, `rust-beast-mode` (`../agents/<name>.agent.md`): frontmatter (model, tools, flags), role matches catalog
- [AGENTS.md](../AGENTS.md): agent architecture, catalog (models, tools, access flags), workflow patterns, conventions

## Audit procedure

1. **Gather current state** — run these commands and read key files:
   - `cargo test --lib -- --list` and `cargo test --test cli_tests -- --list` — enumerate unit and integration tests; enumeration does not run them
   - `cargo test` and `cargo clippy -- -D warnings` — run tests and check warnings; verify each exit status before reporting success
   - Read `Cargo.toml` — check dependencies and versions
   - Read `src/cli/mod.rs`, `src/hcp/mod.rs`, `src/output/mod.rs` — check which resources/subcommands exist
   - Read `src/hcp/traits.rs` — check current trait definitions
   - Read `.github/workflows/release.yml` — check CI targets
   - `ls src/hcp/ src/output/ src/cli/ src/context/` — resource modules, formatters, CLI subcommands, context module

2. **Compare and identify drift** — for each Copilot file, compare what it says vs what actually exists:
   - Are there new resources not mentioned?
   - Are there removed resources still mentioned?
   - Are test counts outdated?
   - Are dependency versions wrong?
   - Are file references pointing to files that no longer exist?
   - Are code patterns described that are no longer used?
   - Are there new patterns not yet documented?
   - Does the project structure match reality?

3. **Report drift** — before making changes, list all differences found in a table:
   | File | Section | Current value | Actual value | Action |

4. **Apply updates** — edit each file that has drift. Rules:
   - Preserve the existing structure and style of each file
   - Only change factual content (counts, names, paths, versions), not writing style
   - If a whole new section is needed (e.g., new resource), follow the pattern of existing sections
   - Do NOT remove rules or conventions unless they contradict current code
   - Do NOT add speculative content — only document what exists

5. **Verify** — after edits, confirm:
   - All file references in all Copilot files point to existing files (relative links resolved from each file's own directory)
   - Test counts match the latest enumeration; report test execution separately
   - Resource lists match `ls src/hcp/`
   - No Copilot file references a deleted resource or renamed file
