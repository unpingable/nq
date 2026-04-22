-- Migration 034: SMART witness ingestion spine.
--
-- Phase 1 of docs/gaps/SMART_* (profile at ~/git/nq-witness/profiles/smart.md):
-- the NQ side can ingest, store, and surface a conforming nq.witness.smart.v0
-- report. No detectors; raw evidence only.
--
-- Structurally mirrors 031_zfs_witness.sql with three key differences:
--
--   1. Coverage is two-tier (per the SMART profile): witness-level coverage
--      is just device_enumeration, and each device observation carries its
--      own per-device coverage block. That second tier is a separate table
--      (smart_device_coverage_current) keyed by (host, subject, tag).
--
--   2. Observations are a single kind (smart_device), so there's one
--      observation table instead of four. The table carries both
--      protocol-specific subsets (NVMe health log fields, SCSI error
--      counter fields) as nullable columns — a column is null when the
--      class doesn't apply, which matches the witness's explicit-null
--      discipline.
--
--   3. Raw payload is stored as TEXT (bounded at 32 KiB per profile).
--      Kept for forensic queries and later detector work. Truncation
--      bookkeeping (raw_truncated, raw_original_bytes, raw_truncated_bytes)
--      is stored on the observation row so consumers don't have to infer.
--
-- See ~/git/nq-witness/profiles/smart.md for the contract this schema mirrors.

-- Widen collector_runs.collector CHECK to accept 'smart_witness'.
-- Same rebuild pattern as migrations 017 and 031.
CREATE TABLE collector_runs_v4 (
    generation_id      INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    source             TEXT NOT NULL,
    collector          TEXT NOT NULL CHECK (collector IN ('host', 'services', 'sqlite_health', 'prometheus', 'logs', 'zfs_witness', 'smart_witness')),
    status             TEXT NOT NULL CHECK (status IN ('ok', 'error', 'timeout', 'skipped')),
    collected_at       TEXT,
    entity_count       INTEGER,
    error_message      TEXT,
    PRIMARY KEY (generation_id, source, collector)
);
INSERT INTO collector_runs_v4 SELECT * FROM collector_runs;
DROP TABLE collector_runs;
ALTER TABLE collector_runs_v4 RENAME TO collector_runs;
CREATE INDEX idx_collector_runs_source_gen ON collector_runs(source, generation_id DESC);

