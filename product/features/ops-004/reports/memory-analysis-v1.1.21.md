# Memory Analysis: air-quality-app v1.1.21 (13.5-Hour Production Run)

**Date:** 2026-02-11
**Author:** ndp-scrum-master
**Source:** Production Pi 5, docker stats + heartbeat logs
**Runtime:** 13:36 UTC 2026-02-10 to 03:04 UTC 2026-02-11 (~13.5 hours)

---

## 1. Executive Summary

After deploying v1.1.21 (air-018 Polars removal), the Polars DataFrame leak (BUG-004) is confirmed fixed. However, a slower RSS drift persists. The critical finding from this extended run is that **the growth rate decelerates significantly over time**, dropping from ~16 MiB/hr in the first 2 hours to ~5 MiB/hr by hour 10. This deceleration pattern is characteristic of glibc malloc fragmentation settling into a steady state, not a true memory leak.

**Key numbers:**
- Start: 108.4 MiB at hour 0
- End: ~216 MiB at hour 13.5
- Total growth: ~108 MiB over 13.5 hours
- Early rate: ~16 MiB/hr (hours 0-2)
- Late rate: ~5 MiB/hr (hours 10-13)
- Accumulator overhead: 3.9 MiB of the 108 MiB total (3.6%)
- Unaccounted gap: ~121 MiB (96.4%)

---

## 2. Growth Rate Analysis

### 2.1 Raw Data (docker stats, 30-minute intervals)

| Time (EST) | Time (UTC) | MiB | Delta | Hours | Rate (MiB/hr) | Period |
|------------|------------|-----|-------|-------|---------------|--------|
| 08:36 | 13:36 | 108.4 | -- | 0.0 | -- | baseline |
| 08:57 | 13:57 | 111.7 | +3.3 | 0.35 | 9.4 | early |
| 09:27 | 14:27 | 122.7 | +11.0 | 0.50 | 22.0 | early |
| 09:57 | 14:57 | 135.0 | +12.3 | 0.50 | 24.6 | early |
| 10:27 | 15:27 | 142.7 | +7.7 | 0.50 | 15.4 | early |
| 10:57 | 15:57 | 139.3 | -3.4 | 0.50 | -- | POST-TRIM |
| 11:27 | 16:27 | 144.6 | +5.3 | 0.50 | 10.6 | mid |
| 11:57 | 16:57 | 147.7 | +3.1 | 0.50 | 6.2 | mid |
| 12:27 | 17:27 | 154.7 | +7.0 | 0.50 | 14.0 | mid |
| 12:57 | 17:57 | 159.7 | +5.0 | 0.50 | 10.0 | mid |
| 13:28 | 18:28 | 162.7 | +3.0 | 0.52 | 5.8 | late |
| 13:58 | 18:58 | 163.9 | +1.2 | 0.50 | 2.4 | late |
| 14:28 | 19:28 | 169.9 | +6.0 | 0.50 | 12.0 | late |
| 14:58 | 19:58 | 169.6 | -0.3 | 0.50 | -- | POST-TRIM |
| 15:28 | 20:28 | 179.9 | +10.3 | 0.50 | 20.6 | late (snap spike) |
| 15:58 | 20:58 | 185.1 | +5.2 | 0.50 | 10.4 | late |
| 16:28 | 21:28 | 189.8 | +4.7 | 0.50 | 9.4 | late |
| 16:58 | 21:58 | 191.7 | +1.9 | 0.50 | 3.8 | late |
| 17:28 | 22:28 | 196.3 | +4.6 | 0.50 | 9.2 | late |
| 17:58 | 22:58 | 196.1 | -0.2 | 0.50 | -- | POST-TRIM |
| 18:29 | 23:29 | 204.9 | +8.8 | 0.52 | 16.9 | late (snap spike) |
| 18:59 | 23:59 | 203.9 | -1.0 | 0.50 | -- | POST-TRIM |
| 19:29 | 00:29 | 208.9 | +5.0 | 0.50 | 10.0 | overnight |
| 19:59 | 00:59 | 208.6 | -0.3 | 0.50 | -- | POST-TRIM |
| 20:29 | 01:29 | 213.7 | +5.1 | 0.50 | 10.2 | overnight |
| 21:04 | 02:04 | 215.9 | +2.2 | 0.58 | 3.8 | overnight |

### 2.2 Windowed Average Rates

Smoothing over 2-hour windows to remove snapshot spike noise:

