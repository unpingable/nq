# Gap: History Compaction — chunked storage for older temporal evidence

**Status:** proposed
**Depends on:** none (orthogonal to REGIME_FEATURES_GAP, which consumes history regardless of storage layout)
**Build phase:** infrastructure — storage efficiency, not new semantic capability
**Blocks:** nothing critical. Useful when retention windows grow or when federation introduces multiple instances' histories needing storage.
**Last updated:** 2026-04-14

## The Problem

NQ stores temporal evidence in three hot append-only tables:
- `metrics_history` — per-generation metric samples
- `hosts_history` — per-generation host resource state
- `finding_observations` — per-generation finding emissions

Every row carries SQLite row overhead, a `generation_id`, denormalized identity columns (host, detector_id, subject, etc.), and the actual value or evidence. At 60s intervals, that adds up fast. A single metric reporting for a year is 525,960 rows. Multiply by series count and you're storing numbers with more metadata than data.

**The gap is not that NQ lacks a TSDB.** The store spine is fine. The gap is that older history is stored in the same fat-row format as hot history, and there's no compaction path.

This gap is explicitly **not REGIME_FEATURES_GAP**. That one compiles history into typed facts. This one stores history more efficiently. They're complementary — features consume history regardless of storage layout, and compaction helps even if the feature layer never ships.

## Design Stance

**Generation is the time axis.** NQ's primary clock is the generation_id, not wall-clock time. For contiguous per-generation observations, the time axis is implicit — chunks can store `start_generation` + `sample_count` and the rest is positional. This kills a huge amount of overhead that general-purpose TSDBs pay to handle irregular timestamps.

**Two shapes, two storage strategies.** Don't force everything into one layout.

1. **Dense numeric series** — metrics, host resource state. One value per generation, slowly changing. Compress with delta encoding + run-length compression.
2. **Sparse finding state** — finding presence/absence across generations. Store as runs (start_gen, end_gen, state), not one row per generation.

The same table layout cannot serve both well. Dense wants compressed blobs; sparse wants run rows.

**Scaled ints over floats.** Store `disk_used_pct = 7342` with `scale = 100`, not `73.42` as f64. Most NQ metrics are either already integer (byte counts, page counts) or bounded decimals that fit in integer * scale. Integer delta codecs are simpler and more effective than float codecs. Defer float codec support until a metric genuinely needs it.

**Immutable compacted chunks.** Once a chunk is written, it is never modified. Compaction errors emit replacement chunks or rebuild from raw. This keeps correctness simple and makes chunk validation trivially re-runnable.

## Architectural Invariants

The following are frozen as doctrine. They affect every downstream decision and should not be revisited casually. If one proves wrong, revisit the whole section, not individual items.

### Identity and semantics

1. **Generation is the primary time axis.** Not wall-clock. Wall-clock is metadata/rendering only. Chunk semantics, windows, and feature computation key off generation. No silent reimport of fake continuity.
2. **Scale or unit changes force a new series.** If `disk_used_pct` changes scale (10 → 100) or `mem` changes unit (MiB → bytes), allocate a new `series_id`. Unit drift in the same lineage is how you invent nonsense with confidence.
3. **Series identity is stable across metric renames.** Rename = metadata update on the same `series_id`. Changing the internal identity on rename fabricates discontinuity the system didn't actually experience.

### Storage separation

4. **Metadata lives outside the blob.** `series_id`, generation bounds, sample count, encoding name, scale, checksum — all in SQLite columns. The blob is only the compressed value stream. Keeps pruning, lookup, and corruption handling sane.
5. **Sparse state is represented semantically, not as a codec.** `finding_runs` (start_gen, end_gen, state) is the canonical compressed form for finding presence/absence. Do not "optimize" it into opaque blobs. Runs are already the right data model.
6. **Scaled-int normalization happens before compression.** The codec only sees integers. How `73.42%` becomes `7342` is a series-registration decision, not a codec concern. Keeps codecs dumb and reusable.
7. **Codec negotiation is explicit.** `encoding = 'delta_i64_v1'` is schema-level metadata. Future codecs coexist; no silent reinterpretation, no "v2 decoder is backwards-ish compatible probably."
8. **Payload format is endian-independent by construction.** Varints already handle this, but if fixed-width fields ever appear in payloads, specify byte order. Cross-machine archaeology is tedious enough.

