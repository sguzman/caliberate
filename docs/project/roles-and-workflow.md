# Roles and Repository Workflow

The Git repository is the communication conduit between the human maintainer, ChatGPT, and Codex. Important decisions, tasks, evidence, and handoffs must survive outside chat history.

## Role ownership

### Human maintainer

Owns:

- deciding when to start/stop an iteration;
- local Windows/Linux runtime testing that agents cannot reproduce;
- merging/rejecting pull requests;
- uploading or committing runtime evidence when needed;
- reporting subjective GUI/reader behavior such as startup, rendering, interaction, and speech quality.

The maintainer should not be required to diagnose implementation failures. The preferred handoff is: run the prescribed command/harness, preserve the output, and give the evidence back to the architect.

### ChatGPT / architect

Owns:

- `ARCHITECTURE.md`;
- `docs/project/philosophy.md`;
- roadmap ordering and subsystem boundaries;
- decomposition strategy;
- defining bounded Codex work items;
- reviewing Codex reports/diffs against architecture;
- deciding the next implementation task;
- updating current-status and architecture documents when reality changes.

ChatGPT may make repo-level documentation/governance changes directly. Broad implementation work should normally be delegated to Codex once the task contract is written.

### Codex / implementation agent

Owns:

- implementing one bounded delegated work item at a time;
- adding/updating tests required by that work item;
- running the task's validation commands;
- recording exact results in a work report;
- identifying architectural blockers instead of silently redesigning the system;
- leaving the repository in a state another agent can understand without chat context.

Codex does not own project philosophy, roadmap priority, or cross-subsystem architecture unless a task explicitly delegates that design decision.

## Directory model

```text
docs/
  project/
    philosophy.md
    current-status.md
    roles-and-workflow.md
  work/
    README.md
    ready/
    active/
    blocked/
    done/
    reports/
  roadmaps/
  inventory/
  tranches/
```

The older `roadmaps`, `inventory`, and `tranches` trees remain useful historical and subsystem context. The `project` and `work` trees are the live coordination layer for the restarted project.

## Work lifecycle

### 1. Architect creates a task

A task enters `docs/work/ready/` only when its scope, constraints, and acceptance criteria are sufficiently clear to implement without inventing new architecture.

Each task should include:

- why the task exists;
- exact scope;
- non-goals;
- implementation constraints;
- acceptance criteria;
- required validation;
- platform-specific verification that must be deferred to the human, if any.

### 2. Codex claims it

Codex moves the task file from `ready/` to `active/` and works only on that item.

Prefer one active implementation task at a time unless the architect explicitly defines independent parallel work.

### 3. Codex reports

Codex writes `docs/work/reports/<task-id>.md` with the implementation summary and exact validation results.

A report should distinguish:

- verified behavior;
- inferred behavior;
- platform behavior that still requires human validation.

### 4. Task becomes done or blocked

- `done/`: acceptance criteria satisfied.
- `blocked/`: architectural expansion, missing environment capability, or unresolved failure prevents completion.

A blocked task is useful evidence, not a failed interaction. The architect uses the report to revise the plan.

### 5. Human validation

For GUI, Windows, device, and TTS behavior, the architect may prescribe a local command or a tiny validation scenario. The human runs it and returns evidence without needing to debug it.

## Branching

Preferred naming:

- `chatgpt/<purpose>` for architecture/governance changes;
- `codex/<task-id>-<slug>` for delegated implementation;
- `fix/<slug>` or `feature/<slug>` only when a human intentionally chooses a conventional branch.

Implementation should reach `main` through reviewable commits/PRs. The repo documents should make the branch understandable even if chat context is lost.

## Evidence preservation

Do not rely on terminal scrollback for important validation.

On PowerShell, prefer patterns such as:

```powershell
cargo test --workspace 2>&1 | Tee-Object cargo-test.log
```

For richer iterations, add a repository script that captures validation output and packages it into a single uploadable artifact. This mirrors the previous evidence-preserving handoff model: the human executes; the artifact preserves success/failure; the architect diagnoses.

## Architecture escalation rule

Codex should not solve an architectural ambiguity by expanding scope. Instead:

1. preserve the smallest useful implementation/evidence;
2. document the conflict in the report;
3. mark the task blocked when necessary;
4. let the architect change the contract or architecture.

This separation is intentional: Codex is optimized for implementation throughput; ChatGPT maintains system coherence across iterations.
