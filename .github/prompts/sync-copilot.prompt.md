---
description: 'Audit the hcpctl project and update all Copilot infrastructure files to match current state'
---

# Sync Copilot Infrastructure with Project Reality

The project may have drifted from what the Copilot configuration files describe.
Your job is to audit the actual codebase and update every Copilot file so they accurately reflect the current state.

## Files to audit and update

| File | What to check |
|------|---------------|
| [copilot-instructions.md](../../.github/copilot-instructions.md) | Tech stack versions, project structure tree, top rules, key conventions, resource aliases, flags |
| [rust-hcpctl.instructions.md](../../.github/instructions/rust-hcpctl.instructions.md) | Clap patterns actually used, code organization table, pattern checklist references, API efficiency rules |
| [testing.instructions.md](../../.github/instructions/testing.instructions.md) | Test counts (unit + integration), coverage gaps, wiremock patterns, test layer descriptions |
| [github-actions.instructions.md](../../.github/instructions/github-actions.instructions.md) | CI workflow structure, cross-compilation targets, release-please config |
| [add-resource SKILL.md](../../.github/skills/add-resource/SKILL.md) | Step-by-step accuracy, code templates, file references |
| [design.agent.md](../../.github/agents/design.agent.md) | Design output format, workflow steps, design principles |
| [critic.agent.md](../../.github/agents/critic.agent.md) | Review checklist, severity rules, response format, infer: false |
| [review.agent.md](../../.github/agents/review.agent.md) | Review checklist items, tool restrictions |
| [implement.agent.md](../../.github/agents/implement.agent.md) | File placement rules, workflow steps |
| [test.agent.md](../../.github/agents/test.agent.md) | Coverage gaps, test patterns, priority order |
| [feature.agent.md](../../.github/agents/feature.agent.md) | Coordinator pipeline phases, design↔critic loop, retry policy, rules |
| [AGENTS.md](../../AGENTS.md) | Build/test commands, project layout, key rules, test counts, CI summary, agent registry |

## Audit procedure

1. **Gather current state** — run these commands and read key files:
   - `cargo test 2>&1 | tail -5` — get current test counts
   - `cargo clippy 2>&1 | tail -5` — check for warnings
   - Read `Cargo.toml` — check dependencies and versions
   - Read `src/cli/mod.rs`, `src/hcp/mod.rs`, `src/output/mod.rs` — check which resources/subcommands exist
   - Read `src/hcp/traits.rs` — check current trait definitions
   - Read `.github/workflows/release.yml` — check CI targets
   - `ls src/hcp/` — check which resource modules exist
   - `ls src/output/` — check which output formatters exist
   - `ls src/cli/` — check which CLI subcommands exist

2. **Compare and identify drift** — for each Copilot file, compare what it says vs what actually exists:
   - Are there new resources not mentioned?
   - Are there removed resources still mentioned?
   - Are test counts outdated?
   - Are dependency versions wrong?
   - Are file references pointing to files that no longer exist?
   - Are code patterns described that are no longer used?
   - Are there new patterns not yet documented?
   - Does the project structure tree match reality?

3. **Report drift** — before making changes, list all differences found in a table:
   | File | Section | Current value | Actual value | Action |

4. **Apply updates** — edit each file that has drift. Rules:
   - Preserve the existing structure and style of each file
   - Only change factual content (counts, names, paths, versions), not writing style
   - If a whole new section is needed (e.g., new resource), follow the pattern of existing sections
   - Do NOT remove rules or conventions unless they contradict current code
   - Do NOT add speculative content — only document what exists

5. **Verify** — after edits, confirm:
   - All file references in all Copilot files point to existing files
   - Test counts match latest `cargo test` output
   - Resource lists match `ls src/hcp/`
   - No Copilot file references a deleted resource or renamed file
