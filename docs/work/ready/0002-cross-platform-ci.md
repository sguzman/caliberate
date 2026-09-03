# 0002 — Add Windows and Linux CI baseline

## Why

Task 0001 restored a green native-Windows workspace baseline. Caliberate treats Windows and Linux as first-class targets, and future Luna/Codex iterations need automatic protection against platform regressions.

This task adds only the minimal GitHub Actions validation baseline. Do not use it as an excuse to clean warnings or change application behavior.

## Scope

Create a GitHub Actions workflow under `.github/workflows/` that runs on both:

- `windows-latest`
- `ubuntu-latest`

For each platform, use the stable Rust toolchain and run:

```text
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

Use Cargo caching if it can be added simply with a standard maintained GitHub action. Keep the workflow readable and conventional.

Trigger the workflow on:

- pushes to `main`;
- pull requests targeting `main`.

## Non-goals

- Do not add macOS CI.
- Do not add release/package jobs.
- Do not add coverage tooling.
- Do not make Clippy warnings fatal.
- Do not fix the existing GUI warning backlog.
- Do not change Rust/application source files unless the workflow reveals a genuine platform build/test failure. If that happens, stop and mark the task blocked rather than broadening scope.
- Do not modify product/architecture/roadmap docs.

## Constraints

- Use official/common maintained actions.
- Pin actions to current stable major versions rather than obscure commit hashes unless the repo already follows another convention.
- The workflow must use the checked-in `Cargo.lock`.
- Do not install Calibre or depend on Calibre executables.
- Keep platform-specific setup to the minimum actually required by the current workspace.

## Acceptance criteria

1. A workflow exists under `.github/workflows/`.
2. Its test matrix contains `windows-latest` and `ubuntu-latest`.
3. Each matrix job runs format check, workspace check, and workspace tests.
4. It triggers on pushes to `main` and PRs targeting `main`.
5. Local validation remains green on the native Windows Codex environment.
6. No application behavior is changed.

## Validation

Run locally:

```text
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

Also inspect the workflow YAML carefully for valid GitHub Actions syntax. If a YAML parser/tool is already available locally, using it is fine, but do not add a project dependency just to validate YAML.

## Repository handoff

- Move this file to `docs/work/active/` when starting.
- Write `docs/work/reports/0002.md` with the workflow summary and exact local validation results.
- Move the task to `docs/work/done/` only if acceptance criteria are satisfied; otherwise move it to `docs/work/blocked/`.
- Commit all task/workflow/report state.
- Push to `codex/0002-cross-platform-ci`.
- Do not ask the human maintainer to relay the workflow or report to ChatGPT.

## Human verification

No special manual UI verification is required. After the architect integrates the branch, GitHub Actions itself will provide the real Windows/Linux CI result. If a CI-only failure appears, the architect will create a follow-up task rather than asking the human to diagnose it.
