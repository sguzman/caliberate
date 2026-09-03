# Work Queue

This directory is the live implementation queue shared by the architect, Codex, and the human maintainer.

## States

- `ready/`: architect-approved tasks that may be implemented.
- `active/`: a task currently being implemented.
- `blocked/`: implementation stopped because the contract cannot be completed safely within scope.
- `done/`: acceptance criteria satisfied.
- `reports/`: implementation and validation reports keyed by task ID.

Only tasks in `ready/` are authorization to begin new implementation work.

## Task IDs

Use monotonically increasing four-digit IDs such as `0001`, `0002`, etc. Keep the same ID through every state transition and use it for the report file.

Example:

```text
ready/0007-extract-reader-navigation.md
reports/0007.md
```

## Task template

```markdown
# 0000 — Short title

## Why

## Scope

## Non-goals

## Constraints

## Acceptance criteria

## Validation

## Human verification
```

Tasks should be small enough that one implementation pass produces a reviewable diff and specific enough that Codex is not forced to make cross-subsystem architectural decisions.

## Reports

Every implemented or blocked task gets a report:

```markdown
# 0000 Report

## Result

## Changes

## Validation

## Unverified / human follow-up

## Risks or deviations
```

Exact command output can be summarized in the report, but failures and platform caveats must not be hidden.