### Compaction discipline

9. **Chunks are immutable.** No in-place edit. Fix by replacement (emit a new chunk covering the range) or rebuild from raw.
10. **No overlapping chunks for the same series.** For a given `series_id`, chunk generation ranges do not overlap. Replacement is explicit, not accidental coexistence.
11. **Chunk write is transactional.** A chunk exists only if metadata row + payload + verification all commit together. Never "metadata row exists but payload is half a thought."
12. **Verification is mandatory, not aspirational.** The compactor's flow is: read raw slice → encode → decode → byte-compare against source → only then commit chunk and consider raw eviction. No shortcut.
13. **Contiguous-only chunks in v1.** If the compactor encounters a gap, it emits two chunks, not one with a bitmap. Missing generations break continuity; no silent imputation.
14. **Compaction is subject-local.** One noisy host/database/subject does not drag unrelated history into the same maintenance action. Locality aligns with series/subject boundaries.
15. **Compaction boundaries are deterministic.** Same input history yields the same chunk boundaries every time. Boundary choice does not depend on load, mood, or batching artifacts — unless that policy is explicitly modeled.
16. **Compaction is restart-safe.** Process death mid-compaction leaves either intact raw rows or a completed chunk. Never "history vanished into a half-written idea."
17. **Compaction is observable.** The compactor emits enough telemetry/facts to answer: rows compacted, chunk count, bytes before/after, verification failures, skipped ranges and why. The storage layer is not a dark forest under the epistemic machine.
18. **Rebuild from raw is a first-class escape hatch.** Compressed history is reproducible from raw history for as long as raw is retained. A codec bug should never become literature.

### Reader contract

19. **Hot rows win on overlap.** If a query spans both hot rows and compacted chunks covering the same generation, hot rows authoritatively overlay chunk data. Overlap ambiguity is how history gets weird.
20. **Query API is storage-agnostic.** Same semantic slice in → same reconstructed series out. Callers do not know whether data came from raw rows, chunk payloads, or finding runs.
21. **Queries decode the minimum necessary.** Prune by metadata in SQLite → decode only overlapping chunks → merge with hot rows. No "decompress the archive and ask questions later."
22. **Corruption behavior is explicit.** When a chunk fails to decode or a checksum mismatches, the reader's response is documented, not left to vibes. v1 default: hard fail on mismatch, log loudly, do not silently drop chunks.
23. **Derived facts never depend on blob internals.** `regime_features` and diagnosis read reconstructed series, not raw payload bytes. Compression is a storage concern, not an epistemic one. This separation is load-bearing — if it breaks, compression changes start mutating semantic outputs.

### Policy separation

24. **A hot raw window is always retained.** Retention policy must guarantee some recent generations stay uncompressed. Makes debugging, codec rollout, and edge-case forensics possible.
25. **Retention and compaction are separate policies.** "Eligible for compaction" and "eligible for deletion" are different questions. Valid to compact after N generations, delete raw after M, retain some series raw indefinitely.
26. **Compression is opportunistic, not constitutional.** Some series will compress well; some will be rude. The invariant is correctness and bounded operational cost, not "every chunk gets smaller."
27. **Compression is optional per series class.** Not every series deserves chunking. Leave series raw if cardinality is low, history is shallow, access is hot/random, or compression benefit is negligible. Otherwise you compress compulsively because the framework exists.

### Deliberately deferred (do NOT freeze in v1)

The following are explicitly not decided now. Revisit after v1 ships:
- Predicate pushdown into blobs
- Min/max side indexes inside chunk payloads
- Delta-of-delta codec
- Float codecs
- Gap bitmaps (contiguous-only is v1)
- Cross-chunk summary indexes
- Automatic retention-aware compaction thresholds

## Three-Tier Model

```
Tier 1: raw hot tables         (metrics_history, hosts_history, finding_observations)
          ↓ (background compactor)
Tier 2: compacted history      (metric_chunks, finding_runs)
          ↓ (feature pass, per REGIME_FEATURES_GAP)
Tier 3: derived temporal facts (regime_features)
```

Tier 1 is the current state. Tier 2 is this gap. Tier 3 is REGIME_FEATURES.

