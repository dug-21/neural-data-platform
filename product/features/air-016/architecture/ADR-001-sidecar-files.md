# ADR-001: Per-Flush Sidecar Parquet Files

## Status: Accepted

## Context

The `ParquetStore` read-modify-write pattern causes O(file_size) memory allocation every 30-second flush cycle. The air-quality-app container grew from 96 MiB to 490 MiB in days and will OOM-kill at the 512 MiB Docker limit.

## Decision

Replace the read-modify-write append pattern with per-flush sidecar files. Each flush writes a new, small Parquet file named with an epoch-microsecond suffix. The read path globs all files in the partition directory.

## Alternatives Considered

### A: arrow-rs row group append
**Rejected.** The `parquet` crate v57 has no API for appending row groups to existing files. Custom footer manipulation is ~250 lines of byte-level code with corruption risk on power loss (no UPS on Pi). The arrow-rs maintainers explicitly declined to implement this.

### C: Per-flush files with mandatory compaction
**Rejected.** The write path is identical to our chosen approach, but mandatory compaction introduces race conditions (flush during compaction loses data), crash recovery state machines, file locking, and sequence tracking. This complexity is appropriate for S3 + Spark data lakes, not a single Raspberry Pi.

### D-4: Polars lazy concat
**Rejected.** `concat([existing.lazy(), new.lazy()]).collect()` still materializes the full merged DataFrame. Reduces the multiplier from 3x to ~1.5x but doesn't eliminate O(file_size) growth.

## Consequences

### Positive
- Memory usage becomes O(batch_size) constant — eliminates the OOM risk
- Write path becomes simpler (delete ~60 lines of read-back logic)
- Zero new dependencies
- Backward compatible with existing single-file partitions

### Negative
- Up to ~2,880 small files per stream per day (8 partitions x 2,880 = ~23,000 files/day total)
- Read path slightly slower due to globbing multiple files
- ext4 inode budget consumed faster (~170 days on 64GB SD card)

### Mitigations
- Optional compaction can be added as a future feature (not required for correctness)
- File count is bounded per day (time-partitioned directories)
- Each file is small (2-10 KB), well within filesystem limits

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Inode exhaustion (long-running) | Medium | High | Monitor, add compaction later |
| Query slowdown (many files) | Low | Low | Data volume per day is small (~11K rows) |
| SD card wear from metadata writes | Low | Medium | Fewer total bytes written than current approach |
| Legacy file not found by glob | Low | Low | `readings*.parquet` matches `readings.parquet` |