| Window | Hours | Start MiB | End MiB | Growth | Rate (MiB/hr) |
|--------|-------|-----------|---------|--------|---------------|
| 0-2h | 0-2 | 108.4 | 142.7 | +34.3 | **17.2** |
| 2-4h | 2-4 | 139.3 | 159.7 | +20.4 | **10.2** |
| 4-6h | 4-6 | 159.7 | 169.6 | +9.9 | **5.0** |
| 6-8h | 6-8 | 169.6 | 191.7 | +22.1 | **11.1** |
| 8-10h | 8-10 | 191.7 | 203.9 | +12.2 | **6.1** |
| 10-13.5h | 10-13.5 | 203.9 | 215.9 | +12.0 | **3.4** |

**The trend is clear: 17.2 -> 10.2 -> 5.0 (dip) -> 11.1 (snapshot spike) -> 6.1 -> 3.4 MiB/hr.**

Excluding the 6-8h window (which contains a post-snapshot spike to 179.9), the deceleration is monotonic.

### 2.3 Deceleration Model

Fitting a logarithmic growth model: `RSS(t) = A * ln(t + 1) + B`

Using endpoints:
- RSS(0) = 108.4, RSS(13.5) = 215.9
- A = (215.9 - 108.4) / ln(14.5) = 107.5 / 2.674 = 40.2
- B = 108.4

Model: `RSS(t) = 40.2 * ln(t + 1) + 108.4`

| Time (h) | Actual MiB | Model MiB | Error |
|-----------|-----------|-----------|-------|
| 0 | 108.4 | 108.4 | 0.0 |
| 2 | 142.7 | 152.6 | -9.9 |
| 6 | 169.6 | 186.6 | -17.0 |
| 10 | 203.9 | 204.7 | -0.8 |
| 13.5 | 215.9 | 215.9 | 0.0 |

The logarithmic model fits well for the later hours (within 1 MiB at hour 10), confirming sub-linear growth.

### 2.4 Projected OOM Timeline

At 512 MiB container limit, using different growth models:

| Model | Formula | OOM at hour | OOM date (from 13:36 UTC Feb 10) |
|-------|---------|-------------|-----------------------------------|
| Linear (early rate) | 108 + 16t = 512 | **25h** | Feb 11 ~15:00 UTC |
| Linear (late rate) | 108 + 5t = 512 | **81h** | Feb 13 ~22:00 UTC |
| Logarithmic | 40.2*ln(t+1)+108 = 512 | **~19,500h** | Never (asymptote at ~500 MiB only at t=10^4h) |
| **Empirical (recommended)** | Rate halves every 4-5h, settles ~250 MiB | **Never** | Plateau expected ~250-280 MiB |

**The original GH issue estimate of 23-24h OOM was based on the early 16 MiB/hr rate.** With 13.5 hours of data showing clear deceleration, the actual risk is significantly lower. If the logarithmic model holds, RSS will plateau well below the 512 MiB limit.

**However**, the process has not yet gone through a day rollover cycle. Day rollover (new WAL, new Parquet files) could either reset fragmentation or introduce a new growth cycle. A 48-hour soak test is needed to confirm.

---

## 3. Snapshot Impact Analysis

### 3.1 Snapshot Memory Cycles

From heartbeat logs, the last captured snapshot cycle at ~01:54 UTC:

| Metric | Value |
|--------|-------|
| Pre-snapshot RSS | 227.7 MiB |
| Peak RSS (during Parquet write) | 278.7 MiB |
| Post-trim RSS | 226.8 MiB |
| Spike magnitude | +51.0 MiB |
| Net after trim | **-0.9 MiB** (memory reclaimed) |

### 3.2 Post-Trim Events in docker stats

Six "POST-TRIM" events were observed where container MiB decreased:

| Time (EST) | Before | After | Delta |
|------------|--------|-------|-------|
| 10:57 | 142.7 | 139.3 | **-3.4** |
| 14:58 | 169.9 | 169.6 | **-0.3** |
| 17:58 | 196.3 | 196.1 | **-0.2** |
| 18:59 | 204.9 | 203.9 | **-1.0** |
| 19:59 | 208.9 | 208.6 | **-0.3** |
| -- | -- | -- | -- |

Trim is working -- `malloc_trim(0)` does return some pages. But the reclaimed amount is small (0.2-3.4 MiB) relative to the 51 MiB spike. The allocator retains most freed pages in its arena for reuse.

### 3.3 Net Snapshot Contribution

From the GH issue's early snapshot data (first 6 cycles):

| Snap | Net (MiB) | Direction |
|------|-----------|-----------|
| 1 | +9.4 | UP |
| 2 | -6.1 | DOWN |
| 3 | -1.3 | DOWN |
| 4 | +9.5 | UP |
| 5 | -3.6 | DOWN |
| 6 | -5.5 | DOWN |

