# DB Inventory (Calibre Reference)

## Schema files
- `tmp/calibre/resources/metadata_sqlite.sql`
- `tmp/calibre/resources/notes_sqlite.sql`
- `tmp/calibre/resources/fts_sqlite.sql`
- `tmp/calibre/resources/fts_triggers.sql`

## Schema upgrades
- `tmp/calibre/src/calibre/db/schema_upgrades.py`
- `tmp/calibre/src/calibre/db/fts/schema_upgrade.py`
- `tmp/calibre/src/calibre/db/notes/schema_upgrade.py`

## Core DB components
- `tmp/calibre/src/calibre/db/backend.py` (SQLite backend)
- `tmp/calibre/src/calibre/db/cache.py` (metadata cache)
- `tmp/calibre/src/calibre/db/tables.py` (table definitions)

Parity target: maintain schema compatibility with the SQL resources above, including versioned upgrades.
