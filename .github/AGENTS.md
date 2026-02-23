# hcpctl — Custom Agents Registry

## Agent Architecture

```text
User
 ├── @feature         (end-to-end coordinator, orchestrates all phases)
 │    ├── subagent → design     (requirements + plan, read-only)
 │    ├── subagent → critic     (plan review, iterates with design, different model)
 │    ├── subagent → implement  (Rust/clap implementation)
 │    ├── subagent → test       (coverage gaps, write missing tests)
 │    └── subagent → review     (post-implementation verification, different model)
 ├── @design          (analysis + planning, read-only, handoff to @feature)
 ├── @implement       (feature implementation, standalone or subagent)
 ├── @test            (test coverage, standalone or subagent)
 ├── @review          (code review, standalone or subagent)
 └── @code_review     (comprehensive code review orchestrator)
      ├── subagent → code_review_duplicates  (find duplicate patterns)
      ├── subagent → code_review_dead_code   (unused exports, dead code)
      ├── subagent → code_review_errors      (error handling consistency)
      └── subagent → code_review_security    (security vulnerabilities)
```

## Agent Catalog

### Feature Pipeline Agents

| Agent | Role | Model | Tools | Edits | Terminal | User-invokable | Subagent-ready |
|-------|------|-------|-------|-------|----------|----------------|----------------|
| **feature** | End-to-end coordination, delegates to workers | Claude Opus 4.6 | agent, search, editFiles, runInTerminal, problems, fetch, read | ✔ | ✔ | ✔ | ❌ (`disable-model-invocation: true`) |
| **design** | Analysis, planning, handoff to feature | Claude Opus 4.6 | search, fetch, problems, read | ❌ | ❌ | ✔ | ✔ |
| **critic** | Plan critique, challenges design | GPT-5.3-Codex | search, fetch, problems, read | ❌ | ❌ | ❌ (`user-invokable: false`) | ✔ |
| **implement** | Rust/clap implementation | Claude Opus 4.6 | search, editFiles, runInTerminal, problems, fetch, read | ✔ | ✔ | ✔ | ✔ |
| **test** | Test coverage + gap filling | Claude Opus 4.6 | search, editFiles, runInTerminal, problems, read | ✔ | ✔ | ✔ | ✔ |
| **review** | Post-implementation code review | GPT-5.3-Codex | search, fetch, problems, read | ❌ | ❌ | ✔ | ✔ |

### Code Review Agents

| Agent | Role | Model | Tools | Edits | Terminal | User-invokable | Subagent-ready |
|-------|------|-------|-------|-------|----------|----------------|----------------|
| **code_review** | Parallel code review orchestrator, produces report file | Claude Opus 4.6 | agent, search, editFiles, runInTerminal, problems, read | ✔ | ✔ | ✔ | ❌ (`disable-model-invocation: true`) |
| **code_review_duplicates** | Find duplicate code patterns | GPT-5.3-Codex | search, problems, read | ❌ | ❌ | ❌ (`user-invokable: false`) | ✔ |
| **code_review_dead_code** | Unused exports and dead code | GPT-5.3-Codex | search, problems, read | ❌ | ❌ | ❌ (`user-invokable: false`) | ✔ |
| **code_review_errors** | Error handling consistency | GPT-5.3-Codex | search, problems, read | ❌ | ❌ | ❌ (`user-invokable: false`) | ✔ |
| **code_review_security** | Security vulnerabilities | GPT-5.3-Codex | search, problems, read | ❌ | ❌ | ❌ (`user-invokable: false`) | ✔ |

**Model diversity**: Design and implementation use Claude Opus 4.6 for deep reasoning. Critic, review, and code review analyzers use GPT-5.3-Codex for independent perspective — different models catch different issues.

**Key access controls**:
- Feature has `disable-model-invocation: true` — only user-invokable, never spawned as a subagent
- Critic has `user-invokable: false` — only feature spawns it, never appears in the dropdown
- Feature declares `agents: [design, critic, implement, test, review]` — explicit subagent allowlist
- Design has a `handoff` to feature — after user reviews the plan, one click to start execution
- code_review has `disable-model-invocation: true` — only user-invokable, never a subagent
- All 4 code_review_* analyzers have `user-invokable: false` — only code_review orchestrator spawns them