-- Witness metadata: one row per publisher host. Upserted on every
-- successful collection that carries a SMART witness payload.
CREATE TABLE smart_witness_current (
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

-- Witness-level coverage: device_enumeration and (optionally)
-- smartd_cache_readable. Distinct from per-device coverage below.
CREATE TABLE smart_witness_coverage_current (
  host         TEXT NOT NULL,
  tag          TEXT NOT NULL,
  can_testify  INTEGER NOT NULL CHECK (can_testify IN (0, 1)),
  PRIMARY KEY (host, tag)
);

-- Standing block. Identical shape to zfs_witness_standing_current.
CREATE TABLE smart_witness_standing_current (
  host      TEXT NOT NULL,
  fact      TEXT NOT NULL,
  standing  TEXT NOT NULL CHECK (standing IN ('authoritative', 'advisory', 'inadmissible')),
  PRIMARY KEY (host, fact)
);

-- Per-device observations. One row per (host, subject). Subject carries
-- the tier prefix (wwn:/serial:/path:) from the profile, so consumers
-- can classify stability tier by pattern-matching the prefix.
CREATE TABLE smart_devices_current (
  host                        TEXT NOT NULL,
  subject                     TEXT NOT NULL,
  device_path                 TEXT NOT NULL,
  device_class                TEXT NOT NULL CHECK (device_class IN ('nvme','scsi','ata','usb_bridge','unknown')),
  protocol                    TEXT NOT NULL,
  collection_outcome          TEXT NOT NULL CHECK (collection_outcome IN ('ok','partial','unsupported','permission_denied','timeout','error')),

  -- Identity / capacity
  model                       TEXT,
  serial_number               TEXT,
  firmware_version            TEXT,
  capacity_bytes              INTEGER,
  logical_block_size          INTEGER,

  -- SMART availability / overall status (the latter is advisory per profile)
  smart_available             INTEGER CHECK (smart_available IN (0, 1)),
  smart_enabled               INTEGER CHECK (smart_enabled IN (0, 1)),
  smart_overall_passed        INTEGER CHECK (smart_overall_passed IN (0, 1)),

  -- Cross-class normalized fields
  temperature_c               INTEGER,
  power_on_hours              INTEGER,

  -- SCSI-only error counter normalizations. Null on NVMe and ATA.
  uncorrected_read_errors     INTEGER,
  uncorrected_write_errors    INTEGER,
  uncorrected_verify_errors   INTEGER,

  -- NVMe-only fields. Null on SCSI and ATA.
  media_errors                INTEGER,
  nvme_percentage_used        INTEGER,
  nvme_available_spare_pct    INTEGER,
  nvme_critical_warning       INTEGER,
  nvme_unsafe_shutdowns       INTEGER,

  -- Raw smartctl subtree for forensic queries. Bounded at 32 KiB by the
  -- witness; NQ stores what arrives verbatim. NULL if omitted by witness.
  raw_json                    TEXT,
  raw_truncated               INTEGER CHECK (raw_truncated IN (0, 1)),
  raw_original_bytes          INTEGER,
  raw_truncated_bytes         INTEGER,

  as_of_generation            INTEGER NOT NULL,
  collected_at                TEXT NOT NULL,

  PRIMARY KEY (host, subject)
);

CREATE INDEX smart_devices_current_by_host
  ON smart_devices_current(host);

-- Per-device coverage: which attribute groups each specific device can
-- testify about. Keyed by (host, subject, tag). This is the structural
-- difference between SMART and ZFS profiles — SMART coverage varies
-- per device (NVMe vs SCSI vs ATA vs USB bridge), so the second tier
-- lives in its own table rather than collapsing into witness-level.
CREATE TABLE smart_device_coverage_current (
  host         TEXT NOT NULL,
  subject      TEXT NOT NULL,
  tag          TEXT NOT NULL,
  can_testify  INTEGER NOT NULL CHECK (can_testify IN (0, 1)),
  PRIMARY KEY (host, subject, tag)
);

-- Collection errors emitted by the witness itself. Mirrors
-- zfs_witness_errors_current one-for-one.
CREATE TABLE smart_witness_errors_current (
  host         TEXT NOT NULL,
  ordinal      INTEGER NOT NULL,
  kind         TEXT NOT NULL,
  detail       TEXT NOT NULL,
  observed_at  TEXT NOT NULL,
  PRIMARY KEY (host, ordinal)
);

-- Witness freshness view, parallel to v_zfs_witness.
CREATE VIEW v_smart_witness AS
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
FROM smart_witness_current;

-- Operator-facing device surface. Joins device observations with the
-- witness's freshness so a query can reason about "drive reports 88
-- uncorrected errors, witness last reported 12 seconds ago" in one step.
CREATE VIEW v_smart_devices AS
SELECT
  d.host,
  d.subject,
  d.device_path,
  d.device_class,
  d.protocol,
  d.collection_outcome,
  d.model,
  d.serial_number,
  d.firmware_version,
  d.capacity_bytes,
  d.logical_block_size,
  d.smart_available,
  d.smart_enabled,
  d.smart_overall_passed,
  d.temperature_c,
  d.power_on_hours,
  d.uncorrected_read_errors,
  d.uncorrected_write_errors,
  d.uncorrected_verify_errors,
  d.media_errors,
  d.nvme_percentage_used,
  d.nvme_available_spare_pct,
  d.nvme_critical_warning,
  d.nvme_unsafe_shutdowns,
  d.raw_truncated,
  d.as_of_generation,
  d.collected_at,
  w.witness_status,
  w.witness_collected_at,
  CAST((JULIANDAY('now') - JULIANDAY(w.received_at)) * 86400 AS INTEGER) AS received_age_s
FROM smart_devices_current d
LEFT JOIN smart_witness_current w ON w.host = d.host;
