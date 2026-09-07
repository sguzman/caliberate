# Architect review — task 0026 final correction

Status: **REJECTED — one tiny CLI continuation-token correction required**

Reviewed correction:
- `2147e780db0bc6a5e063ce02fca2e7861b8b579b`

## Accepted

The restartability correction itself is accepted:

- `after_book_format_id` is optional and defaults to `None`;
- it is converted into an exclusive format-only cursor;
- persistent-failure starvation is covered by a focused regression;
- dry-run honors the cursor without filesystem mutation;
- a later healthy format can be adopted after a failed bounded window;
- source bytes are explicitly proven unchanged;
- the original source reference asset row is explicitly proven to survive unchanged;
- no persistent checkpoint/job table was introduced;
- the existing bulk adoption architecture remains intact.

Do not redesign any of this.

## Blocker — human CLI does not expose the continuation token

The review required the returned last scanned cursor to be visible in both machine and human output.

Machine JSON already contains:

```text
last_book_format_id
```

but the human `sources adopt` output currently prints source/apply/count/readiness fields and omits `last_book_format_id`.

This makes the stateless resume feature incomplete for normal human use: after a failure-heavy bounded batch, the operator needs the exact returned format ID to supply to:

```text
--after-book-format-id <ID>
```

## Required correction

In the human output block print the continuation token explicitly, for example:

```rust
match result.last_book_format_id {
    Some(id) => println!("last_book_format_id={id}"),
    None => println!("last_book_format_id="),
}
```

Exact empty/`none` representation is flexible, but it must be unambiguous and copyable.

Also add CLI parsing coverage proving an explicit:

```text
--after-book-format-id 123
```

parses as `Some(123)`.

If practical, factor/render the human adoption summary through a small testable helper and assert the cursor line is present. Do not create a large output abstraction just for this.

## Scope

Correction only.

Do not change bulk selection, adoption, cursor semantics, readiness counts, progress events, CAS behavior, or source-preservation behavior.

Do not access the real library.
Do not add concurrency, persistent checkpoints, `--all`, GUI/HTTP/OPDS work, source deletion, or resync.

Preserve local `config/control-plane.toml`.

Rerun the same 0026 validation set, update `docs/work/reports/0026.md`, push the corrected same branch, and return checkout to `main`.