## Workflow Patterns

### Pattern 1: Full Automation (recommended for new features)

1. Switch to `@feature` — describe what you want
2. Feature creates `.feature-runs/<timestamp>-<slug>/` forensic trail directory
3. Feature runs the full pipeline autonomously, saving every subagent output:
   - **Design↔Critic loop** (max 7 iterations until APPROVE or consensus)
   - **Implement** the approved plan
   - **Test** — fill coverage gaps
   - **Review** — final quality check
   - **Fix loop** — if review finds bugs/violations (max 2 rounds)
4. Delivers final report (to trail + chat)

### Pattern 2: Design First (for complex tasks needing human review of plan)

1. Switch to `@design` — describe requirements
2. Review the plan yourself
3. Click **Execute Plan** handoff button → switches to `@feature`
4. Feature creates forensic trail, takes over from Phase 2 (implement → test → review)

### Pattern 3: Specialist (for focused work)

Switch directly to `@implement`, `@test`, or `@review` for targeted tasks.

### Pattern 4: Comprehensive Code Review

1. Switch to `@code_review` — optionally describe focus areas
2. Orchestrator spawns all 4 analyzers **in parallel**:
   - Duplicates, Dead Code, Error Handling, Security
3. Collects all findings, deduplicates, prioritizes
4. Writes `./code-review-<timestamp>.md` with full report
5. Presents executive summary

## Design ↔ Critic Iterative Loop

The `@feature` coordinator runs a peer-review loop between design and critic:

```text
feature (coordinator)
 ├── 1. Spawn design  → produces plan
 ├── 2. Spawn critic  → reviews plan (verdict: APPROVE or REVISE)
 ├── 3. If REVISE: send critic's issues back to design
 ├── 4. Repeat steps 2-3 (max 7 iterations)
 └── 5. Proceed with latest plan (document disagreements if no consensus)
```

- **Convergence**: Critic issues APPROVE when all issues are resolved or only LOW severity remain
- **No consensus**: If 7 iterations reached without APPROVE, feature coordinator proceeds with latest plan and documents unresolved disagreements
- **No human gate**: Plan is never presented for approval — feature coordinator proceeds autonomously

## Retry Policy

The feature coordinator NEVER assumes a failed subagent's role:

1. Retry same subagent with error context appended (up to 3 attempts)
2. After 3 failures, document the failure and skip that phase
3. Never attempt the work directly — specialists exist for a reason

## Conventions

- Agent files: `.github/agents/<name>.agent.md`
- All agents inherit `copilot-instructions.md` (tech stack, top rules, key conventions)
- Worker agents (implement, test, review) have default flags — user-invokable and subagent-ready
- Feature has `disable-model-invocation: true` — only user-invokable, never a subagent
- Critic has `user-invokable: false` — only feature spawns it, never user-facing
- Design has `handoffs` to feature — guided workflow from plan to execution
- code_review has `disable-model-invocation: true` — coordinator only, never a subagent
- code_review_* analyzers have `user-invokable: false` — spawned only by code_review orchestrator
- code_review output goes to `./code-review-<timestamp>.md` (gitignored)
- Feature forensic trails go to `.feature-runs/<timestamp>-<slug>/` (gitignored)

## Forensic Trail

Every `@feature` run creates a full audit trail in `.feature-runs/<timestamp>-<slug>/`:

```
.feature-runs/20260223-143021-add-get-variables/
  00-request.md              # original request (verbatim)
  01-design-v1.md            # first design output
  02-critic-v1.md            # first critic review
  03-design-v2.md            # revised design (if REVISE)
  04-critic-v2.md            # second review (if needed)
  ...
  NN-plan-approved.md        # final plan (+ disagreements if no consensus)
  NN-implement.md            # implementation log
  NN-test.md                 # test audit
  NN-review.md               # review findings
  NN-report.md               # final summary (ALWAYS LAST)
```

This is NOT project documentation — it is process forensics. Every subagent's complete raw output is preserved so design decisions, trade-offs, and iteration history can be reconstructed after the fact.

## Adding a New Agent

1. Create `.github/agents/<name>.agent.md`
2. Add YAML frontmatter: description, model, tools, agents (if coordinator), handoffs (if workflow)
3. Update this file (catalog table + architecture diagram)
