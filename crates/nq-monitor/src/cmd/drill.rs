//! `nq drill wal-bloat` — D0-Origin cross-repo bridge entry point.
//!
//! Forcing case (operator-load-bearing): AG's D0-Origin slice
//! (`~/git/agent_gov/working/campaign-standing-before-spendability.md`
//! §3 D0-Origin) requires "real condition in a sandbox, real evaluator,
//! drill provenance minted at the witness layer." The condition is
//! manufactured (Night Shift stages a bloated WAL on a temp DB); the
//! evaluator is the production one (no `--fake-finding` flag, no
//! hardcoded `FindingSnapshot` shortcut). The bridge from operator-
//! staged condition to provenance-correct testimony is exactly this
//! CLI entry.
//!
//! What runs here, in order:
//!
//!   1. `sqlite_health::collect()` against the sandbox path. **Same
//!      function** the production `pull_all` invokes — stats the DB
//!      and `-wal` files for sizes, parses the SQLite header for
//!      `page_size`/`freelist`/`auto_vacuum`. Returns the same
//!      `SqliteDbData` shape.
//!   2. Build a synthetic `Batch` envelope around the real collector
//!      output and `nq_db::publish_batch()`. The envelope is the
//!      production type — `Batch`, `SqliteDbSet`, `SqliteDbRow` from
//!      `nq_core::batch`. We do not invent a parallel persistence
//!      pathway.
//!   3. `nq_db::detect::run_all()` reads `v_sqlite_dbs` (view of
//!      `monitored_dbs_current` we just populated) and runs the
//!      production `detect_wal_bloat`.
//!   4. `nq_db::update_warning_state_with_origin_mode()` upserts the
//!      findings into `warning_state`, stamping `origin_mode` with
//!      the operator-supplied value (default `drill`). This is the new
//!      seam landed alongside this command; the previous symmetry gap
//!      (import path threaded `origin_mode`; native detector path
//!      silently fell back to `'observed'`) is closed here.
//!   5. `nq_db::export_findings()` reads the `warning_state` rows back
//!      and emits canonical `nq.finding_snapshot.v1` JSON, one per
//!      line by default (jsonl).
//!
//! The cross-repo invariant Night Shift relies on: the FindingSnapshot
//! JSON byte-output of this command carries `origin_mode = "<chosen
//! mode>"` and is otherwise structurally identical to a production
//! snapshot from the same condition observed authentically. AG's
//! `governor.drill_runner` consumes the JSON verbatim and feeds the
//! shape through the cooked-context orchestrator — the FindingSnapshot
//! is the bridge wire DTO, not a fixture.

use crate::cli::DrillWalBloatCmd;
use nq_core::batch::{Batch, SqliteDbRow, SqliteDbSet};
use nq_core::status::{
    CollectorKind, CollectorStatus, ServiceStatus, SourceStatus,
};
use nq_core::PublisherConfig;
use nq_db::{
    export_findings, migrate, open_ro, open_rw, publish_batch,
    update_warning_state_with_origin_mode, DetectorConfig, EscalationConfig, ExportFilter,
};
use nq_witness::collect::sqlite_health;
use std::path::PathBuf;
use time::OffsetDateTime;

pub fn run(cmd: crate::cli::DrillCmd) -> anyhow::Result<()> {
    match cmd.action {
        crate::cli::DrillAction::WalBloat(c) => run_wal_bloat(c),
    }
}