The compactor runs as a background task: for each series, when raw rows older than a threshold accumulate into a contiguous run of at least N generations (e.g. 128), encode them into a chunk and delete the raw rows after a verification pass.

## Shape 1: Dense Numeric Chunks

```sql
CREATE TABLE series_registry (
    series_id      INTEGER PRIMARY KEY,
    subject_kind   TEXT NOT NULL,       -- 'host' / 'database' / etc.
    subject_id     TEXT NOT NULL,
    metric_name    TEXT NOT NULL,
    value_type     TEXT NOT NULL,       -- 'int' / 'scaled_int' / 'float'
    scale          INTEGER,             -- e.g. 100 for percent*100
    UNIQUE(subject_kind, subject_id, metric_name)
);

CREATE TABLE metric_chunks (
    series_id         INTEGER NOT NULL REFERENCES series_registry(series_id),
    start_generation  INTEGER NOT NULL,
    end_generation    INTEGER NOT NULL,
    sample_count      INTEGER NOT NULL,
    encoding          TEXT NOT NULL,    -- 'delta_i64_v1' / future codecs
    scale             INTEGER,
    payload           BLOB NOT NULL,
    payload_crc32     INTEGER,
    PRIMARY KEY (series_id, start_generation)
);

CREATE INDEX idx_metric_chunks_range ON metric_chunks(series_id, end_generation);
```

Query flow:
1. Find chunks where `series_id = ?` and `start_generation <= gen_hi` and `end_generation >= gen_lo`
2. Decode only those chunks
3. Merge with any still-hot rows from `metrics_history`
4. Return series to consumer

## Shape 2: Sparse Finding Runs

```sql
CREATE TABLE finding_runs (
    finding_key        TEXT NOT NULL,     -- canonical identity from compute_finding_key()
    subject_kind       TEXT NOT NULL,
    subject_id         TEXT NOT NULL,
    state              TEXT NOT NULL,     -- 'observed' / 'suppressed' / enum
    start_generation   INTEGER NOT NULL,
    end_generation     INTEGER NOT NULL,
    -- Denormalized fields for query convenience
    kind               TEXT NOT NULL,
    host               TEXT NOT NULL DEFAULT '',
    subject            TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (finding_key, start_generation)
);

CREATE INDEX idx_finding_runs_range ON finding_runs(finding_key, end_generation);
CREATE INDEX idx_finding_runs_host ON finding_runs(host, start_generation DESC);
```

This layout gives streak length, recurrence interval, and interruption count for free:
- Streak length of current run = `end_generation - start_generation + 1`
- Recurrence interval = gap between consecutive runs of the same `finding_key`
- Interruption count in a window = number of runs in that window

No blob decoding needed. The schema IS the feature.

## Codec: `delta_i64_v1` (Proposed)

The first concrete codec for dense numeric chunks. Deliberately simple.

**Payload layout:**
```
version (u8) | flags (u8) | first_value (zigzag varint) | op stream
```

**Three opcodes** (top 2 bits of control byte = kind, low 6 bits = run length minus 1, with varint extension for long runs):

| Kind | Opcode | Payload |
|------|--------|---------|
| 0 | Literal run | `n` zigzag varints (arbitrary deltas) |
| 1 | Zero run | none (repeat previous value `n` times) |
| 2 | Repeated-delta run | one zigzag varint (add same delta `n` times) |
| 3 | Reserved | — |

**Encoding heuristics** (greedy, left-to-right):
1. If 2+ consecutive zero deltas: zero run
2. Else if 3+ consecutive identical nonzero deltas: repeated-delta run
3. Else: literal run until one of the above patterns

**Why these three:**
- Zero runs are the biggest win (flat metrics = near-zero storage)
- Repeated-delta covers steady growth (counters, monotonic series)
- Literal mops up everything else

**Deliberately omitted from v1:**
- Delta-of-delta (useful for very smooth monotonic series; defer)
- Bit-packed blocks (premature)
- SIMD decode (premature)
- Gap bitmaps (contiguous-only v1)
- Chunk-local min/max index (add when query pruning needs it)

## Non-Goals

