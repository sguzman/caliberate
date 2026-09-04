# Roles and Repository Workflow

The Git repository is the communication conduit between the human maintainer, ChatGPT, and Codex. Important decisions, tasks, evidence, and handoffs must survive outside chat history.

The human maintainer is **not** the courier between ChatGPT and Codex. Agent-to-agent communication happens through committed repository state.

## Role ownership

### Human maintainer

Owns:

- deciding when to start/stop an iteration;
- maintaining the local checkout and pulling accepted `main` changes;
- launching Codex locally against the current repository/task;
- local Windows/Linux runtime testing that agents cannot reproduce;
- reporting subjective GUI/reader behavior such as startup, rendering, interaction, and speech quality.

The maintainer does **not** own:

- carrying implementation reports or patches from Codex to ChatGPT;
- carrying architecture/task instructions from ChatGPT to Codex when those instructions can live in the repository;
- reviewing agent diffs;
- merging agent code;
- diagnosing implementation failures.

The preferred human interaction is operational: pull `main`, run the prescribed command or local scenario, and report the observed result when runtime evidence is needed.

### ChatGPT / architect and integrator

Owns:

- `ARCHITECTURE.md`;
- `docs/project/philosophy.md`;
- `docs/project/product-scope.md`;
- roadmap ordering and subsystem boundaries;
- decomposition strategy;
- defining bounded Codex work items;
- inspecting Codex commits/branches/reports directly from GitHub;
- accepting, rejecting, or revising Codex work;
- integrating accepted work into `main`;
- deciding the next implementation task;
- updating current-status and architecture documents when reality changes.

ChatGPT may make architecture, roadmap, governance, and small integration changes directly on `main` when appropriate. Broad implementation work should normally be delegated to Codex once the task contract is written.

### Codex / implementation agent

Owns:

- implementing one bounded delegated work item at a time;
- adding/updating tests required by that work item;
- running the task's validation commands;
- recording exact results in a work report;
- identifying architectural blockers instead of silently redesigning the system;
- committing and pushing implementation plus handoff evidence to the repository;
- leaving enough evidence that ChatGPT can review the result directly without a human explanation.

Codex does not own project philosophy, roadmap priority, or cross-subsystem architecture unless a task explicitly delegates that design decision.

## Implementation-agent assumption

The Codex worker may be a low-cost, low-reasoning model. Current local usage may use Luna. Task design must therefore avoid depending on broad inference or architectural taste.

Architect-authored work items should prefer:

- one narrow objective;
- explicit context and failure mode;
- exact non-goals;
- named files/functions when known;
- concrete acceptance criteria;
- exact validation commands;
- clear instructions for what to report when blocked.

Large ambiguous tasks should be decomposed before delegation rather than trusting the worker to invent a coherent architecture.

## Directory model

```text
docs/
  project/
    philosophy.md
    product-scope.md
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

### 1. Architect creates a task on `main`

A task enters `docs/work/ready/` only when its scope, constraints, and acceptance criteria are sufficiently clear to implement without inventing new architecture.

Each task should include:

- why the task exists;
- exact scope;
- non-goals;
- implementation constraints;
- acceptance criteria;
- required validation;
- platform-specific verification that must be deferred to the human, if any.

### 2. Human launches Codex from the primary checkout

The maintainer uses the normal project checkout. The human does not perform routine implementation-branch switching.

Codex/Luna is responsible for starting from `main`, fast-forwarding it, creating/switching to the assigned `codex/<task-id>-<slug>` branch, and returning the checkout to `main` before the agent exits.

The checkout may temporarily be on the implementation branch while Luna is actively working. The human should not runtime-test the project during that implementation window.

### 3. Codex implements, pushes, and restores main

Codex moves the task from `ready/` to `active/`, implements only that item, writes its report, moves the task to `done/` or `blocked/`, commits, and pushes the result to a `codex/<task-id>-<slug>` branch or other architect-approved remote branch.

After the push, Codex must switch the same checkout back to `main` and verify the final branch with `git branch --show-current`. Codex must not merge its own task branch into `main`; the architect owns integration.

If uncommitted human changes prevent safe branch switching, Codex must stop and report them. It must not discard, hard-reset, force-clean, or silently stash those changes.

The important requirement is that the result becomes visible in GitHub while the human checkout is returned to a predictable `main` state. The human should not have to export/upload/deliver the diff or report manually.

### 4. Architect reviews directly from GitHub

ChatGPT inspects the pushed branch/commit and report directly from the repository.

ChatGPT then:

- integrates accepted work into `main`;
- rejects/revises work that violates the task or architecture;
- updates project state/roadmaps if the implementation exposes new information;
- writes the next ready task when appropriate.

A pull request is optional plumbing, not part of the human workflow. The architect may integrate directly without asking the human to manage PRs.

### 5. Human pulls accepted `main`

After ChatGPT integrates the iteration, the maintainer updates the primary checkout with:

```text
git switch main
git pull --ff-only
```

That is the normal delivery mechanism from the architect back to the human runtime/testing environment. The human does not switch to the implementation branch to test accepted work.

### 6. Human runtime validation when required

For GUI, Windows, device, and TTS behavior, the architect may prescribe a local command or a tiny validation scenario. The human runs it and reports observations. The human is testing the software, not transporting agent work.

## Branching

Preferred implementation branch naming:

- `codex/<task-id>-<slug>` for delegated implementation.

The project uses one primary local checkout. Codex/Luna owns the temporary branch transition:

```text
main -> codex/<task> -> main
```

The human maintainer should not be asked to perform those branch switches as part of the normal workflow.

Architecture/governance updates by ChatGPT may be committed directly to `main` unless isolation is useful for a risky change.

The architect is responsible for integrating Codex work into `main`. The human should not be asked to merge branches or PRs as routine workflow.

A previous two-worktree experiment is abandoned. Do not create a routine secondary `caliberate-luna` worktree. If such a stale worktree exists, Codex may remove it only when Git confirms it is clean; never force-remove unknown work.

## Evidence preservation

Do not rely on terminal scrollback for important validation.

On PowerShell, prefer patterns such as:

```powershell
cargo test --workspace 2>&1 | Tee-Object cargo-test.log
```

For richer iterations, add a repository script that captures validation output and packages it into a single artifact. This is for runtime evidence, not for carrying Codex's normal implementation handoff to ChatGPT; code/report handoff belongs in GitHub.

## Architecture escalation rule

Codex should not solve an architectural ambiguity by expanding scope. Instead:

1. preserve the smallest useful implementation/evidence;
2. document the conflict in the report;
3. mark the task blocked when necessary;
4. push the blocked state/report;
5. let the architect change the contract or architecture directly in the repository.

This separation is intentional: Codex is optimized for bounded implementation throughput; ChatGPT maintains system coherence and integration across iterations; the human operates and tests the actual program.
