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
 └── @review          (code review, standalone or subagent)
```

## Agent Catalog

| Agent | Role | Model | Tools | Edits | Terminal | User-invokable | Subagent-ready |
|-------|------|-------|-------|-------|----------|----------------|----------------|
| **feature** | End-to-end coordination, delegates to workers | Claude Opus 4.6 | agent, search, editFiles, runInTerminal, problems, fetch, read | ✔ | ✔ | ✔ | ❌ (`disable-model-invocation: true`) |
| **design** | Analysis, planning, handoff to feature | Claude Opus 4.6 | search, fetch, problems, read | ❌ | ❌ | ✔ | ✔ |
| **critic** | Plan critique, challenges design | GPT-5.3-Codex | search, fetch, problems, read | ❌ | ❌ | ❌ (`user-invokable: false`) | ✔ |
| **implement** | Rust/clap implementation | Claude Opus 4.6 | search, editFiles, runInTerminal, problems, fetch, read | ✔ | ✔ | ✔ | ✔ |
| **test** | Test coverage + gap filling | Claude Opus 4.6 | search, editFiles, runInTerminal, problems, read | ✔ | ✔ | ✔ | ✔ |
| **review** | Post-implementation code review | GPT-5.3-Codex | search, fetch, problems, read | ❌ | ❌ | ✔ | ✔ |

**Model diversity**: Design and implementation use Claude Opus 4.6 for deep reasoning. Critic and review use GPT-5.3-Codex for independent perspective — different models catch different issues.

**Key access controls**:
- Feature has `disable-model-invocation: true` — only user-invokable, never spawned as a subagent
- Critic has `user-invokable: false` — only feature spawns it, never appears in the dropdown
- Feature declares `agents: [design, critic, implement, test, review]` — explicit subagent allowlist
- Design has a `handoff` to feature — after user reviews the plan, one click to start execution

## Workflow Patterns

### Pattern 1: Full Automation (recommended for new features)

1. Switch to `@feature` — describe what you want
2. Feature runs the full pipeline autonomously:
   - **Design↔Critic loop** (max 7 iterations until APPROVE or consensus)
   - **Implement** the approved plan
   - **Test** — fill coverage gaps
   - **Review** — final quality check
   - **Fix loop** — if review finds bugs/violations (max 2 rounds)
3. Delivers final report with all results

### Pattern 2: Design First (for complex tasks needing human review of plan)

1. Switch to `@design` — describe requirements
2. Review the plan yourself
3. Click **Execute Plan** handoff button → switches to `@feature`
4. Feature takes over from Phase 2 (implement → test → review)

### Pattern 3: Specialist (for focused work)

Switch directly to `@implement`, `@test`, or `@review` for targeted tasks.

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

## Adding a New Agent

1. Create `.github/agents/<name>.agent.md`
2. Add YAML frontmatter: description, model, tools, agents (if coordinator), handoffs (if workflow)
3. Update this file (catalog table + architecture diagram)
