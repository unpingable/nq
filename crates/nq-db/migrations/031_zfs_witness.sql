-- Migration 031: ZFS witness ingestion spine.
--
-- Phase A of docs/gaps/ZFS_COLLECTOR_GAP.md: the NQ side can ingest,
-- store, and surface a conforming nq-witness report from a publisher.
-- Detectors that gate off `coverage.can_testify` land in Phase B.
--
-- Current-gen only. No history tables. The witness report replaces
-- prior per-host state on each successful publish; partial collections
-- honestly demote coverage (handled in publish.rs, mirroring the SPEC
-- §Partial collection rule enforced on the witness side).
--
-- See ~/git/nq-witness/SPEC.md and ~/git/nq-witness/profiles/zfs.md
-- for the report shape this schema mirrors.

-- Widen the collector_runs CHECK constraint to accept 'zfs_witness'.
-- Same rebuild pattern as migration 017.
CREATE TABLE collector_runs_v3 (
    generation_id      INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    source             TEXT NOT NULL,
    collector          TEXT NOT NULL CHECK (collector IN ('host', 'services', 'sqlite_health', 'prometheus', 'logs', 'zfs_witness')),
    status             TEXT NOT NULL CHECK (status IN ('ok', 'error', 'timeout', 'skipped')),
    collected_at       TEXT,
    entity_count       INTEGER,
    error_message      TEXT,
    PRIMARY KEY (generation_id, source, collector)
);
INSERT INTO collector_runs_v3 SELECT * FROM collector_runs;
DROP TABLE collector_runs;
ALTER TABLE collector_runs_v3 RENAME TO collector_runs;
CREATE INDEX idx_collector_runs_source_gen ON collector_runs(source, generation_id DESC);

-- Witness metadata: one row per publisher host. Upserted on every
-- successful collection that carries a witness payload.
CREATE TABLE zfs_witness_current (
  host                  TEXT PRIMARY KEY,
  witness_id            TEXT NOT NULL,
  witness_type          TEXT NOT NULL,
  witness_host          TEXT NOT NULL,
  observed_subject      TEXT,
  profile_version       TEXT NOT NULL,
  collection_mode       TEXT NOT NULL,
  privilege_model       TEXT NOT NULL,
  witness_status        TEXT NOT NULL,
  witness_collected_at  TEXT NOT NULL,
  duration_ms           INTEGER,
  as_of_generation      INTEGER NOT NULL,
  received_at           TEXT NOT NULL
);

-- Coverage per (host, tag). Detectors query this table directly for
-- gating: the ZFS gap's normative rule is that a detector fires only
-- when every required tag has can_testify=1 in the current row set.
CREATE TABLE zfs_witness_coverage_current (
  host         TEXT NOT NULL,
  tag          TEXT NOT NULL,
  can_testify  INTEGER NOT NULL CHECK (can_testify IN (0, 1)),
  PRIMARY KEY (host, tag)
);

-- Standing: which facts the witness claims to be authoritative,
-- advisory, or explicitly inadmissible for. Renderers and reviewers
-- consume this to explain a finding's evidentiary basis.
CREATE TABLE zfs_witness_standing_current (
  host      TEXT NOT NULL,
  fact      TEXT NOT NULL,
  standing  TEXT NOT NULL CHECK (standing IN ('authoritative', 'advisory', 'inadmissible')),
  PRIMARY KEY (host, fact)
);

-- Pool observations: one row per (host, pool).
CREATE TABLE zfs_pools_current (
  host                 TEXT NOT NULL,
  pool                 TEXT NOT NULL,
  state                TEXT,
  health_numeric       INTEGER,
  size_bytes           INTEGER,
  alloc_bytes          INTEGER,
  free_bytes           INTEGER,
  readonly             INTEGER,
  fragmentation_ratio  REAL,
  as_of_generation     INTEGER NOT NULL,
  collected_at         TEXT NOT NULL,
  PRIMARY KEY (host, pool)
);