fn run_wal_bloat(cmd: DrillWalBloatCmd) -> anyhow::Result<()> {
    // Resolve NQ-side DB path. Operator-supplied wins; otherwise a fresh
    // path under the system tmp dir. The DB is left on disk after the
    // command returns so Night Shift / AG can inspect it if needed.
    let db_path = match cmd.db {
        Some(p) => p,
        None => {
            let dir = std::env::temp_dir();
            dir.join(format!("nq-drill-{}.db", std::process::id()))
        }
    };

    // 1. Stand up the DB with current schema.
    let mut write_db = open_rw(&db_path)?;
    migrate(&mut write_db)?;

    // 2. Real production collector reads the sandbox substrate from
    //    disk. No synthetic SqliteDbData injection.
    let publisher_config = PublisherConfig {
        bind_addr: "127.0.0.1:0".to_string(),
        sqlite_paths: vec![cmd
            .sandbox_db
            .to_string_lossy()
            .into_owned()],
        service_health_urls: vec![],
        prometheus_targets: vec![],
        log_sources: vec![],
        zfs_witness: None,
        smart_witness: None,
        sqlite_wal_targets: vec![],
        sqlite_wal_proc_locks_enabled: true,
        nq_binary_path: None,
    };

    let payload = sqlite_health::collect(&publisher_config);

    // Honest-silence guard: if the collector returned a payload with no
    // observable DB rows, refuse loudly. A drill against a missing
    // substrate is exactly the laundering shape this slice exists to
    // refuse — we will not synthesize a `SqliteDbData` row to keep
    // going.
    let rows = match payload.data {
        Some(d) if !d.is_empty() => d,
        _ => {
            anyhow::bail!(
                "sqlite_health::collect produced no rows for sandbox path {:?}. \
                 D0-Origin refuses to fabricate substrate observation; either \
                 the sandbox was not staged, the sandbox path is unreadable, or \
                 the file is not SQLite-shaped.",
                cmd.sandbox_db
            );
        }
    };
    if payload.status != CollectorStatus::Ok {
        anyhow::bail!(
            "sqlite_health collector returned non-OK status {:?}: {:?}",
            payload.status,
            payload.error_message
        );
    }

    let collected_at = payload.collected_at.unwrap_or_else(OffsetDateTime::now_utc);
    let sqlite_db_set = SqliteDbSet {
        host: cmd.host.clone(),
        collected_at,
        rows: rows
            .iter()
            .map(|d| SqliteDbRow {
                db_path: d.db_path.clone(),
                db_size_mb: d.db_size_mb,
                wal_size_mb: d.wal_size_mb,
                page_size: d.page_size,
                page_count: d.page_count,
                freelist_count: d.freelist_count,
                journal_mode: d.journal_mode.clone(),
                auto_vacuum: d.auto_vacuum.clone(),
                last_checkpoint: d.last_checkpoint,
                checkpoint_lag_s: d.checkpoint_lag_s,
                last_quick_check: d.last_quick_check.clone(),
                last_integrity_check: d.last_integrity_check.clone(),
                last_integrity_at: d.last_integrity_at,
                db_mtime: d.db_mtime,
                wal_mtime: d.wal_mtime,
            })
            .collect(),
    };

    // 3. Build a one-source `Batch` and persist via the production
    //    `publish_batch`. The source/collector run rows declare the
    //    sandbox-drill identity so anyone inspecting the DB after this
    //    invocation can see exactly where the substrate came from.
    let cycle_started_at = OffsetDateTime::now_utc();
    let source_run = nq_core::batch::SourceRun {
        source: cmd.host.clone(),
        status: SourceStatus::Ok,
        received_at: cycle_started_at,
        collected_at: Some(collected_at),
        duration_ms: Some(0),
        error_message: None,
    };
    let collector_run = nq_core::batch::CollectorRun {
        source: cmd.host.clone(),
        collector: CollectorKind::SqliteHealth,
        status: CollectorStatus::Ok,
        collected_at: Some(collected_at),
        entity_count: Some(sqlite_db_set.rows.len() as u32),
        error_message: None,
    };
    let cycle_completed_at = OffsetDateTime::now_utc();
    let batch = Batch {
        cycle_started_at,
        cycle_completed_at,
        sources_expected: 1,
        source_runs: vec![source_run],
        collector_runs: vec![collector_run],
        host_rows: vec![],
        service_sets: vec![],
        sqlite_db_sets: vec![sqlite_db_set],
        metric_sets: vec![],
        log_sets: vec![],
        zfs_witness_rows: vec![],
        smart_witness_rows: vec![],
        wal_observation_sets: vec![],
        nq_binary_observation_rows: vec![],
    };

    let publish_result = publish_batch(&mut write_db, &batch)?;

    // 4. Run the production detectors. `run_all` reads `v_sqlite_dbs`
    //    which we just populated. The wal_bloat detector is the same
    //    code path that runs every minute in `serve`.
    let detector_config = DetectorConfig::default();
    let findings = nq_db::detect::run_all(write_db.conn(), &detector_config)?;

    // Honest-silence guard #2: if the detector did not flag the staged
    // condition, the staging was insufficient. Refuse loudly — D0-Origin
    // would otherwise launder by writing zero findings and still
    // emitting JSON.
    if findings.is_empty() {
        anyhow::bail!(
            "production detector pipeline ran against sandbox {:?} but produced \
             no findings. D0-Origin requires the staged condition to actually \
             trigger the real detector; the operator staging is insufficient \
             (e.g. WAL is too small relative to DB).",
            cmd.sandbox_db
        );
    }

    // 5. Persist findings through the production lifecycle, stamped
    //    with the operator-supplied `origin_mode`. The new
    //    `_with_origin_mode` entry is the symmetric counterpart to the
    //    import path's existing `origin_mode` plumbing.
    let escalation = EscalationConfig::default();
    update_warning_state_with_origin_mode(
        &mut write_db,
        publish_result.generation_id,
        &findings,
        &escalation,
        &[],
        &cmd.origin_mode,
    )?;

    // 6. Export snapshots. We must drop the writer before re-opening
    //    the same DB read-only — rusqlite holds the connection
    //    exclusively while `write_db` is in scope.
    drop(write_db);
    let read_db = open_ro(&db_path)?;
    let filter = ExportFilter {
        changed_since_generation: None,
        detector: Some("wal_bloat".to_string()),
        host: Some(cmd.host.clone()),
        finding_key: None,
        include_cleared: false,
        include_suppressed: false,
        observations_limit: 10,
    };
    let snapshots = export_findings(&read_db, &filter)?;
    if snapshots.is_empty() {
        anyhow::bail!(
            "export_findings returned zero snapshots after persistence. \
             This is a contract violation — findings were upserted but the \
             export filter does not match. Db path: {:?}",
            db_path
        );
    }

    // Final guard: every snapshot must carry the requested origin_mode.
    // The SQL CHECK constraint plus our `_with_origin_mode` plumbing
    // makes this a tautology, but asserting at the wire boundary
    // prevents any future regression from laundering an `observed`
    // value through.
    for s in &snapshots {
        if s.origin_mode != cmd.origin_mode {
            anyhow::bail!(
                "finding {:?} exported with origin_mode={:?}, expected {:?}",
                s.finding_key,
                s.origin_mode,
                cmd.origin_mode
            );
        }
    }

    match cmd.format.as_str() {
        "json" => println!("{}", serde_json::to_string_pretty(&snapshots)?),
        _ => {
            for s in &snapshots {
                println!("{}", serde_json::to_string(s)?);
            }
        }
    }
    Ok(())
}

// Compile-time suppression of the unused-import warning when the
// command is built but no caller needs every helper. Avoids the
// "unused import" rebuild flap if upstream type renames migrate.
#[allow(dead_code)]
fn _unused() -> PathBuf {
    PathBuf::from("/")
}
#[allow(dead_code)]
fn _unused_status() -> ServiceStatus {
    ServiceStatus::Unknown
}
