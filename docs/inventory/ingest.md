# Ingest Inventory (Calibre Reference)

## Core ingest flow
- `tmp/calibre/src/calibre/db/adding.py` (import routines and duplicate handling)
- `tmp/calibre/src/calibre/db/copy_to_library.py` (copying between libraries)
- `tmp/calibre/src/calibre/library/add_to_library.py` (library add behavior)

Policy goals
- Support copy and reference ingest modes.
- Support archive reference ingestion with on-demand extraction (ZIP now; RAR/7Z later).
- Preserve duplicate detection and conflict policy parity.
