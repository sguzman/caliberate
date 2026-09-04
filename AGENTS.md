# Caliberate Agent Guide

This file is the entry point for coding agents. Keep it short. The repository documentation is the system of record.

## Read before changing code

1. `docs/project/philosophy.md`
2. `docs/project/product-scope.md`
3. `docs/project/priorities.md`
4. `ARCHITECTURE.md`
5. `docs/project/library-platform-architecture.md`
6. `docs/project/current-status.md`
7. `docs/project/roles-and-workflow.md`
8. The single assigned work item under `docs/work/ready/` or `docs/work/active/`

Do not infer authority from old tranche or parity documents when they conflict with the files above. Historical roadmaps and tranches are useful context, not permission to broaden scope.

## Roles

- **ChatGPT / architect** owns high-level architecture, product priorities, roadmap ordering, work-item definitions, review, and integrating accepted implementation into `main`.
- **Codex / implementation agent** owns bounded implementation work explicitly delegated through `docs/work/` and commits/pushes that work to the repository.
- **Human maintainer** owns local runtime testing and operating the local checkout. The human is not the communication courier between agents and is not responsible for reviewing/merging agent code.

## Assume bounded implementation intelligence

Implementation tasks must be executable by a relatively weak/cheap coding model without requiring it to reconstruct architecture from inference.

Therefore:

- Follow the work item literally.
- Prefer explicit file/function targets when the task provides them.
- Respect every non-goal.
- Do not guess at cross-subsystem architecture.
- Do not bundle "helpful" cleanup.
- If two plausible implementations would change architecture differently, stop and report the choice instead of improvising.

## Scope discipline

- Implement only the assigned work item.
- Do not perform opportunistic refactors outside that scope.
- Do not alter architecture, philosophy, product scope, priorities, or roadmap ordering unless the work item explicitly asks for it.
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
- Platform-specific behavior must live behind explicit abstractions and `cfg` boundaries rather than leaking through reader/UI code.
- Windows and Linux are first-class targets.
- Prefer deterministic, local tests. Add regression tests for bugs when practical.
- Use `tracing` for meaningful runtime diagnostics rather than ad-hoc prints.
- Avoid new dependencies unless they materially simplify a bounded problem. Use current compatible releases when adding one is justified.
- Avoid external-process dependencies for core functionality. Compatibility bridges must remain optional and isolated.
- Caliberate must remain useful without Calibre installed or running.

## Current P0 boundary

Current P0 is the **visual library platform**, not exhaustive Calibre parity.

- Build a reusable library/query/content service.
- Make the egui GUI a Calibre-like visual client of that service.
- Make HTTP/JSON and OPDS thin adapters over the same service semantics.
- Preserve managed, arbitrary-directory, and attached-Calibre source workflows.
- Do not expand low-priority editor/news/email/plugin/device work unless a task explicitly says to.

## Work-item protocol

Work state lives under `docs/work/`.

1. Start from one file in `docs/work/ready/`.
2. Move it to `docs/work/active/` when implementation begins.
3. Implement the smallest patch that satisfies its acceptance criteria.
4. Run every validation command listed in the work item, plus any directly relevant tests.
5. Write `docs/work/reports/<task-id>.md` with summary, files changed, exact validation results, risks/unverified behavior, and deviations.
6. Move the task to `docs/work/done/` only when its acceptance criteria are satisfied.
7. If criteria cannot be satisfied without architectural expansion, move it to `docs/work/blocked/` and explain why in the report.
8. Commit and push the result so the architect can inspect it directly from the repository. Do not require the human maintainer to relay patches, reports, or explanations between agents.
9. **Codex/Luna must work in a dedicated implementation worktree, never in the human maintainer's primary checkout.** The primary checkout is reserved for `main` and runtime testing. The implementation worktree may remain on `codex/<task>`; it must not switch, reset, or otherwise mutate the branch checked out in the human worktree.
10. After pushing the implementation branch, stop. Do not merge into `main` locally; the architect owns integration.

Never claim Windows runtime behavior was verified unless it actually ran on Windows. A Linux compile or test is not a substitute for a Windows observation.

## Worktree and branch safety

The human maintainer's primary checkout and the implementation agent's checkout are separate Git worktrees.

Required model:

- primary human worktree: stays on `main`; used for pulls and runtime testing;
- implementation worktree: used for `codex/<task>` branches and agent edits;
- Luna/Codex must never switch the human worktree away from `main`;
- the human should not need to switch branches to inspect or test an implementation; accepted work is integrated remotely by the architect, then pulled into the primary `main` worktree;
- a successful fetch does not change the currently checked-out branch. Human delivery remains `git switch main` followed by `git pull --ff-only`;
- when repository state looks inconsistent, check `git branch --show-current`, `git rev-parse HEAD`, `git rev-parse origin/main`, and `git worktree list` before diagnosing missing files or sparse checkout.

If a dedicated implementation worktree is not available, stop and ask the human/architect to create or identify one rather than using the primary checkout.

## Validation baseline

Unless a task narrows this deliberately, finish with:

```text
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

Run Clippy when the task changes enough code for it to be informative. Do not convert the existing warning backlog into unrelated scope unless the task says to.

## Handoff

The repository is the communication conduit. Leave enough committed evidence that the architect can understand and integrate the work without chat history or a human-delivered explanation.
