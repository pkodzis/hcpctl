# hcpctl — Custom Agents Registry

## Agent Architecture

- User-invoked:
  - `@feature`: end-to-end coordinator, orchestrates all phases
    - Subagents: `design` (requirements and plan, read-only), `critic` (plan review, iterates with design, different model family), `api-perf` (API query efficiency review, iterates with design), `implement` (Rust/clap implementation), `test` (coverage gaps, writes missing tests), `review` (post-implementation verification, different model family), `rust-beast-mode` (deep Rust verification, may add tests)
  - `@design`: analysis and planning, read-only, handoff to `@feature`
  - `@implement`: feature implementation, standalone or subagent
  - `@test`: test coverage, standalone or subagent
  - `@review`: code review, standalone or subagent
  - `@guide`: `docs/src/guides` maintainer, analysis-first, dynamic coverage
  - `@code_review`: comprehensive code review orchestrator
    - Subagents: `code_review_duplicates` (duplicate patterns), `code_review_dead_code` (unused exports, dead code), `code_review_errors` (error handling consistency), `code_review_security` (security vulnerabilities)

## Agent Catalog

Model names use the `(litellm-connector)` suffix. The agent definitions in `.github/agents/*.agent.md` are the source of truth for models, fallbacks, tools, and access flags.

### Models and tools

- `feature`, `code_review`: `gpt-5.6-sol (litellm-connector)`; delegate and write reports, with terminal access. Both are user-invoked coordinators (`disable-model-invocation: true`).
- `design`, `implement`, `rust-beast-mode`: `claude-sonnet-5-5 (litellm-connector)`. Design is read-only and hands off to feature; implement edits and runs commands.
- `critic`, `api-perf`, `test`, `review`, `guide`, and `code_review_*`: `gpt-5.6-terra (litellm-connector)` with `gemini-3.8-flash (litellm-connector)` fallback. Critic, api-perf, review and the four code-review analyzers are read-only; security also has web access for advisories. Test and guide can edit and run commands.
- `critic`, `api-perf`, and the four `code_review_*` analyzers set `user-invocable: false`: hidden from the picker but available as subagents. This flag alone does not restrict callers.

**Rust Beast Mode**: `rust-beast-mode` (`claude-sonnet-5-5 (litellm-connector)`, no tools declared) is available standalone and is also invoked by `feature` for deep verification and re-review after fixes. Its available tools depend on the active environment.

**Model families**: Prefer different producer and checker families, including fallbacks. Claude produces designs and implementations; GPT or Gemini reviews them. `rust-beast-mode` verifies Claude implementation with the same family; `test` and `review` both use GPT/Gemini. `review` remains the cross-family implementation check.

## Model Routing and Cost

Cost policy (ECS policy; verify before relying on it):
- For `litellm-connector` models, the cost multiplier is max(input, output) USD per 1M tokens: `gpt-5.6-sol` 20, `gpt-5.6-terra` 12.
- Multipliers for `claude-sonnet-5-5` and `gemini-3.8-flash` are not stated in ECS policy; do not assume them.
- Actual prices and model availability require verification against the LiteLLM connector. This file does not state prices.

The ECS roster states that a subagent fails if its first available model's multiplier exceeds the caller's, without trying the next model; it also describes this limit for direct chat invocations. Verify this behavior in the target harness. Sonnet roles may exceed a Terra or Sol caller's ceiling; Sonnet pricing is not specified. Model lists choose the first available model; do not assume they recover from runtime errors or cost-ceiling rejections.

Routing for ad-hoc work:
- Use `Explore` for bulk reading, inventories, version sweeps, and searches.
- Delegate mechanical repetition and running/reporting of suites too.
- For ad-hoc delegation, use `gemini-3.8-flash (litellm-connector)` via `runSubagent` with `model` set, when it can do the work equally well and context savings exceed delegation overhead.
- Keep on the assigned model: design, version and dependency decisions, evidence conflicts, review, debugging, and short single-file edits.
- For borderline cases that would need full re-verification if wrong, ask the user first.
- If the entire task suits the cheap tier, suggest selecting a smaller model up front.
- Pipeline roles keep their assigned models.

