# 0001 — Fix Windows input/output path identity

## Why

The native Windows restart baseline exposed one failing integration test: `ebook_convert_rejects_input_equals_output`.

`ebook-convert` canonicalizes the input path but may leave an absolute output path in ordinary Windows spelling. The same existing file can therefore compare as different `PathBuf`s when one side is an extended canonical path such as `\\?\A:\...` and the other is `A:\...`.

The CLI must reject conversion when input and output refer to the same existing filesystem object regardless of path spelling.

## Scope

- Fix path-identity validation in `crates/app/src/bin/ebook-convert.rs`.
- Preserve current CLI semantics for ordinary distinct input/output paths.
- Add or adjust focused regression coverage as needed so the intent is explicit.
- Keep the change confined to path validation and its tests.

## Non-goals

- Do not redesign conversion architecture.
- Do not implement cross-format conversion.
- Do not perform unrelated CLI cleanup.
- Do not start the GUI refactor.
- Do not broadly normalize every path in the application.
- Do not add a dependency solely for this small comparison problem unless there is a compelling reason documented in the report.

## Constraints

- The input path is already required to exist.
- The important failing case has an output path that is the same existing file as the input.
- Avoid comparing filesystem identity solely by textual path spelling on Windows.
- Preserve useful error text for the identical-path rejection.
- The solution must remain valid on Linux.

A small targeted approach is preferred over introducing a global path abstraction in this task.

## Acceptance criteria

1. `ebook-convert --dry-run <existing.epub> <same-existing.epub>` returns failure even when Windows canonicalization changes the path spelling.
2. The existing integration test `ebook_convert_rejects_input_equals_output` passes.
3. Distinct input/output paths continue to be accepted through dry-run validation when otherwise valid.
4. No unrelated behavior changes.
5. The task report explains what was actually verified in the Codex environment and what still requires native Windows confirmation.

## Validation

Run at minimum:

```text
cargo fmt --check
cargo test -p caliberate-app --test cli ebook_convert_rejects_input_equals_output
cargo test --workspace
```

If the package selector differs in the current manifest, use the actual package name and record the exact command in the report.

## Human verification

After the implementation is reviewed, the human maintainer will rerun the relevant test on native Windows. Codex must not claim native Windows verification unless its environment is actually Windows.
