# Library Layout Inventory (Calibre Reference)

## Layout logic
- `tmp/calibre/src/calibre/library/save_to_disk.py` (filesystem layout rules)
- `tmp/calibre/src/calibre/library/add_to_library.py` (copy/import behavior)
- `tmp/calibre/src/calibre/db/adding.py` (import flow)

Parity goals
- Mirror Calibre naming conventions for author/title directories.
- Preserve metadata sidecar generation behaviors where applicable.
- Allow reference-only storage alongside copy mode.
- Support optional zstd compression for copied raw assets while leaving the metadata DB uncompressed.
- Track stored assets with integrity checks (size/checksum) and compaction routines.
