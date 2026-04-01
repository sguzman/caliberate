# Ingest Inventory (Calibre Reference)

## Core ingest flow
- `calibre/src/calibre/db/adding.py` (import routines and duplicate handling)
- `calibre/src/calibre/db/copy_to_library.py` (copying between libraries)
- `calibre/src/calibre/library/add_to_library.py` (library add behavior)

Policy goals
- Support copy and reference ingest modes.
- Support archive reference ingestion with on-demand extraction (ZIP, RAR, 7Z, ZPAQ).
- Preserve duplicate detection and conflict policy parity.
