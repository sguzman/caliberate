# Work Queue

This directory is the live implementation queue shared by the architect, Codex, and the human maintainer.

The repository itself is the agent-to-agent communication channel. Codex must commit/push its result and report so ChatGPT can inspect and integrate it directly. The human maintainer is not responsible for relaying patches, reports, or architecture instructions between agents.

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

## Task design for Codex/Luna-class workers

Assume the implementation worker may be relatively weak at architecture and broad inference. Tasks should therefore be deliberately over-specified rather than clever.

Prefer:

- one narrow objective;
- concrete failure/current behavior;
- exact scope and non-goals;
- named files/functions when known;
- acceptance criteria that can be mechanically checked;
- exact validation commands;
- an explicit blocked/escalation path.

Do not delegate a vague architectural problem and expect the worker to infer the intended system design.

## Task template

```markdown
# 0000 — Short title

## Why

## Scope

## Non-goals

## Constraints

## Acceptance criteria

## Validation

## Repository handoff

## Human verification
```

Tasks should be small enough that one implementation pass produces a reviewable diff and specific enough that Codex is not forced to make cross-subsystem architectural decisions.

## Repository handoff

Unless a task explicitly says otherwise, Codex should:

1. move the task from `ready/` to `active/` when work begins;
2. implement and validate it;
3. write `docs/work/reports/<task-id>.md`;
4. move the task to `done/` or `blocked/`;
5. commit all task code/docs/report state;
6. push the result to a remote branch named `codex/<task-id>-<slug>`.

ChatGPT reviews that pushed branch directly and is responsible for integrating accepted work into `main`.

A pull request is optional plumbing. The human maintainer should not be asked to create, review, or merge one as routine workflow.

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

## Human role in a normal iteration

The normal human loop is intentionally small:

1. `git pull` accepted `main` changes;
2. launch/feed Codex the repository task;
3. run local GUI/Windows/device/TTS verification only when the architect asks for it;
4. report observed runtime behavior.

The human is operating and testing the software, not delivering messages between the agents.
