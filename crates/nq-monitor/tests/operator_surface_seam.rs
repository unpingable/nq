//! Operator-surface seam receipts.
//!
//! Two guarantees for the seam introduced when the 9 inline
//! `evaluate_*_preflight` call sites were lifted out of
//! `http/routes.rs` (see
//! `docs/working/decisions/OPERATOR_SURFACE_SPLIT_TRIPWIRE.md`):
//!
//! 1. **Boundary (the poor-man type-wall).** `http/routes.rs` must call
//!    the `operator_surface::preflight` facade, never an evaluator
//!    directly. Until the verdict type literally cannot coerce to an
//!    operational status (`MONITORING_PROJECTION_SEAM_CANDIDATE.md`),
//!    this source-level check holds the line and keeps the gate at 0.
//! 2. **Injected clock + determinism.** The facade threads one explicit
//!    `now` to the evaluator's `_at` form, so the surfaced result is a
//!    pure function of (DB facts, injected now) — no ambient `now_utc()`
//!    drift at the glass. Evaluator-level verdict-vs-clock correctness
//!    (stale thresholds etc.) is covered by the `nq-db` `_at` tests; this
//!    proves the *facade* actually carries the injected clock through.

use nq_db::{migrate, open_ro, open_rw};
use nq_monitor::operator_surface::preflight;
use tempfile::TempDir;
use time::OffsetDateTime;

/// The boundary: zero `evaluate_*_preflight` call sites in the HTTP
/// surface. Comments are stripped so the doctrine note in the import
/// block ("never `evaluate_*_preflight` directly") does not self-trip;
/// the discriminator is the call-shape substring `_preflight(`, which the
/// facade functions (`preflight::disk_state(` etc.) do not contain.
#[test]
fn http_routes_has_no_direct_preflight_call_sites() {
    const ROUTES: &str = include_str!("../src/http/routes.rs");
    let offenders: Vec<(usize, &str)> = ROUTES
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let code = match line.find("//") {
                Some(idx) => &line[..idx],
                None => line,
            };
            code.contains("_preflight(").then_some((i + 1, line.trim()))
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "http/routes.rs must route preflight through operator_surface::preflight, \
         never call an evaluate_*_preflight evaluator directly. Offending lines: {offenders:?}"
    );
}

/// Same DB facts + same injected `now` => byte-identical surfaced result;
/// a different `now` changes it. Proves the clock is injected, not ambient.
#[test]
fn facade_injects_clock_and_is_deterministic() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("seam.db");
    let mut w = open_rw(&db_path).unwrap();
    migrate(&mut w).unwrap();
    drop(w);
    let db = open_ro(&db_path).unwrap();

    // Two clocks a decade apart. ingest_state on an empty DB returns
    // InsufficientCoverage either way; the variable under test is whether
    // the injected `now` reaches the output, not the verdict.
    let now_2020 = OffsetDateTime::from_unix_timestamp(1_577_836_800).unwrap(); // 2020-01-01Z
    let now_2030 = OffsetDateTime::from_unix_timestamp(1_893_456_000).unwrap(); // 2030-01-01Z

    let a1 = serde_json::to_value(preflight::ingest_state(&db, now_2020).unwrap()).unwrap();
    let a2 = serde_json::to_value(preflight::ingest_state(&db, now_2020).unwrap()).unwrap();
    assert_eq!(
        a1, a2,
        "same DB facts + same injected now must be byte-identical — no ambient now_utc() at the glass"
    );

    let b = serde_json::to_value(preflight::ingest_state(&db, now_2030).unwrap()).unwrap();
    assert_ne!(
        a1, b,
        "a different injected now must change the surfaced result — the clock is injected, not ambient"
    );
    assert!(
        serde_json::to_string(&a1).unwrap().contains("2020"),
        "generated_at must reflect the injected 2020 clock, not wall time"
    );
    assert!(
        serde_json::to_string(&b).unwrap().contains("2030"),
        "generated_at must reflect the injected 2030 clock, not wall time"
    );

    // The confession marker is present and honest: nothing here is sealed.
    assert_eq!(a1["evaluation_basis"]["kind"], "request_time_unsealed");
    assert_eq!(a1["evaluation_basis"]["clock"], "wall_utc");
    assert_eq!(a1["evaluation_basis"]["sealed"], serde_json::Value::Bool(false));
}
