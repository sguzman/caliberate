# Caliberate Agent Guide

This file is the entry point for coding agents. Keep it short. The repository documentation is the system of record.

## Read before changing code

1. `docs/project/philosophy.md`
2. `ARCHITECTURE.md`
3. `docs/project/current-status.md`
4. `docs/project/roles-and-workflow.md`
5. The single assigned work item under `docs/work/ready/` or `docs/work/active/`

Do not infer authority from old tranche or parity documents when they conflict with the files above. Historical roadmaps and tranches are useful context, not permission to broaden scope.

## Roles

- **ChatGPT / architect** owns high-level architecture, project philosophy, roadmap ordering, work-item definitions, and acceptance/rejection of architectural changes.
- **Codex / implementation agent** owns bounded implementation work explicitly delegated through `docs/work/`.
- **Human maintainer** owns local runtime testing, merge decisions, and observations that cannot be reproduced in the agent environment.

## Scope discipline

- Implement only the assigned work item.
- Do not perform opportunistic refactors outside that scope.
- Do not alter `ARCHITECTURE.md`, `docs/project/philosophy.md`, or roadmap priority unless the work item explicitly asks for it.
- Do not silently change public behavior, persistence formats, config semantics, or database schemas.
- Do not replace a real implementation with a mock, shell, placeholder, or UI-only parity surface.
- Do not weaken or delete a failing test merely to make the suite green.

If the task exposes a design conflict, stop broadening the patch. Record the conflict in the work report and mark the task blocked if necessary.

## Implementation principles

- Rust stable, edition 2024 where already configured.
- Preserve the multi-crate architecture and explicit dependency direction.
- Prefer small modules with a single responsibility. Do not create new god files.
- Existing very large files are migration targets, not patterns to copy.
- Files above roughly 1,000 lines deserve active scrutiny; do not grow multi-thousand-line hand-maintained modules without explicit architectural approval.
- Platform-specific behavior must live behind explicit abstractions and `cfg` boundaries rather than leaking through the reader/UI code.
- Windows and Linux are first-class targets.
- Prefer deterministic, local tests. Add regression tests for bugs when practical.
- Use `tracing` for meaningful runtime diagnostics rather than ad-hoc prints.
- Avoid new dependencies unless they materially simplify a bounded problem. Use current compatible releases when adding one is justified.
- Avoid external-process dependencies for core functionality. Compatibility bridges must remain optional and isolated.

## Work-item protocol

Work state lives under `docs/work/`.

1. Start from one file in `docs/work/ready/`.
2. Move it to `docs/work/active/` when implementation begins.
3. Implement the smallest patch that satisfies its acceptance criteria.
4. Run every validation command listed in the work item, plus any directly relevant tests.
5. Write `docs/work/reports/<task-id>.md` with:
   - summary of changes
   - files changed
   - exact validation commands and results
   - remaining risks or unverified platform behavior
   - deviations from the task, if any
6. Move the task to `docs/work/done/` only when its acceptance criteria are satisfied.
7. If criteria cannot be satisfied without architectural expansion, move it to `docs/work/blocked/` and explain why in the report.

Never claim Windows runtime behavior was verified unless it actually ran on Windows. A Linux compile or test is not a substitute for a Windows observation.

## Validation baseline

Unless a task narrows this deliberately, finish with:

```text
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

Run Clippy when the task changes enough code for it to be informative. Do not convert the existing warning backlog into unrelated scope unless the task says to.

## Reader-specific boundary

The long-term reader architecture is defined in `ARCHITECTURE.md`. In particular:

- GUI reader code should consume a normalized document model.
- Format parsing belongs outside the GUI.
- TTS belongs behind a speech-engine abstraction.
- Windows speech APIs must not be called directly from generic reader widgets/state.

## Handoff

The repository is the communication conduit. Leave enough committed evidence that another agent can understand what happened without chat history.
