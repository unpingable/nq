-- Migration 064: GPU witness ingestion spine.
--
-- V0 of docs/working/gaps/GPU_WITNESS_GAP.md: the NQ side can ingest,
-- store, and surface a conforming nq.witness.gpu.v0 report. No
-- detectors; raw evidence only (same phase discipline as 031/034).
--
-- Structurally mirrors 034_smart_witness.sql with three differences:
--
--   1. Coverage is single-tier. SMART's per-device coverage exists
--      because device classes differ (NVMe vs SCSI vs ATA); a GPU
--      witness talks to one driver, and per-field absence is expressed
--      as nullable columns ([N/A] on consumer silicon → NULL, per the
--      explicit-null discipline).
--
--   2. Two observation tables: per-device state (gpu_devices_current)
--      and per-process VRAM holdings (gpu_compute_apps_current).
--      Compute-app process names are island-local evidence
--      (GPU_WITNESS_GAP.md custody note): they never leave this host's
--      own nq.db, and the public box carries no GPU witness config.
--
--   3. No raw payload column. The witness is embedded (nvidia-smi CSV,
--      not a JSON-emitting helper); every parsed field is already a
--      column, and the unparsed line is preserved in
--      gpu_witness_errors_current when malformed.
--
-- Widen collector_runs.collector CHECK to accept 'gpu_witness'.
-- Same rebuild pattern as 007, 017, 031, 034, 050, 055, 060.
CREATE TABLE collector_runs_v8 (
    generation_id      INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    source             TEXT NOT NULL,
    collector          TEXT NOT NULL CHECK (collector IN (
        'host',
        'services',
        'sqlite_health',
        'prometheus',
        'logs',
        'zfs_witness',
        'smart_witness',
        'gpu_witness',
        'sqlite_wal_probe',
        'nq_binary'
    )),
    status             TEXT NOT NULL CHECK (status IN ('ok', 'error', 'timeout', 'skipped', 'not_supported')),
    collected_at       TEXT,
    entity_count       INTEGER,
    error_message      TEXT,
    PRIMARY KEY (generation_id, source, collector)
);
INSERT INTO collector_runs_v8 SELECT * FROM collector_runs;
DROP TABLE collector_runs;
ALTER TABLE collector_runs_v8 RENAME TO collector_runs;
CREATE INDEX idx_collector_runs_source_gen ON collector_runs(source, generation_id DESC);

-- Witness metadata: one row per publisher host.
CREATE TABLE gpu_witness_current (
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

-- Witness-level coverage.
CREATE TABLE gpu_witness_coverage_current (
  host         TEXT NOT NULL,
  tag          TEXT NOT NULL,
  can_testify  INTEGER NOT NULL CHECK (can_testify IN (0, 1)),
  PRIMARY KEY (host, tag)
);

-- Standing block. Identical shape to the ZFS/SMART standing tables.
CREATE TABLE gpu_witness_standing_current (
  host      TEXT NOT NULL,
  fact      TEXT NOT NULL,
  standing  TEXT NOT NULL CHECK (standing IN ('authoritative', 'advisory', 'inadmissible')),
  PRIMARY KEY (host, fact)
);

-- Per-device observations. One row per (host, subject); subject is the
-- GPU UUID as reported by the driver (stable across reboots and
-- reordering, unlike index).
CREATE TABLE gpu_devices_current (
  host                        TEXT NOT NULL,
  subject                     TEXT NOT NULL,
  gpu_index                   INTEGER NOT NULL,
  name                        TEXT NOT NULL,
  collection_outcome          TEXT NOT NULL CHECK (collection_outcome IN ('ok','partial','error')),

  driver_version              TEXT,
  pstate                      TEXT,

  temperature_c               INTEGER,
  fan_speed_pct               INTEGER,

  utilization_gpu_pct         INTEGER,
  utilization_mem_pct         INTEGER,

  memory_total_mib            INTEGER,
  memory_used_mib             INTEGER,

  power_draw_w                REAL,
  power_limit_w               REAL,

  sm_clock_mhz                INTEGER,
  persistence_mode            TEXT,
  compute_mode                TEXT,

  -- Verbatim hex bitmask from clocks_throttle_reasons.active. Decoding
  -- the bits is detector work.
  throttle_reasons_active     TEXT,

  -- Volatile corrected ECC total. NULL on silicon without ECC ([N/A]
  -- on consumer cards) — absent, not zero.
  ecc_errors_corrected_total  INTEGER,

  as_of_generation            INTEGER NOT NULL,
  collected_at                TEXT NOT NULL,

  PRIMARY KEY (host, subject)
);

CREATE INDEX gpu_devices_current_by_host
  ON gpu_devices_current(host);

-- Per-process VRAM holdings. Island-local evidence: process names stay
-- in this host's own nq.db (custody note in GPU_WITNESS_GAP.md).
CREATE TABLE gpu_compute_apps_current (
  host             TEXT NOT NULL,
  gpu_uuid         TEXT,
  pid              INTEGER NOT NULL,
  process_name     TEXT,
  used_memory_mib  INTEGER,
  as_of_generation INTEGER NOT NULL,
  collected_at     TEXT NOT NULL,
  PRIMARY KEY (host, pid)
);

-- Collection errors emitted by the witness itself. Mirrors
-- zfs/smart_witness_errors_current one-for-one.
CREATE TABLE gpu_witness_errors_current (
  host         TEXT NOT NULL,
  ordinal      INTEGER NOT NULL,
  kind         TEXT NOT NULL,
  detail       TEXT NOT NULL,
  observed_at  TEXT NOT NULL,
  PRIMARY KEY (host, ordinal)
);

-- Witness freshness view, parallel to v_zfs_witness / v_smart_witness.
CREATE VIEW v_gpu_witness AS
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
FROM gpu_witness_current;

-- Operator-facing device surface. Joins device observations with the
-- witness's freshness so a query can reason about "GPU at 86% with
-- 12.9 GiB held, witness last reported 12 seconds ago" in one step.
CREATE VIEW v_gpu_devices AS
SELECT
  d.host,
  d.subject,
  d.gpu_index,
  d.name,
  d.collection_outcome,
  d.driver_version,
  d.pstate,
  d.temperature_c,
  d.fan_speed_pct,
  d.utilization_gpu_pct,
  d.utilization_mem_pct,
  d.memory_total_mib,
  d.memory_used_mib,
  d.power_draw_w,
  d.power_limit_w,
  d.sm_clock_mhz,
  d.persistence_mode,
  d.compute_mode,
  d.throttle_reasons_active,
  d.ecc_errors_corrected_total,
  d.as_of_generation,
  d.collected_at,
  w.witness_status,
  CAST((JULIANDAY('now') - JULIANDAY(w.received_at)) * 86400 AS INTEGER) AS witness_received_age_s
FROM gpu_devices_current d
LEFT JOIN gpu_witness_current w ON w.host = d.host;

-- Per-process VRAM surface with the same freshness join.
CREATE VIEW v_gpu_compute_apps AS
SELECT
  a.host,
  a.gpu_uuid,
  a.pid,
  a.process_name,
  a.used_memory_mib,
  a.as_of_generation,
  a.collected_at,
  w.witness_status,
  CAST((JULIANDAY('now') - JULIANDAY(w.received_at)) * 86400 AS INTEGER) AS witness_received_age_s
FROM gpu_compute_apps_current a
LEFT JOIN gpu_witness_current w ON w.host = a.host;