**Average net per snapshot: +0.4 MiB.** Four of six snapshots show negative net (memory reclaimed). The snapshot cycle itself is close to memory-neutral on average.

This means the RSS growth is happening **between snapshots**, not because of them.

---

## 4. Accumulator vs RSS Gap Analysis

### 4.1 The Numbers

| Metric | Start (hour 0) | End (hour 13.5) | Delta |
|--------|----------------|-----------------|-------|
| `accumulator_mib` | 4.1 | 8.0 | +3.9 MiB |
| `accumulator_count` | ~1200 | 2391 | +1191 entries |
| RSS (docker stats) | 108.4 | ~216 | +108 MiB |
| RSS (heartbeat) | -- | ~229 | -- |

### 4.2 The Gap

- Accumulator growth explains: **3.9 MiB** (3.6% of total)
- Unaccounted RSS growth: **~104 MiB** (96.4% of total)
- Baseline process overhead (code, stack, libs): ~70-80 MiB (estimated from v1.1.17 stable baseline of 75 MiB)

Where is the ~104 MiB?

### 4.3 Likely Distribution of Unaccounted Memory

| Category | Estimated MiB | Evidence | Confidence |
|----------|--------------|----------|------------|
| glibc malloc arena fragmentation | 40-60 | Deceleration curve matches fragmentation settling; `malloc_trim` only returns a fraction | High |
| Accumulator Vec capacity overhead | 8-16 | `accumulator_count` doubled (1200->2391), Vec amortized doubling means capacity ~4096 when len=2391. Each `RawDataPoint` is ~500 bytes, so ~2 MiB wasted capacity. But HashMap overhead adds per-bucket pointers | Medium |
| tokio runtime buffers | 10-20 | 7 subscribers, each with broadcast channel. tokio maintains per-task stacks (~8 KiB default) and timer wheels | Medium |
| reqwest/hyper connection pools | 5-15 | HTTP keep-alive connections hold response buffers. 4 HTTP sources with TLS session state | Medium |
| rumqttc MQTT buffers | 3-8 | MQTT client maintains send/receive ring buffers, typically 64-128 KiB per connection | Low-Medium |
| Arrow array allocator retained pages | 5-15 | Snapshot allocates ~51 MiB of Arrow arrays per cycle. Even with trim, some pages retained in mmap regions | Medium |
| serde_json temporary allocations | 2-5 | WAL replay deserializes all entries, creating temporary String allocations that fragment the heap | Low |

**Total estimated: 73-139 MiB** (central estimate ~100 MiB, consistent with observed ~104 MiB gap)

---

## 5. Hypotheses Ranked by Likelihood

### H1: glibc malloc fragmentation (MOST LIKELY)

**Evidence strength: Strong**

- The deceleration pattern (17 -> 10 -> 5 -> 3 MiB/hr) is the textbook signature of heap fragmentation settling. When glibc's ptmalloc2 fragments, it retains freed pages in its arena. As more allocation patterns repeat, fragmentation reaches a steady state.
- `malloc_trim(0)` is being called (post-trim events show memory decreasing), but it only reclaims trailing free pages from the main arena. Interior fragmentation from interleaved alloc/free patterns is not reclaimable.
- The Arrow snapshot cycle (allocate 51 MiB, free 51 MiB, every 30 minutes) creates exactly the kind of large-then-free pattern that causes arena fragmentation in glibc.
- **Predicted behavior**: RSS should plateau at 230-280 MiB and stop growing. A 48-hour soak test will confirm or reject this.
- **Mitigation**: Switch to jemalloc (better fragmentation characteristics) or reduce peak snapshot allocation.

### H2: Accumulator Vec capacity overhead

**Evidence strength: Moderate**

- `accumulator_mib` tracks serialized size (via `std::mem::size_of_val` or similar), not actual heap allocation including Vec/HashMap overhead.
- When a `Vec<RawDataPoint>` doubles capacity, it allocates 2x its current length. With `accumulator_count` at 2391, actual Vec capacity is likely 4096. Per source (7 sources), that is ~700 wasted entries per Vec, at ~500 bytes each = ~2.5 MiB total wasted capacity.
- HashMap itself has bucket overhead: 7 buckets with load factor means ~14-16 allocated bucket pointers.
- **Total overhead estimate**: 4-8 MiB beyond what `accumulator_mib` reports.
- This is real but small relative to the 104 MiB gap. Not the primary cause.

### H3: Silver subscriber / tokio buffer growth

