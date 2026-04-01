# Assets & Storage Roadmap

- [x] Define library filesystem layout parity goals
- [x] Implement asset storage abstraction and path policy
- [x] Wire asset compression policy checks (no-op)
- [x] Implement raw asset compression policy (excluding metadata DB)
- [x] Implement zstd compression for copied assets
- [x] Add compression level tuning to control-plane
- [x] Implement reference-only asset tracking and integrity checks
- [x] Add asset checksum support and hashing helpers
- [x] Extend asset records with compression metadata
- [x] Implement asset integrity verification (size + checksum)
- [x] Implement storage compaction/cleanup routines
- [x] Implement compaction planning (missing assets + orphan files)
- [x] Implement compaction apply routine (remove orphan files)
- [x] Implement storage stats and auditing
- [x] Implement storage stats computation
- [x] Implement CLI reporting for asset stats/verify/compact
