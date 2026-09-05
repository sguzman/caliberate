# Caliberate HTTP/JSON API

The versioned API is rooted at `/api/v1` (or at
`<server.url_prefix>/api/v1` when a prefix is configured). It is source-neutral:
the same routes use the configured Caliberate database or an attached,
read-only Calibre backend selected when the server starts.

OPDS keeps its legacy primary acquisition route at
`/opds/books/{id}/download`. When a book has alternate formats, OPDS adds
acquisition links for them and serves them at
`/opds/books/{id}/download/{format}`. JSON and OPDS content routes share the
same authorization and streaming policy.

## Routes

| Method | Route | Purpose |
| --- | --- | --- |
| GET | `/api/v1/books` | Bounded summary browsing |
| POST | `/api/v1/books/query` | Structured summary query |
| GET | `/api/v1/search?q=term` | Compatibility simple search |
| GET | `/api/v1/books/{id}` | Book detail and formats |
| GET | `/api/v1/books/{id}/formats` | All available formats |
| GET | `/api/v1/books/{id}/content` | Primary content bytes |
| GET | `/api/v1/books/{id}/content/{format}` | Format-specific content bytes |
| GET | `/api/v1/facets/{kind}` | Authors, tags, series, publishers, ratings, or languages |

Browse and query pagination defaults to `limit=100`, `offset=0`, and is
bounded to a maximum limit of 500. Supported sorts are `id`, `title`,
`authors`, `series`, `tags`, `format`, `rating`, `publisher`, `languages`,
`date_added`, `date_modified`, and `pubdate`; direction is `asc` or `desc`.
Structured metadata filters use `authors`, `tags`, `series`, `publishers`,
`ratings`, or `languages`, with `include` or `exclude` mode.

Metadata responses contain protocol DTOs and never expose filesystem paths,
source roots, SQLite paths, or storage modes. Format entries include a
versioned `content_href`, for example:

```json
{"format":"epub","size_bytes":354595,"content_href":"/api/v1/books/56016/content/epub"}
```

Errors use this envelope:

```json
{"error":{"code":"invalid_request","message":"limit must be between 1 and 500"}}
```

Codes include `invalid_request`, `not_found`, `forbidden`,
`payload_too_large`, and `internal_error`. Content routes return raw ebook
bytes, stream through the shared OPDS/content authorization policy, honor
download enablement, external-reference policy, canonical attached-source
containment, and the configured maximum size.

The compatibility `/search` endpoint currently materializes the existing
simple-search result set before applying server-side offset/limit. It is not
claimed to be full-library-optimal. Attached static-source safety remains
controlled by server launch flags, not API consumers. Authentication is the
existing bearer-auth middleware; `--disable-auth` remains available for local
development.