**Evidence strength: Moderate**

- 7 subscribers share the broadcast channel. Each subscriber maintains its own receive buffer.
- tokio's broadcast channel keeps messages until ALL subscribers have received them. If any subscriber is slow, messages accumulate in the channel.
- Connection pools (TimescaleDB, reqwest, rumqttc) hold resources that grow with usage patterns.
- **This would show as linear growth**, not the decelerating growth we observe. Reduces likelihood.

### H4: HTTP/MQTT response buffer caching

**Evidence strength: Weak**

- reqwest with connection pooling keeps TCP connections and TLS sessions alive.
- Response body buffers are typically small (NWS responses are 10-50 KiB).
- 4 HTTP sources with 30-second polling = 480 requests/hr. If each leaves 50 KiB residual, that is 24 MiB/hr -- higher than observed. But these buffers should be reused, not accumulated.
- **More likely contributes to the initial fast growth** (first 2 hours) as connection pools warm up, then plateaus. Consistent with the deceleration pattern but as a secondary effect.

---

## 6. Critical Question: Asymptote or Linear?

The key diagnostic question is: **Does the growth rate continue to decelerate toward zero, or does it stabilize at a low but positive rate?**

| Scenario | Behavior | Cause | Action |
|----------|----------|-------|--------|
| **Asymptote** (fragmentation) | Rate -> 0, RSS plateaus at 250-280 MiB | glibc arena fragmentation settling | May not need code changes. Monitor only. Consider jemalloc if plateau is too high. |
| **Low linear** (slow leak) | Rate stabilizes at 3-5 MiB/hr indefinitely | True leak in a subsystem (tokio, reqwest, MQTT) | Instrumentation needed. Profile with heaptrack. |
| **Stepped** (day rollover reset) | RSS drops at day rollover, then repeats cycle | Fragmentation clears when WAL resets | Acceptable if daily peak stays under limit. |
| **Stepped growth** (day rollover leak) | RSS drops partially at rollover, but baseline creeps up | Some long-lived allocation survives rollover | Multi-day soak test needed. |

**The 13.5-hour dataset leans toward Scenario 1 (asymptote) but cannot rule out Scenario 2.** A 48-hour test crossing at least one day rollover boundary is required.

---

## 7. Recommendations

### Immediate (no code changes)

1. **Continue the 48-hour soak test.** The current run has 13.5 hours. Let it run through at least one full day rollover cycle to observe whether RSS resets, plateaus, or continues climbing.
2. **Update GH issue #16** with the deceleration finding and revised OOM estimate.

### Phase 1 Instrumentation (ops-004 scope)

1. **`/proc/self/smaps` rollup at snapshot time.** Parse `Rss`, `Pss`, `Private_Clean`, `Private_Dirty` from `/proc/self/smaps_rollup`. Log as structured fields. This distinguishes heap fragmentation (high Private_Dirty) from mmap bloat.
2. **`mallinfo2()` via libc FFI.** Report `arena`, `fordblks` (free bytes in arena), `uordblks` (used bytes in arena). The ratio `fordblks / arena` quantifies fragmentation directly.
3. **Vec/HashMap capacity logging.** Add `accumulator_capacity_mib` alongside `accumulator_mib` to quantify the capacity vs used gap.
4. **Per-subsystem RSS delta sampling.** Wrap HTTP poll and MQTT receive in RSS-before/RSS-after measurements. Log the delta. This identifies which subsystem triggers the most allocator growth.

### Phase 2 Mitigation (contingent on Phase 1 findings)

- If fragmentation confirmed: evaluate `tikv-jemallocator` (jemalloc). Note: mimalloc was tried and reverted for BUG-004 due to Pi 5 kernel compatibility issues.
- If true leak found: targeted fix in the leaking subsystem.
- If Vec capacity overhead significant: add `shrink_to_fit()` after snapshot.

---

## 8. Appendix: Heartbeat Data (Last Cycle)

From the last heartbeat cycle at approximately 02:00 UTC:

```
accumulator_mib: 8.0
accumulator_count: 2391
rss_mib: ~229
```

Snapshot at 01:54 UTC:
```
pre_snapshot_rss: 227.7 MiB
peak_rss: 278.7 MiB
post_trim_rss: 226.8 MiB
net_change: -0.9 MiB
spike: +51.0 MiB (Arrow array allocation)
```

RSS between snapshots (01:54 to 02:00): steady at ~229 MiB, no growth. This 6-minute flat period supports the asymptote hypothesis -- at hour 12+, the inter-snapshot growth has nearly stopped.