-- Vdev observations. Subject is the witness-declared identifier
-- ({pool}/{group}/{device} when a raidz/mirror group exists).
CREATE TABLE zfs_vdevs_current (
  host              TEXT NOT NULL,
  subject           TEXT NOT NULL,
  pool              TEXT NOT NULL,
  vdev_name         TEXT,
  state             TEXT,
  read_errors       INTEGER,
  write_errors      INTEGER,
  checksum_errors   INTEGER,
  status_note       TEXT,
  is_spare          INTEGER NOT NULL DEFAULT 0,
  is_replacing      INTEGER NOT NULL DEFAULT 0,
  as_of_generation  INTEGER NOT NULL,
  collected_at      TEXT NOT NULL,
  PRIMARY KEY (host, subject)
);

CREATE INDEX zfs_vdevs_current_by_pool
  ON zfs_vdevs_current(host, pool);

-- Scan (scrub/resilver) observations: one per (host, pool).
CREATE TABLE zfs_scans_current (
  host               TEXT NOT NULL,
  pool               TEXT NOT NULL,
  scan_type          TEXT,
  scan_state         TEXT,
  last_completed_at  TEXT,
  errors_found       INTEGER,
  as_of_generation   INTEGER NOT NULL,
  collected_at       TEXT NOT NULL,
  PRIMARY KEY (host, pool)
);

-- Spare observations: one per declared spare device.
CREATE TABLE zfs_spares_current (
  host                 TEXT NOT NULL,
  subject              TEXT NOT NULL,
  pool                 TEXT NOT NULL,
  spare_name           TEXT,
  state                TEXT,
  is_active            INTEGER NOT NULL DEFAULT 0,
  replacing_vdev_guid  TEXT,
  as_of_generation     INTEGER NOT NULL,
  collected_at         TEXT NOT NULL,
  PRIMARY KEY (host, subject)
);

-- Collection errors emitted by the witness itself. One row per entry
-- in the report's errors[] array. Ordinal preserves report order so
-- review can follow the witness's own failure narrative.
CREATE TABLE zfs_witness_errors_current (
  host         TEXT NOT NULL,
  ordinal      INTEGER NOT NULL,
  kind         TEXT NOT NULL,
  detail       TEXT NOT NULL,
  observed_at  TEXT NOT NULL,
  PRIMARY KEY (host, ordinal)
);

-- Current-gen witness surface. Augments the stored metadata with
-- witness_age_s (time since the witness collected its own data) and
-- received_age_s (time since NQ's publisher cycle). Detectors that
-- flag witness silence consume received_age_s.
CREATE VIEW v_zfs_witness AS
SELECT
  host,
  witness_id,
  witness_type,
  witness_host,
  observed_subject,
  profile_version,
  collection_mode,
  privilege_model,
  witness_status,
  witness_collected_at,
  duration_ms,
  as_of_generation,
  received_at,
  CAST((JULIANDAY('now') - JULIANDAY(received_at))          * 86400 AS INTEGER) AS received_age_s,
  CAST((JULIANDAY('now') - JULIANDAY(witness_collected_at)) * 86400 AS INTEGER) AS witness_age_s
FROM zfs_witness_current;

-- Current-gen pool surface. Joins pool observations with the
-- witness's status so a detector can reason about "pool DEGRADED,
-- but witness last reported N seconds ago" in one query.
CREATE VIEW v_zfs_pools AS
SELECT
  p.host,
  p.pool,
  p.state,
  p.health_numeric,
  p.size_bytes,
  p.alloc_bytes,
  p.free_bytes,
  p.readonly,
  p.fragmentation_ratio,
  p.as_of_generation,
  p.collected_at,
  w.witness_status,
  w.witness_collected_at
FROM zfs_pools_current p
LEFT JOIN zfs_witness_current w ON w.host = p.host;