**Key access controls**:
The allowlists and design handoff below are Local-harness features; verify discovery and delegation in the target harness. Copilot does not accept `agents` or `handoffs` in its schema.
- Feature has `disable-model-invocation: true` — user-invoked, not automatically selected as a subagent
- Critic has `user-invocable: false` — hidden from the picker, used by feature
- Feature declares `agents: [design, critic, api-perf, implement, test, review, rust-beast-mode]` — explicit subagent allowlist
- api-perf has `user-invocable: false` — hidden from the picker, used by feature after design↔critic converge
- Design has a `handoff` to feature — after user reviews the plan, one click to start execution
- code_review has `disable-model-invocation: true` — only user-invokable, never a subagent
- All 4 code_review_* analyzers have `user-invocable: false` — hidden from the picker, used by code_review

## Workflow Patterns

### Pattern 1: Full Automation (recommended for new features)

1. Switch to `@feature` and describe the request
2. Feature creates the `.feature-runs/` trail and runs autonomously: design↔critic loop → api-perf review → human gate → implement → test → review + rust-beast-mode → fix loop (max 2 rounds); limits are in the loop section below
3. Delivers final report (to trail + chat)

### Pattern 2: Design First (for complex tasks needing human review of plan)

1. Switch to `@design` — describe requirements
2. Review the plan yourself
3. Click **Execute Plan** handoff button → switches to `@feature`
4. Feature creates the forensic trail and runs its required design/review pipeline and human gate before implementation

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

1. Spawn `design`, which produces the plan.
2. Spawn `critic`, which reviews the plan (verdict: APPROVE or REVISE).
3. If REVISE: send critic's issues back to `design`.
4. Repeat steps 2-3 (max 7 iterations).
5. Spawn `api-perf`, which reviews API query efficiency (verdict: APPROVE or REJECT).
6. If REJECT: send api-perf feedback to `design`, then re-review (max 3 iterations).
7. Human gate: present the plan to the user and wait for approval.
8. Proceed to implementation.

- **Convergence**: Critic issues APPROVE when all issues are resolved or only LOW severity remain
- **No consensus**: If 7 iterations reached without APPROVE, feature coordinator proceeds with latest plan and documents unresolved disagreements
- **API perf**: After design↔critic converge, api-perf reviews for fetch-all-filter-locally anti-patterns, server-side filtering opportunities, query order optimization
- **Human gate**: Plan is presented to user for approval before implementation begins

## Retry Policy

The feature coordinator NEVER assumes a failed subagent's role:

1. Retry same subagent with error context appended (up to 3 attempts)
2. After 3 failures, document the failure and skip that phase
3. Never attempt the work directly — specialists exist for a reason

## Conventions

- Agent files: `.github/agents/<name>.agent.md`; all agents inherit `copilot-instructions.md`
- Access flags (`disable-model-invocation`, `user-invocable`, `handoffs`) are defined in each agent's frontmatter and summarized above
- Guide agent maintains `docs/src/guides/*.md` dynamically (no hardcoded topic list)
- Gitignored outputs: `./code-review-<timestamp>.md` and `.feature-runs/<timestamp>-<slug>/`

## Forensic Trail

Every `@feature` run creates a full audit trail in `.feature-runs/<timestamp>-<slug>/`.

Example for `.feature-runs/20260223-143021-add-get-variables/`:

- `00-request.md`: original request (verbatim)
- `01-design-v1.md`: first design output
- `02-critic-v1.md`: first critic review
- `03-design-v2.md`: revised design (if REVISE)
- `04-critic-v2.md`: second review (if needed)
- `NN-api-perf-v1.md`: API performance review
- `NN-plan-approved.md`: final plan (+ disagreements if no consensus)
- `NN-implement.md`: implementation log
- `NN-test.md`: test audit
- `NN-review.md`: review findings
- `NN-beast-mode.md`: deep Rust verification
- `NN-report.md`: final summary (ALWAYS LAST)

This is NOT project documentation — it is process forensics. Every subagent's complete raw output is preserved so design decisions, trade-offs, and iteration history can be reconstructed after the fact.

## Adding a New Agent

1. Create `.github/agents/<name>.agent.md`
2. Add YAML frontmatter: description, model, tools, agents (if coordinator), handoffs (if workflow)
3. Update this file (catalog entry and architecture list)
