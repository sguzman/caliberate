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

The human maintainer uses one primary checkout. Codex/Luna owns all implementation-branch switching inside that checkout.

1. Start from the single file in `docs/work/ready/`.
2. Before implementation, ensure the repository can safely change branches. If uncommitted changes would be overwritten, STOP and report them instead of stashing, resetting, or discarding them.
3. Switch the checkout to `main` and fast-forward it from `origin/main`.
4. Create or switch to the task branch named by the work item, normally `codex/<task-id>-<slug>`.
5. Move the task from `docs/work/ready/` to `docs/work/active/`.
6. Implement the smallest patch that satisfies its acceptance criteria.
7. Run every validation command listed in the work item, plus any directly relevant tests.
8. Write `docs/work/reports/<task-id>.md` with summary, files changed, exact validation results, risks/unverified behavior, and deviations.
9. Move the task to `docs/work/done/` only when its acceptance criteria are satisfied. If criteria cannot be satisfied without architectural expansion, move it to `docs/work/blocked/` and explain why in the report.
10. Commit and push the result so the architect can inspect it directly from GitHub.
11. **Before exiting, switch the same checkout back to `main`.** Do not merge the implementation branch into `main`; the architect owns integration.
12. Verify the postcondition explicitly with `git branch --show-current`. A successful handoff must leave the human checkout on `main`. If switching back fails, report that failure prominently instead of claiming completion.

Never claim Windows runtime behavior was verified unless it actually ran on Windows. A Linux compile or test is not a substitute for a Windows observation.

## Single-checkout branch safety

The project uses one normal checkout for both the human maintainer and Codex/Luna. This is intentional.

- The human should not perform routine task-branch switching.
- Codex/Luna owns switching from `main` to its task branch and back to `main`.
- The checkout may be temporarily on `codex/<task>` while Luna is actively working. The human should not runtime-test during that implementation window.
- At the end of every Luna run, the checkout must be back on `main`.
- A bare `git pull` does not change branches. Therefore the agent must verify the final branch instead of assuming a fetch/pull returned the checkout to `main`.
- Never use `git reset --hard`, destructive clean commands, or automatic stashing merely to make branch switching succeed.
- If repository state looks inconsistent, inspect `git branch --show-current`, `git status --short`, `git rev-parse HEAD`, and `git rev-parse origin/main` before diagnosing missing files.

### Cleanup of the abandoned worktree experiment

A previous workflow experiment may have created a sibling worktree such as `../caliberate-luna`. Codex/Luna may remove that stale worktree as housekeeping before starting a task **only if Git reports it is clean and safe to remove**. If it contains uncommitted changes or Git refuses normal removal, do not force-remove it; report the blocker. Do not create new secondary worktrees for routine tasks.

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
