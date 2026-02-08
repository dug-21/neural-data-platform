# ADR-AIR017-001: WAL Position (Subscriber vs Store)

## Status

Proposed

## Context

The current WAL lives inside `ParquetStore` (`core/src/storage/parquet.rs:17`). It is
created by `ParquetStore::new()` at `{base_path}/wal.log` (line 25) and used exclusively
within `write_raw_batch()` (lines 710-741):

```
write_raw_batch():
  1. wal.lock().append(point) for each point   -- lines 716-721
  2. group points by partition path             -- lines 725-729
  3. append_to_raw_parquet() per partition      -- lines 732-734
  4. wal.lock().commit()                        -- lines 737-738
```

This means the WAL is written only when `BronzeSubscriber.flush()` calls
`store.write_raw_batch()`, which happens on the flush timer (every 30 seconds) or when
the batch buffer reaches `batch_size` (100 events). Events sit in `BronzeSubscriber.buffer`
(a `Vec<RawDataPoint>`, line 87) with no durability for up to 30 seconds after receipt.

The question is where WAL ownership should move to achieve immediate durability.

### Option A: WAL stays in ParquetStore

Keep WAL in `ParquetStore`, but BronzeSubscriber calls a new `store.wal_append()` method
on every event receipt. The store manages both WAL and Parquet.

Pros:
- Storage concerns stay in the storage layer.
- ParquetStore already has the WAL infrastructure.

Cons:
- BronzeSubscriber must call two store methods per event (`wal_append()` on receipt,
  `write_snapshot()` on timer). The store becomes a stateful service with two phases.
- The WAL's lifetime is tied to the store, but the snapshot decision (when to write Parquet)
  is made by the subscriber. This splits the durability contract across two components.
- `Arc<dyn RawStore>` would need WAL-specific methods added to the trait, which is a
  storage-implementation detail leaking into the trait.

### Option B: WAL moves to BronzeSubscriber

BronzeSubscriber creates and owns the WAL directly. On event receipt, it appends to WAL
and to the in-memory accumulator. On snapshot timer, it writes from accumulator to
ParquetStore, then commits the WAL.

Pros:
- Durability is a subscriber concern: the subscriber decides when data is durable.
- Single component owns the full lifecycle: event -> WAL -> accumulator -> snapshot.
- ParquetStore becomes simpler (no WAL, no state between calls).
- RawStore trait stays clean (no WAL methods).

Cons:
- BronzeSubscriber grows in responsibility (it is already ~346 lines).
- WAL is a storage mechanism being used by a subscriber. This crosses the traditional
  hexagonal architecture boundary where storage lives behind a trait.

### Option C: Dedicated WAL service (new component)

Extract WAL into a standalone `BronzeWalService` that both BronzeSubscriber and ParquetStore
interact with.

Pros:
- Clean separation of concerns.
- Reusable WAL for other subscribers.

Cons:
- Over-engineering for a single subscriber.
- Introduces shared mutable state (the WAL service) between components.
- No other subscriber needs a WAL -- Silver writes directly to TimescaleDB.

## Decision

**Option B: WAL moves to BronzeSubscriber.**

The WAL exists to provide durability, and the durability decision belongs to the component
that first receives data. BronzeSubscriber is that component. It receives events from the
EventBus and must ensure they are durable before any further processing.

The hexagonal architecture principle of "storage behind a trait" applies to the archival
concern (Parquet), not the durability concern (WAL). The WAL is a local implementation
detail of how BronzeSubscriber ensures crash safety. It is analogous to a database's
internal WAL -- the database client (ParquetStore) does not manage the WAL; the database
engine (BronzeSubscriber) does.

Concretely:

- `BronzeSubscriber::new()` takes a `wal_path: PathBuf` parameter and creates a
  `WriteAheadLog` instance.
- `BronzeSubscriber.start()` appends to WAL on event receipt (before accumulator insert).
- `BronzeSubscriber.start()` commits WAL after successful Parquet snapshot.
- `ParquetStore` drops its `wal` field and `wal.log` file creation.
- `ParquetStore::new()` signature simplifies to just `base_path`.

## Consequences

**Positive:**
- Durability latency drops from up to 30 seconds to milliseconds.
- ParquetStore becomes a pure archival storage backend with no internal state between calls.
- RawStore trait remains clean -- no WAL-specific methods.
- Testing is simpler: WAL behavior is tested on BronzeSubscriber directly, not through
  the store trait indirection.

**Negative:**
- BronzeSubscriber takes on more responsibility (~100 additional lines for WAL management,
  startup recovery, watermark tracking).
- The WAL file path is now configured on BronzeSubscriber, not on ParquetStore. The WAL
  file should still live under the Bronze data directory for consistency.
- If a future subscriber needs WAL behavior, the WAL code would need to be extracted
  or duplicated. This is acceptable because no other subscriber is planned to need a WAL.

**Neutral:**
- The WAL file format (JSON lines) is unchanged in Phase 1. Phase 2 adds sequence numbers
  to each line, which is a WAL-internal change regardless of ownership.
- Existing tests on `ParquetStore.write_raw_batch()` that verify WAL behavior will need
  to be migrated to BronzeSubscriber tests.
