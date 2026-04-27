-- Migration 037: SMART reallocated-sector raw evidence + history.
--
-- Phase 2 sibling to ZFS migration 032 (zfs_vdev_errors_history). The
-- ATA SMART attribute Reallocated_Sector_Ct (#5) is the canonical
-- "drive remapped a bad block" counter. Like ZFS error counters, it is
-- meaningful only as an edge: "did this drive remap MORE blocks since
-- last cycle?" Level-triggered would be useless on a fresh drive that
-- already has a small reallocated count from factory testing.
--
-- This migration:
--
--   1. Adds reallocated_sector_count to smart_devices_current as a new
--      ATA-only normalized field (NULL on NVMe, NULL on SCSI). Joins
--      the existing ATA-only attribute family deliberately small until
--      a real ATA device in fleet justifies more.
--
--   2. Creates smart_reallocated_history — narrow projection table
--      mirroring zfs_vdev_errors_history. One row per (generation,
--      host, device subject). Detector reads the two most recent rows
--      via window function and classifies the delta in Rust.
--
-- Schema choices match migration 032's discipline:
--   - subject is the same device subject used in smart_devices_current
--     (wwn:/serial:/path: prefix per profile).
--   - reallocated_sector_count is the only payload column; this is a
--     single-attribute history rather than a multi-counter family. If
--     a future ATA detector wants other attributes (pending_sector,
--     offline_uncorrectable, current_pending_sector), adding columns
--     here is the right shape.
--   - ON DELETE CASCADE on generation_id: retention drops old
--     generations and the history follows.
--
-- Witness coordination: the `nq-smart-witness` reference impl does not
-- yet emit reallocated_sector_count as a normalized field (raw_json
-- carries it but the structured column is empty). The `ata_smart_attributes`
-- coverage tag exists per the SMART profile and is set to can_testify=0
-- on every current device because the witness has no ATA support yet.
-- The detector this migration enables will stay correctly silent until
-- witness work catches up — gating discipline holds.

ALTER TABLE smart_devices_current ADD COLUMN reallocated_sector_count INTEGER;

CREATE TABLE smart_reallocated_history (
    generation_id            INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    host                     TEXT    NOT NULL,
    subject                  TEXT    NOT NULL,
    reallocated_sector_count INTEGER,
    collected_at             TEXT    NOT NULL,
    PRIMARY KEY (generation_id, host, subject)
);

CREATE INDEX idx_smart_reallocated_history_subject
  ON smart_reallocated_history(host, subject, generation_id DESC);
