# OPS-004: BUG-005 Memory Instrumentation

**Goal:** Instrument air-quality-app to identify the root cause of inter-snapshot RSS growth (~9 MiB/hr decelerating) that is NOT attributable to Parquet writes or the accumulator.

**Problem:** After air-018 eliminated the Polars leak, RSS still grows from 104 MiB to 229 MiB over 13.5 hours. The accumulator only accounts for 3.9 MiB of that 125 MiB gap. The remaining 121 MiB is unattributed.

**Approach:** Instrument first, optimize second. Add per-subsystem memory attribution to the heartbeat and snapshot diagnostic logs so the leak source is visible in production telemetry.

**Constraints:**
- Instrumentation overhead < 1% CPU
- No new crate dependencies (use libc FFI and /proc/self directly)
- Must run on Raspberry Pi 5 (aarch64, glibc, 512 MiB container)
- No behavioral changes to data pipeline

**Phases:**
1. Instrument (this feature) -- enhanced diagnostics, per-subsystem attribution
2. Mitigate -- targeted fix based on Phase 1 data (separate feature)
3. Validate -- 48h soak test proving RSS < 256 MiB (separate feature)

**Related:** BUG-004 (Polars leak, fixed in air-018), GitHub Issue #16