- **A new database.** This is SQLite tables with BLOB payloads. The codec fits in one Rust file.
- **Float compression.** Defer until a metric genuinely requires it. Scaled ints cover 95% of NQ use cases.
- **Compression across series.** Each chunk = one series. Mixing kills compression and complicates queries.
- **Mutable chunks.** Immutable. Fix by replacement, not edit.
- **Wall-clock-based storage.** Generation is the clock. If you want wall-clock queries, join against the `generations` table.
- **Automatic retention-aware compaction thresholds.** Start with a fixed "older than N generations" rule. Tune later if needed.
- **Cross-metric codecs.** Each series encodes independently.

## V1 Slice

1. **Series registry table.** One row per unique (subject_kind, subject_id, metric_name). Assigns series_id.
2. **metric_chunks table + index.** The append-only compacted store.
3. **finding_runs table + index.** Run-based finding history.
4. **`delta_i64_v1` codec.** Encoder + decoder + round-trip tests.
5. **Background compactor.** For each series with enough contiguous raw rows older than threshold, encode a chunk, verify round-trip, delete raw rows.
6. **Read path.** Given a series and generation range, return merged compacted + hot samples.

That's enough to prove the architecture. Does not require a feature pass, regime features, or any detector changes.

## Boundary Discipline

- **Verify before delete.** The compactor must decode what it just encoded and byte-compare against the source slice before deleting any raw rows. Paranoia is earned; compression bugs silently rewriting history is the kind of sentence that ruins a week.
- **Keep hot window uncompressed.** Recent generations stay in raw tables until they age out. This keeps the write path simple and makes the compactor itself a pure read-and-transform operation over aged data.
- **Payload checksum.** `payload_crc32` is external to the codec. Detects blob corruption independent of SQLite. Cheap, catches exotic failures.
- **Contiguous-only chunks.** If the compactor encounters a gap, it emits two chunks, not one chunk with a bitmap. Simpler.

## Open Questions

- **Chunk size.** 128 samples? 256? 512? Favor smaller chunks for faster decode; favor larger for better compression. 256 is probably the right v1 default.
- **Compaction threshold.** "Older than N generations" — what N? Probably `max_generations / 4` so most of the retention window is compacted.
- **Do we ever decompact?** (Need raw rows back for debugging a specific generation.) Probably yes eventually. For v1, if you need raw access, compaction can be disabled or the chunk decoded to a temp table.
- **Finding_runs compaction timing.** finding_observations is already append-only and relatively compact. Is run-based storage worth the migration cost? Measure first. Metrics are the bigger win.

## Acceptance Criteria

1. `metric_chunks`, `finding_runs`, and `series_registry` tables exist with appropriate indexes.
2. `delta_i64_v1` codec has encoder, decoder, and round-trip tests for at least flat, monotonic, oscillating, and mixed series.
3. Background compactor runs on a schedule and successfully compacts at least one series.
4. Verification pass (decode + byte-compare against source) gates every delete.
5. Read path correctly merges compacted chunks with hot rows.
6. No raw data is lost across a compaction cycle.
7. A `payload_crc32` mismatch is detectable and surfaces loudly.
8. Hot tables (`metrics_history`, etc.) shrink after compaction runs in steady state.

## Measurement Expectations

Before shipping, compare:
- Disk usage of a year of raw `metrics_history` rows vs. the same data as chunks
- Query time for a 30-day window in each layout

If the compaction win is less than 5x on storage for typical NQ metrics, revisit the codec or the table layout. If query time degrades by more than 2x, the read path needs work before shipping.

## Relationship to REGIME_FEATURES_GAP

- REGIME_FEATURES computes typed facts from history. It reads from whatever storage layout exists.
- HISTORY_COMPACTION changes that storage layout for older data.
- Neither blocks the other. Build whichever brings more value first.

My read: REGIME_FEATURES first (it's the semantic payload), HISTORY_COMPACTION when storage becomes a real problem. On current NQ scale (one publisher, 60s polls, ~30 metrics), compaction is a distant concern. On federation scale (multiple instances × longer retention), it's more urgent.

## References

- docs/gaps/REGIME_FEATURES_GAP.md (the semantic layer that consumes history)
- docs/gaps/EVIDENCE_LAYER_GAP.md (finding_observations is the sparse evidence store)
- docs/gaps/GENERATION_LINEAGE_GAP.md (per-generation coverage — if a chunk spans low-coverage generations, that's relevant metadata)
