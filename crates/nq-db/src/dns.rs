//! `dns_observations` substrate + `dns_state` preflight evaluator
//! (V0, third bespoke claim kind).
//!
//! See `docs/working/gaps/DNS_WITNESS_FAMILY_GAP.md`. This module owns the
//! insert and latest-per-tuple load paths against the
//! `dns_observations` table (migration 047) and the bespoke evaluator
//! that maps those rows into a bounded `PreflightResult`. No probe, no
//! HTTP route, no registry — those remain later slices, each requiring
//! its own go-ahead.
//!
//! Wording discipline: support text for NODATA/NXDOMAIN says "resolver
//! returned" or "resolver reported", never "confirmed". The witness is
//! the resolver response from one vantage at one instant, not global
//! DNS truth. The closed `ResponseKind` taxonomy is preserved through
//! the evaluator — NXDOMAIN, NODATA, SERVFAIL, REFUSED, timeout, and
//! transport_error are six distinct verdicts and must not collapse into
//! a generic "DNS failed."

use crate::dns_state_witness_projection::project_dns_observation;
use crate::witness_projection_support::{make_projection_refusal_exclusion, packet_identity};
use crate::ReadDb;
use anyhow::Context;
use nq_core::preflight::{
    freshness_horizon_from, ClaimKind, PreflightCoverage, PreflightResult, PreflightSupport,
    PreflightTarget, ResponseKind, Verdict,
};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::str::FromStr;

/// Staleness threshold for the latest dns_observations row for a tuple,
/// in seconds. Default matches `ingest_state`'s 300s heuristic — 5× a
/// 60s probe interval, large enough to absorb a missed cycle, small
/// enough that two consecutive misses are testifiable as stale. Bespoke
/// for V0; per-tuple tuning is a later slice if it forces.
pub const DNS_STATE_STALE_THRESHOLD_SECONDS: i64 = 300;

/// One DNS observation row.
///
/// `observation_id` is `None` for an unwritten record; `insert_observation`
/// returns the assigned id without mutating the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsObservation {
    pub observation_id: Option<i64>,
    pub generation_id: i64,
    pub vantage_host: String,
    pub resolver: String,
    pub query_name: String,
    pub query_type: String,
    pub response_kind: ResponseKind,
    pub rcode: Option<i64>,
    pub answer_summary: Option<String>,
    pub min_ttl_seconds: Option<i64>,
    pub duration_ms: i64,
    pub observed_at: String,
    pub error_detail: Option<String>,
}

/// The four-field identity that selects "a probe at one vantage against
/// one resolver, asking one specific question." `latest_observation_for_tuple`
/// returns the most recent observation matching this key.
#[derive(Debug, Clone, Copy)]
pub struct DnsObservationTuple<'a> {
    pub vantage_host: &'a str,
    pub resolver: &'a str,
    pub query_name: &'a str,
    pub query_type: &'a str,
}

pub fn insert_observation(conn: &Connection, obs: &DnsObservation) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO dns_observations
            (generation_id, vantage_host, resolver, query_name, query_type,
             response_kind, rcode, answer_summary, min_ttl_seconds,
             duration_ms, observed_at, error_detail)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            obs.generation_id,
            obs.vantage_host,
            obs.resolver,
            obs.query_name,
            obs.query_type,
            obs.response_kind.as_str(),
            obs.rcode,
            obs.answer_summary,
            obs.min_ttl_seconds,
            obs.duration_ms,
            obs.observed_at,
            obs.error_detail,
        ],
    )
    .with_context(|| {
        format!(
            "insert dns_observation gen={} tuple=({},{},{},{})",
            obs.generation_id,
            obs.vantage_host,
            obs.resolver,
            obs.query_name,
            obs.query_type
        )
    })?;
    Ok(conn.last_insert_rowid())
}

/// Return the most recent observation row for the given tuple, or
/// `None` if no row exists. The evaluator (later slice) treats `None`
/// as `insufficient_coverage`; absence is **not** stored as a sentinel
/// row.
///
/// Ties on `observed_at` are broken by `observation_id` (largest wins),
/// matching the index's trailing column order in spirit while keeping
/// the result deterministic for callers that need a single row.
pub fn latest_observation_for_tuple(
    conn: &Connection,
    key: &DnsObservationTuple<'_>,
) -> anyhow::Result<Option<DnsObservation>> {
    conn.query_row(
        "SELECT observation_id, generation_id, vantage_host, resolver,
                query_name, query_type, response_kind, rcode, answer_summary,
                min_ttl_seconds, duration_ms, observed_at, error_detail
         FROM dns_observations
         WHERE vantage_host = ?1 AND resolver = ?2
           AND query_name = ?3 AND query_type = ?4
         ORDER BY observed_at DESC, observation_id DESC
         LIMIT 1",
        params![
            key.vantage_host,
            key.resolver,
            key.query_name,
            key.query_type
        ],
        row_to_observation,
    )
    .optional()
    .map_err(Into::into)
}

fn row_to_observation(r: &Row<'_>) -> rusqlite::Result<DnsObservation> {
    let kind_str: String = r.get(6)?;
    let response_kind = ResponseKind::from_str(&kind_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            6,
            rusqlite::types::Type::Text,
            Box::new(e),
        )
    })?;
    Ok(DnsObservation {
        observation_id: Some(r.get(0)?),
        generation_id: r.get(1)?,
        vantage_host: r.get(2)?,
        resolver: r.get(3)?,
        query_name: r.get(4)?,
        query_type: r.get(5)?,
        response_kind,
        rcode: r.get(7)?,
        answer_summary: r.get(8)?,
        min_ttl_seconds: r.get(9)?,
        duration_ms: r.get(10)?,
        observed_at: r.get(11)?,
        error_detail: r.get(12)?,
    })
}

// ---------------------------------------------------------------------------
// `dns_state` evaluator. Reads the latest observation for one
// (vantage_host, resolver, query_name, query_type) tuple and projects
// it into a bounded `PreflightResult`. Verdicts:
//
//   success / nodata / nxdomain / servfail / refused  → AdmissibleWithScope
//   timeout                                           → InsufficientCoverage
//   transport_error                                   → CannotTestify
//   no row                                            → InsufficientCoverage
//   any row older than DNS_STATE_STALE_THRESHOLD_SECONDS → StaleTestimony
//   validation_failure (reserved; V0 never emits)     → ContradictoryTestimony
//
// Constitutional `cannot_testify` (preloaded by skeleton) is always
// populated regardless of verdict. The closed taxonomy is preserved —
// the six error/negative kinds must not collapse into a generic
// "DNS failed."
// ---------------------------------------------------------------------------

/// Public entry point. Returns a `PreflightResult` for `dns_state`
/// against the latest dns_observations row matching `key`.
pub fn evaluate_dns_state_preflight(
    db: &ReadDb,
    key: &DnsObservationTuple<'_>,
) -> anyhow::Result<PreflightResult> {
    evaluate_dns_state_preflight_from_conn(db.conn(), key)
}

/// Variant that accepts a raw `Connection`. Used by tests and by the
/// HTTP route layer (later slice); the public API is the `ReadDb` form
/// above.
pub fn evaluate_dns_state_preflight_from_conn(
    conn: &Connection,
    key: &DnsObservationTuple<'_>,
) -> anyhow::Result<PreflightResult> {
    let generated_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| String::new());

    let target = PreflightTarget {
        host: key.vantage_host.to_string(),
        scope: "dns_query".to_string(),
        id: Some(format!(
            "resolver={};name={};type={}",
            key.resolver, key.query_name, key.query_type
        )),
    };
    let mut result = PreflightResult::skeleton(ClaimKind::DnsState, target, generated_at.clone());

    let Some(obs) = latest_observation_for_tuple(conn, key)? else {
        // No row exists for this tuple. The prober has not run for it
        // (or the row aged out via generation cascade). This is **not**
        // a synthetic substrate state — absence is read as
        // insufficient_coverage at the evaluator layer.
        result.verdict = Verdict::InsufficientCoverage;
        result.verdict_note = Some(format!(
            "No dns_observations row exists for (vantage={}, resolver={}, name={}, type={}); \
             the prober has not run for this tuple. Absence of observation is not affirmative \
             testimony of healthy resolution.",
            key.vantage_host, key.resolver, key.query_name, key.query_type
        ));
        result.coverage.push(PreflightCoverage {
            witness: "dns_resolver".to_string(),
            standing: "absent".to_string(),
            note: Some(format!("no observation row for resolver {}", key.resolver)),
        });
        result.compute_time_basis();
        return Ok(result);
    };

    // Freshness check first. A stale row's response_kind still informs
    // the support text — what the resolver said at that observation
    // time is real evidence — but the verdict is stale_testimony, not
    // the kind-specific verdict. Conflating the two would let a six-
    // hour-old success row pose as live resolution testimony.
    let now = time::OffsetDateTime::now_utc();
    let parsed = time::OffsetDateTime::parse(
        &obs.observed_at,
        &time::format_description::well_known::Rfc3339,
    )
    .ok();
    let age_seconds = parsed.map(|t| (now - t).whole_seconds());
    let stale = matches!(age_seconds, Some(age) if age > DNS_STATE_STALE_THRESHOLD_SECONDS);

    // Slice 2 cut-over: six of the eight ResponseKind paths produce
    // supports (the five admissible-with-scope answer kinds plus
    // ValidationFailure), as do all stale rows regardless of kind.
    // Project once before admitting; if the projector refuses, the row
    // exists but its custody cannot anchor admissible testimony —
    // degrade to InsufficientCoverage with a PreflightExclusion that
    // names the custody constraint. Timeout and TransportError do not
    // produce supports today (silence is not affirmative testimony;
    // unreachable vantage is a witness-standing refusal), so no
    // projection is attempted on those paths. See
    // docs/working/decisions/preflights/DNS_STATE_WITNESS_PACKET_CUTOVER.md.
    let produces_support = stale
        || matches!(
            obs.response_kind,
            ResponseKind::Success
                | ResponseKind::Nodata
                | ResponseKind::Nxdomain
                | ResponseKind::Servfail
                | ResponseKind::Refused
                | ResponseKind::ValidationFailure
        );
    let projected_packet = if produces_support {
        match project_dns_observation(&obs, &generated_at) {
            Ok(packet) => Some(packet),
            Err(refusal) => {
                result.excludes.push(make_projection_refusal_exclusion(
                    format!("dns_{}", obs.response_kind.as_str()),
                    format!(
                        "resolver={};name={};type={}",
                        obs.resolver, obs.query_name, obs.query_type
                    ),
                    &refusal,
                ));
                result.coverage.push(PreflightCoverage {
                    witness: "dns_resolver".to_string(),
                    standing: "observable".to_string(),
                    note: Some(
                        "latest dns_observations row refused projection — see excludes for the custody constraint"
                            .to_string(),
                    ),
                });
                result.verdict = Verdict::InsufficientCoverage;
                result.verdict_note = Some(
                    "Latest dns_observations row could not be projected into an admissible \
                     witness packet; dns_state evidence is in custody refusal."
                        .to_string(),
                );
                result.compute_time_basis();
                return Ok(result);
            }
        }
    } else {
        None
    };

    if stale {
        let mut support = make_support(&obs);
        support.witness_packet = projected_packet.as_ref().and_then(packet_identity);
        result.observed_at_min = Some(obs.observed_at.clone());
        result.observed_at_max = Some(obs.observed_at.clone());
        result.freshness_horizon = freshness_horizon_from(
            result.observed_at_max.as_deref(),
            DNS_STATE_STALE_THRESHOLD_SECONDS,
        );
        result.supports.push(support);
        result.coverage.push(PreflightCoverage {
            witness: "dns_resolver".to_string(),
            standing: "stale".to_string(),
            note: Some(format!(
                "most recent observation from resolver {} is older than {}s",
                obs.resolver, DNS_STATE_STALE_THRESHOLD_SECONDS
            )),
        });
        result.verdict = Verdict::StaleTestimony;
        result.verdict_note = Some(format!(
            "Latest dns_observations row at observed_at {} is {}s old (> {}s threshold); \
             dns_state testimony is stale.",
            obs.observed_at,
            age_seconds.unwrap_or_default(),
            DNS_STATE_STALE_THRESHOLD_SECONDS
        ));
        result.compute_time_basis();
        return Ok(result);
    }

    // Fresh row. Dispatch on response_kind.
    match obs.response_kind {
        ResponseKind::Success
        | ResponseKind::Nodata
        | ResponseKind::Nxdomain
        | ResponseKind::Servfail
        | ResponseKind::Refused => {
            let mut support = make_support(&obs);
            support.witness_packet = projected_packet.as_ref().and_then(packet_identity);
            result.observed_at_min = Some(obs.observed_at.clone());
            result.observed_at_max = Some(obs.observed_at.clone());
            result.freshness_horizon = freshness_horizon_from(
                result.observed_at_max.as_deref(),
                DNS_STATE_STALE_THRESHOLD_SECONDS,
            );
            result.supports.push(support);
            result.coverage.push(PreflightCoverage {
                witness: "dns_resolver".to_string(),
                standing: "observable".to_string(),
                note: Some(format!(
                    "resolver {} answered within budget for ({}, {})",
                    obs.resolver, obs.query_name, obs.query_type
                )),
            });
            result.verdict = Verdict::AdmissibleWithScope;
            result.verdict_note = Some(format!(
                "Resolver {} returned a {} response from vantage {}; admissible only at witness \
                 scope. Consequence claims remain refused (see cannot_testify).",
                obs.resolver,
                obs.response_kind.as_str(),
                obs.vantage_host
            ));
        }
        ResponseKind::Timeout => {
            // The resolver did not answer within budget. No row is
            // promoted into a support; there is no observed answer-
            // shape to admit. coverage records the silence.
            result.coverage.push(PreflightCoverage {
                witness: "dns_resolver".to_string(),
                standing: "silent".to_string(),
                note: Some(format!(
                    "resolver {} did not respond within budget at observed_at {}",
                    obs.resolver, obs.observed_at
                )),
            });
            result.verdict = Verdict::InsufficientCoverage;
            result.verdict_note = Some(format!(
                "Resolver {} did not answer (timeout) for ({}, {}) at observed_at {}; no \
                 answer-shape testimony to admit. Silence is not affirmative testimony.",
                obs.resolver, obs.query_name, obs.query_type, obs.observed_at
            ));
        }
        ResponseKind::TransportError => {
            // The vantage could not reach the resolver. The unknown is
            // the vantage's network stack, not the queried name. No
            // support; this is a witness-standing refusal, not a
            // negative answer.
            let detail = obs
                .error_detail
                .as_deref()
                .map(|e| format!(" — {e}"))
                .unwrap_or_default();
            result.coverage.push(PreflightCoverage {
                witness: "dns_resolver".to_string(),
                standing: "unreachable".to_string(),
                note: Some(format!(
                    "vantage {} could not reach resolver {} at observed_at {}{}",
                    obs.vantage_host, obs.resolver, obs.observed_at, detail
                )),
            });
            result.verdict = Verdict::CannotTestify;
            result.verdict_note = Some(format!(
                "Vantage {} could not reach resolver {} for ({}, {}) at observed_at {}{}; the \
                 unknown is the vantage's path to the resolver, not the queried name.",
                obs.vantage_host,
                obs.resolver,
                obs.query_name,
                obs.query_type,
                obs.observed_at,
                detail
            ));
        }
        ResponseKind::ValidationFailure => {
            // Reserved slot per the gap doc; V0 collectors never emit
            // this. If a future probe writes one, route to
            // contradictory_testimony: a DNSSEC validation failure is
            // testimony that the answer cannot honestly be trusted
            // *or* discarded — admitting either is laundering.
            let mut support = make_support(&obs);
            support.witness_packet = projected_packet.as_ref().and_then(packet_identity);
            result.observed_at_min = Some(obs.observed_at.clone());
            result.observed_at_max = Some(obs.observed_at.clone());
            result.freshness_horizon = freshness_horizon_from(
                result.observed_at_max.as_deref(),
                DNS_STATE_STALE_THRESHOLD_SECONDS,
            );
            result.supports.push(support);
            result.coverage.push(PreflightCoverage {
                witness: "dns_resolver".to_string(),
                standing: "observable".to_string(),
                note: Some(format!(
                    "resolver {} returned an answer that failed DNSSEC validation",
                    obs.resolver
                )),
            });
            result.verdict = Verdict::ContradictoryTestimony;
            result.verdict_note = Some(
                "Resolver returned an answer that failed DNSSEC validation; admitting either \
                 the answer or its absence as live truth is laundering. V0 collectors do not \
                 validate, so encountering this verdict means a later slice's row is being read."
                    .to_string(),
            );
        }
    }

    result.compute_time_basis();
    Ok(result)
}

/// Map a dns_observation row to the operator-facing weaker claim. The
/// claim text carries witness, subject, and observed_at — a consumer
/// that quotes only the `claim` field cannot launder the scope away.
///
/// Wording discipline: NODATA / NXDOMAIN say "resolver returned",
/// SERVFAIL says "resolver reported", REFUSED says "resolver refused".
/// No kind says "confirmed" — the witness is the resolver, not global
/// DNS truth.
fn make_support(obs: &DnsObservation) -> PreflightSupport {
    let claim = match obs.response_kind {
        ResponseKind::Success => format!(
            "Resolver {} returned an answer for ({}, {}) with summary {}, min_ttl {}, at observed_at {}",
            obs.resolver,
            obs.query_name,
            obs.query_type,
            obs.answer_summary.as_deref().unwrap_or("(none)"),
            obs.min_ttl_seconds
                .map(|t| format!("{t}s"))
                .unwrap_or_else(|| "unknown".to_string()),
            obs.observed_at,
        ),
        ResponseKind::Nodata => format!(
            "Resolver {} returned NODATA for ({}, {}) at observed_at {} — name exists per this \
             resolver; no records of type {}",
            obs.resolver, obs.query_name, obs.query_type, obs.observed_at, obs.query_type
        ),
        ResponseKind::Nxdomain => format!(
            "Resolver {} returned NXDOMAIN for ({}) at observed_at {} — cached denial, not \
             eternal nonexistence",
            obs.resolver, obs.query_name, obs.observed_at
        ),
        ResponseKind::Servfail => format!(
            "Resolver {} reported SERVFAIL for ({}, {}) at observed_at {} — testimony about the \
             resolver, not about {}",
            obs.resolver, obs.query_name, obs.query_type, obs.observed_at, obs.query_name
        ),
        ResponseKind::Refused => format!(
            "Resolver {} refused query for ({}, {}) at observed_at {} — testimony about resolver \
             policy, not about {}",
            obs.resolver, obs.query_name, obs.query_type, obs.observed_at, obs.query_name
        ),
        ResponseKind::ValidationFailure => format!(
            "Resolver {} returned an answer for ({}, {}) at observed_at {} that failed DNSSEC \
             validation; this row is reserved for a future validating probe and is not emitted \
             by V0",
            obs.resolver, obs.query_name, obs.query_type, obs.observed_at
        ),
        ResponseKind::Timeout | ResponseKind::TransportError => {
            // These kinds reach make_support only via the stale path,
            // where carrying the most-recent row preserves what we
            // observed at the (now-stale) time.
            format!(
                "Resolver {} produced no answer ({}) for ({}, {}) at observed_at {}",
                obs.resolver,
                obs.response_kind.as_str(),
                obs.query_name,
                obs.query_type,
                obs.observed_at,
            )
        }
    };
    PreflightSupport {
        claim,
        finding_kind: format!("dns_{}", obs.response_kind.as_str()),
        subject: format!(
            "resolver={};name={};type={}",
            obs.resolver, obs.query_name, obs.query_type
        ),
        observed_at: Some(obs.observed_at.clone()),
        freshness: None,
        admissibility_state: Some("observable".to_string()),
        // Caller stamps witness_packet from the projector output. Left
        // None here so a future use of make_support outside the
        // evaluator does not accidentally inherit a stale packet.
        witness_packet: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, open_rw};

    fn make_db() -> crate::WriteDb {
        let mut db = open_rw(std::path::Path::new(":memory:")).unwrap();
        migrate(&mut db).unwrap();
        db
    }

    fn ensure_generation(conn: &Connection, gen_id: i64) {
        conn.execute(
            "INSERT OR IGNORE INTO generations
                   (generation_id, started_at, completed_at, status,
                    sources_expected, sources_ok, sources_failed, duration_ms)
                 VALUES (?1, '2026-05-14T00:00:00Z', '2026-05-14T00:00:00Z',
                         'complete', 1, 1, 0, 0)",
            params![gen_id],
        )
        .unwrap();
    }

    fn obs(
        gen_id: i64,
        vantage: &str,
        resolver: &str,
        name: &str,
        qtype: &str,
        kind: ResponseKind,
        observed_at: &str,
    ) -> DnsObservation {
        DnsObservation {
            observation_id: None,
            generation_id: gen_id,
            vantage_host: vantage.into(),
            resolver: resolver.into(),
            query_name: name.into(),
            query_type: qtype.into(),
            response_kind: kind,
            rcode: None,
            answer_summary: None,
            min_ttl_seconds: None,
            duration_ms: 12,
            observed_at: observed_at.into(),
            error_detail: None,
        }
    }

    fn tuple<'a>(
        vantage: &'a str,
        resolver: &'a str,
        name: &'a str,
        qtype: &'a str,
    ) -> DnsObservationTuple<'a> {
        DnsObservationTuple {
            vantage_host: vantage,
            resolver,
            query_name: name,
            query_type: qtype,
        }
    }

    #[test]
    fn insert_returns_rowid_and_round_trips_through_latest_lookup() {
        let db = make_db();
        ensure_generation(&db.conn, 100);

        let mut row = obs(
            100,
            "sushi-k",
            "8.8.8.8",
            "nq.neutral.zone",
            "A",
            ResponseKind::Success,
            "2026-05-19T18:00:00Z",
        );
        row.rcode = Some(0);
        row.answer_summary = Some("23.92.30.41".into());
        row.min_ttl_seconds = Some(300);
        row.duration_ms = 42;

        let id = insert_observation(&db.conn, &row).unwrap();
        assert!(id > 0, "insert must yield positive rowid");

        let loaded = latest_observation_for_tuple(
            &db.conn,
            &tuple("sushi-k", "8.8.8.8", "nq.neutral.zone", "A"),
        )
        .unwrap()
        .expect("inserted row must be readable back");

        assert_eq!(loaded.observation_id, Some(id));
        assert_eq!(loaded.generation_id, 100);
        assert_eq!(loaded.vantage_host, "sushi-k");
        assert_eq!(loaded.resolver, "8.8.8.8");
        assert_eq!(loaded.query_name, "nq.neutral.zone");
        assert_eq!(loaded.query_type, "A");
        assert_eq!(loaded.response_kind, ResponseKind::Success);
        assert_eq!(loaded.rcode, Some(0));
        assert_eq!(loaded.answer_summary.as_deref(), Some("23.92.30.41"));
        assert_eq!(loaded.min_ttl_seconds, Some(300));
        assert_eq!(loaded.duration_ms, 42);
        assert_eq!(loaded.observed_at, "2026-05-19T18:00:00Z");
        assert_eq!(loaded.error_detail, None);
    }

    #[test]
    fn missing_tuple_returns_none_not_a_sentinel_row() {
        // Absence must not silently become a `no_witness` substrate row;
        // the evaluator owns the insufficient_coverage verdict instead.
        let db = make_db();
        let got = latest_observation_for_tuple(
            &db.conn,
            &tuple("sushi-k", "8.8.8.8", "never.probed", "A"),
        )
        .unwrap();
        assert!(got.is_none());
    }

    #[test]
    fn latest_wins_for_same_tuple() {
        let db = make_db();
        ensure_generation(&db.conn, 100);
        ensure_generation(&db.conn, 200);

        insert_observation(
            &db.conn,
            &obs(
                100,
                "sushi-k",
                "8.8.8.8",
                "nq.neutral.zone",
                "A",
                ResponseKind::Servfail,
                "2026-05-19T17:00:00Z",
            ),
        )
        .unwrap();
        let newer_id = insert_observation(
            &db.conn,
            &obs(
                200,
                "sushi-k",
                "8.8.8.8",
                "nq.neutral.zone",
                "A",
                ResponseKind::Success,
                "2026-05-19T18:00:00Z",
            ),
        )
        .unwrap();

        let loaded = latest_observation_for_tuple(
            &db.conn,
            &tuple("sushi-k", "8.8.8.8", "nq.neutral.zone", "A"),
        )
        .unwrap()
        .unwrap();
        assert_eq!(loaded.observation_id, Some(newer_id));
        assert_eq!(loaded.response_kind, ResponseKind::Success);
        assert_eq!(loaded.observed_at, "2026-05-19T18:00:00Z");
    }

    #[test]
    fn observation_id_breaks_observed_at_ties_deterministically() {
        // Same observed_at on two rows for the same tuple: the larger
        // observation_id wins. The eventual evaluator needs a single
        // deterministic row; ties cannot collapse into nondeterminism.
        let db = make_db();
        ensure_generation(&db.conn, 100);
        let id_first = insert_observation(
            &db.conn,
            &obs(
                100,
                "v",
                "r",
                "n",
                "A",
                ResponseKind::Refused,
                "2026-05-19T18:00:00Z",
            ),
        )
        .unwrap();
        let id_second = insert_observation(
            &db.conn,
            &obs(
                100,
                "v",
                "r",
                "n",
                "A",
                ResponseKind::Success,
                "2026-05-19T18:00:00Z",
            ),
        )
        .unwrap();
        assert!(id_second > id_first);

        let loaded =
            latest_observation_for_tuple(&db.conn, &tuple("v", "r", "n", "A"))
                .unwrap()
                .unwrap();
        assert_eq!(loaded.observation_id, Some(id_second));
        assert_eq!(loaded.response_kind, ResponseKind::Success);
    }

    #[test]
    fn tuple_lookup_narrows_to_exact_match() {
        // Distinct resolver, distinct query_name, distinct query_type
        // each get their own latest row. The evaluator must never read
        // a sibling tuple's testimony as if it were the asked tuple's.
        let db = make_db();
        ensure_generation(&db.conn, 100);

        // Same name, two resolvers — both fresh.
        insert_observation(
            &db.conn,
            &obs(
                100,
                "v",
                "8.8.8.8",
                "example.com",
                "A",
                ResponseKind::Success,
                "2026-05-19T18:00:00Z",
            ),
        )
        .unwrap();
        insert_observation(
            &db.conn,
            &obs(
                100,
                "v",
                "1.1.1.1",
                "example.com",
                "A",
                ResponseKind::Nxdomain,
                "2026-05-19T18:00:00Z",
            ),
        )
        .unwrap();
        // Same resolver+name, different qtype.
        insert_observation(
            &db.conn,
            &obs(
                100,
                "v",
                "8.8.8.8",
                "example.com",
                "AAAA",
                ResponseKind::Nodata,
                "2026-05-19T18:00:00Z",
            ),
        )
        .unwrap();
        // Different vantage entirely.
        insert_observation(
            &db.conn,
            &obs(
                100,
                "other-vantage",
                "8.8.8.8",
                "example.com",
                "A",
                ResponseKind::Timeout,
                "2026-05-19T18:00:00Z",
            ),
        )
        .unwrap();

        let google_a =
            latest_observation_for_tuple(&db.conn, &tuple("v", "8.8.8.8", "example.com", "A"))
                .unwrap()
                .unwrap();
        assert_eq!(google_a.response_kind, ResponseKind::Success);

        let cf_a =
            latest_observation_for_tuple(&db.conn, &tuple("v", "1.1.1.1", "example.com", "A"))
                .unwrap()
                .unwrap();
        assert_eq!(cf_a.response_kind, ResponseKind::Nxdomain);

        let google_aaaa = latest_observation_for_tuple(
            &db.conn,
            &tuple("v", "8.8.8.8", "example.com", "AAAA"),
        )
        .unwrap()
        .unwrap();
        assert_eq!(google_aaaa.response_kind, ResponseKind::Nodata);

        let other_vantage = latest_observation_for_tuple(
            &db.conn,
            &tuple("other-vantage", "8.8.8.8", "example.com", "A"),
        )
        .unwrap()
        .unwrap();
        assert_eq!(other_vantage.response_kind, ResponseKind::Timeout);
    }

    #[test]
    fn check_constraint_rejects_unknown_response_kind() {
        // Belt-and-braces: the typed enum prevents emitting an unknown
        // kind through `insert_observation`, but the SQLite CHECK
        // constraint still has to refuse any direct-SQL writer that
        // tries to slip a new substrate string in without ratification.
        let db = make_db();
        ensure_generation(&db.conn, 100);
        let result = db.conn.execute(
            "INSERT INTO dns_observations
                (generation_id, vantage_host, resolver, query_name, query_type,
                 response_kind, duration_ms, observed_at)
             VALUES (?1, 'v', 'r', 'n', 'A',
                     'dnssec_passed', 1, '2026-05-19T18:00:00Z')",
            params![100],
        );
        assert!(result.is_err(), "CHECK constraint must reject unknown response_kind");
    }

    #[test]
    fn delete_generation_cascades_to_observations() {
        // Retention deletes oldest generations; observations must follow
        // so we never read a row whose generation row is gone (which
        // would otherwise look like fresh testimony from a vanished
        // pulse).
        let db = make_db();
        ensure_generation(&db.conn, 100);
        let id = insert_observation(
            &db.conn,
            &obs(
                100,
                "v",
                "r",
                "n",
                "A",
                ResponseKind::Success,
                "2026-05-19T18:00:00Z",
            ),
        )
        .unwrap();
        assert!(id > 0);

        db.conn
            .execute("DELETE FROM generations WHERE generation_id = 100", [])
            .unwrap();

        let got = latest_observation_for_tuple(&db.conn, &tuple("v", "r", "n", "A")).unwrap();
        assert!(
            got.is_none(),
            "observation should cascade-delete with its generation"
        );
    }

    #[test]
    fn unknown_response_kind_in_db_surfaces_as_load_error() {
        // The CHECK constraint blocks new writes of unknown kinds, but a
        // hypothetical future migration that widens the enum without
        // updating ResponseKind would otherwise read as silent corruption.
        // Drop the CHECK constraint locally to simulate, then assert the
        // load path errors rather than silently coercing.
        let db = make_db();
        ensure_generation(&db.conn, 100);

        // SQLite doesn't allow DROP CHECK directly; rebuild without it.
        db.conn
            .execute_batch(
                "ALTER TABLE dns_observations RENAME TO dns_observations_old;
                 CREATE TABLE dns_observations (
                     observation_id    INTEGER PRIMARY KEY,
                     generation_id     INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
                     vantage_host      TEXT NOT NULL,
                     resolver          TEXT NOT NULL,
                     query_name        TEXT NOT NULL,
                     query_type        TEXT NOT NULL,
                     response_kind     TEXT NOT NULL,
                     rcode             INTEGER,
                     answer_summary    TEXT,
                     min_ttl_seconds   INTEGER,
                     duration_ms       INTEGER NOT NULL,
                     observed_at       TEXT NOT NULL,
                     error_detail      TEXT
                 );
                 INSERT INTO dns_observations
                     (generation_id, vantage_host, resolver, query_name, query_type,
                      response_kind, duration_ms, observed_at)
                 VALUES (100, 'v', 'r', 'n', 'A',
                         'dnssec_passed', 1, '2026-05-19T18:00:00Z');",
            )
            .unwrap();

        let result =
            latest_observation_for_tuple(&db.conn, &tuple("v", "r", "n", "A"));
        let err = result.unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("dnssec_passed"),
            "load error must fingerprint the bad value: {msg}"
        );
    }

    // -----------------------------------------------------------------------
    // `dns_state` evaluator tests.
    // -----------------------------------------------------------------------
    //
    // Phrases that must NEVER appear in any support claim or verdict_note,
    // by claim-kind constitution (see dns_state_cannot_testify and the
    // wording discipline in the gap doc).
    const FORBIDDEN_PHRASES: &[&str] = &[
        "endpoint reachable",
        "endpoint is reachable",
        "service healthy",
        "service is healthy",
        "service alive",
        "globally resolves",
        "global dns",
        "registrar",
        "account status",
        "dnssec validated",
        "dnssec passed",
        "will recover",
        "recovery imminent",
        "name resolves to",
        "ptr",
        // The user-named wording boundary: NXDOMAIN/NODATA must not be
        // narrated as `confirmed`.
        "confirmed",
    ];

    fn assert_supports_are_bounded(r: &PreflightResult) {
        for support in &r.supports {
            let lower = support.claim.to_ascii_lowercase();
            for forbidden in FORBIDDEN_PHRASES {
                assert!(
                    !lower.contains(forbidden),
                    "support claim laundered forbidden vocabulary ({forbidden:?}): {:?}",
                    support.claim
                );
            }
        }
        if let Some(note) = &r.verdict_note {
            let lower = note.to_ascii_lowercase();
            for forbidden in FORBIDDEN_PHRASES {
                assert!(
                    !lower.contains(forbidden),
                    "verdict_note laundered forbidden vocabulary ({forbidden:?}): {note:?}"
                );
            }
        }
    }

    fn rfc3339_at_offset(offset_seconds: i64) -> String {
        let t = time::OffsetDateTime::now_utc() + time::Duration::seconds(offset_seconds);
        t.format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    }

    fn seed_observation(db: &crate::WriteDb, gen_id: i64, kind: ResponseKind) -> DnsObservation {
        ensure_generation(&db.conn, gen_id);
        let observed_at = rfc3339_at_offset(-30); // fresh
        let mut row = obs(
            gen_id,
            "sushi-k",
            "8.8.8.8",
            "nq.neutral.zone",
            "A",
            kind,
            &observed_at,
        );
        if matches!(kind, ResponseKind::Success) {
            row.rcode = Some(0);
            row.answer_summary = Some("23.92.30.41".into());
            row.min_ttl_seconds = Some(300);
        }
        if matches!(kind, ResponseKind::TransportError) {
            row.error_detail = Some("connection refused".into());
        }
        insert_observation(&db.conn, &row).unwrap();
        row
    }

    fn default_tuple() -> DnsObservationTuple<'static> {
        DnsObservationTuple {
            vantage_host: "sushi-k",
            resolver: "8.8.8.8",
            query_name: "nq.neutral.zone",
            query_type: "A",
        }
    }

    #[test]
    fn evaluator_schema_and_target_shape_match_the_gap_doc() {
        let db = make_db();
        let r =
            evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple()).unwrap();
        assert_eq!(r.schema, nq_core::preflight::PREFLIGHT_DNS_STATE_SCHEMA);
        assert_eq!(r.contract_version, nq_core::preflight::PREFLIGHT_CONTRACT_VERSION);
        assert_eq!(r.target.host, "sushi-k");
        assert_eq!(r.target.scope, "dns_query");
        assert_eq!(
            r.target.id.as_deref(),
            Some("resolver=8.8.8.8;name=nq.neutral.zone;type=A")
        );
    }

    #[test]
    fn dns_state_supports_carry_projected_packet_identity_post_cutover() {
        // After the dns_state cut-over, every admitted support carries
        // the projected witness packet's wire identity. Replaces the
        // pre-cut-over pin that previously asserted the opposite.
        // dns_state was the third Track A evaluator to cut over; with
        // this slice landed there are no remaining pre-cut-over Track A
        // evaluators to pin.
        use nq_core::witness::CUSTODY_BASIS_LEGACY_PROJECTION;

        let db = make_db();
        let _row = seed_observation(&db, 100, ResponseKind::Success);

        let r =
            evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple()).unwrap();
        assert_eq!(r.supports.len(), 1, "fresh success → one support");
        let wp = r.supports[0]
            .witness_packet
            .as_ref()
            .expect("post-cut-over dns_state support must carry its packet identity");
        assert_eq!(wp.witness_type, "dns_resolver_legacy_projection");
        assert!(wp.digest.starts_with("sha256:"));
        assert_eq!(wp.digest.len(), "sha256:".len() + 64);
        assert_eq!(
            wp.custody_basis.as_deref(),
            Some(CUSTODY_BASIS_LEGACY_PROJECTION)
        );
    }

    #[test]
    fn dns_state_receipt_anchors_witness_refs_to_projected_packets_post_cutover() {
        // The cross-evaluator gate in From<PreflightResult> flips to the
        // packet-anchored path as soon as any support carries
        // witness_packet. After the dns_state cut-over, every receipt
        // from a support-bearing observation row carries digest-stamped,
        // basis-declared WitnessRefs.
        use nq_core::receipt::Receipt;
        use nq_core::witness::CUSTODY_BASIS_LEGACY_PROJECTION;

        let db = make_db();
        let _row = seed_observation(&db, 100, ResponseKind::Nxdomain);

        let pr =
            evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple()).unwrap();
        let receipt: Receipt = pr.into();
        assert!(!receipt.witnesses.is_empty());
        for w in &receipt.witnesses {
            assert!(
                w.witness_type.ends_with("_legacy_projection"),
                "WitnessRef must reflect projected-packet provenance: {}",
                w.witness_type
            );
            assert!(
                w.digest.as_deref().unwrap_or("").starts_with("sha256:"),
                "WitnessRef must carry a digest: {:?}",
                w.digest
            );
            assert_eq!(
                w.custody_basis.as_deref(),
                Some(CUSTODY_BASIS_LEGACY_PROJECTION),
                "WitnessRef must declare legacy_projection basis: {:?}",
                w.custody_basis
            );
        }
    }

    #[test]
    fn dns_state_projection_refusal_forces_insufficient_coverage_and_excludes_the_row() {
        // The latest dns_observations row is load-bearing for a tuple
        // evaluation — if it cannot be projected (empty / unparseable
        // observed_at), no admissible substrate remains. Verdict must
        // be InsufficientCoverage; the refusal must surface as a
        // PreflightExclusion; supports must be empty; coverage stays
        // observable (the row is present) with a note that points at
        // the exclude.
        let db = make_db();
        ensure_generation(&db.conn, 100);
        // Slip a non-RFC3339 observed_at past the NOT NULL constraint.
        db.conn
            .execute(
                "INSERT INTO dns_observations
                    (generation_id, vantage_host, resolver, query_name, query_type,
                     response_kind, duration_ms, observed_at)
                 VALUES (100, 'sushi-k', '8.8.8.8', 'nq.neutral.zone', 'A',
                         'success', 1, 'not-a-timestamp')",
                [],
            )
            .unwrap();

        let r =
            evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple()).unwrap();
        assert_eq!(r.verdict, Verdict::InsufficientCoverage);
        assert!(r.supports.is_empty(), "refused row → no supports");

        let refusal = r
            .excludes
            .iter()
            .find(|e| e.reason.contains("projection refused"))
            .expect("projection refusal must appear as a PreflightExclusion");
        assert_eq!(refusal.finding_kind, "dns_success");
        assert_eq!(
            refusal.subject,
            "resolver=8.8.8.8;name=nq.neutral.zone;type=A"
        );
        assert!(
            refusal.reason.contains("RFC3339"),
            "exclude must name the substrate-time refusal reason: {:?}",
            refusal.reason
        );

        let cov = r
            .coverage
            .iter()
            .find(|c| c.witness == "dns_resolver")
            .expect("dns_resolver coverage entry");
        assert_eq!(
            cov.standing, "observable",
            "the row is present; only its custody failed"
        );
        assert!(cov
            .note
            .as_deref()
            .unwrap_or("")
            .contains("refused projection"));

        // No witness_packet to stamp on a non-existent support.
        assert!(r.observed_at_min.is_none());
        assert!(r.observed_at_max.is_none());
        assert!(r.freshness_horizon.is_none());
    }

    #[test]
    fn evaluator_no_row_is_insufficient_coverage_with_absent_coverage() {
        let db = make_db();
        let r =
            evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple()).unwrap();
        assert_eq!(r.verdict, Verdict::InsufficientCoverage);
        assert!(r.supports.is_empty(), "no row → no supports");
        assert!(r.observed_at_min.is_none());
        assert!(r.observed_at_max.is_none());
        let cov = r
            .coverage
            .iter()
            .find(|c| c.witness == "dns_resolver")
            .expect("dns_resolver coverage entry");
        assert_eq!(cov.standing, "absent");
        // Constitutional refusal surface is always populated.
        assert!(!r.cannot_testify.is_empty());
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("Endpoint reachability")));
        assert_supports_are_bounded(&r);
    }

    #[test]
    fn evaluator_success_is_admissible_with_scope() {
        let db = make_db();
        let row = seed_observation(&db, 100, ResponseKind::Success);

        let r =
            evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple()).unwrap();
        assert_eq!(r.verdict, Verdict::AdmissibleWithScope);
        assert_eq!(r.supports.len(), 1);
        let s = &r.supports[0];
        assert_eq!(s.finding_kind, "dns_success");
        assert_eq!(s.observed_at.as_deref(), Some(row.observed_at.as_str()));
        assert!(s.claim.contains("Resolver 8.8.8.8 returned an answer"));
        assert!(s.claim.contains("min_ttl 300s"));
        assert!(s.claim.contains("23.92.30.41"));
        // Observation window mirrors the support row.
        assert_eq!(r.observed_at_min, Some(row.observed_at.clone()));
        assert_eq!(r.observed_at_max, Some(row.observed_at));
        assert_supports_are_bounded(&r);
    }

    #[test]
    fn evaluator_nodata_is_admissible_with_scope_and_uses_returned_not_confirmed() {
        let db = make_db();
        seed_observation(&db, 100, ResponseKind::Nodata);

        let r =
            evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple()).unwrap();
        assert_eq!(r.verdict, Verdict::AdmissibleWithScope);
        let claim = &r.supports[0].claim;
        assert!(claim.contains("returned NODATA"), "wording: {claim}");
        assert!(
            !claim.to_ascii_lowercase().contains("confirmed"),
            "NODATA must not be narrated as `confirmed`: {claim}"
        );
        assert_supports_are_bounded(&r);
    }

    #[test]
    fn evaluator_nxdomain_is_admissible_with_scope_and_names_cached_denial() {
        let db = make_db();
        seed_observation(&db, 100, ResponseKind::Nxdomain);

        let r =
            evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple()).unwrap();
        assert_eq!(r.verdict, Verdict::AdmissibleWithScope);
        let claim = &r.supports[0].claim;
        assert!(claim.contains("returned NXDOMAIN"), "wording: {claim}");
        assert!(
            claim.contains("cached denial"),
            "must name NXDOMAIN as cached denial, not eternal nonexistence: {claim}"
        );
        assert!(
            !claim.to_ascii_lowercase().contains("confirmed"),
            "NXDOMAIN must not be narrated as `confirmed`: {claim}"
        );
        assert_supports_are_bounded(&r);
    }

    #[test]
    fn evaluator_servfail_is_admissible_with_scope_about_resolver_not_name() {
        let db = make_db();
        seed_observation(&db, 100, ResponseKind::Servfail);

        let r =
            evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple()).unwrap();
        assert_eq!(r.verdict, Verdict::AdmissibleWithScope);
        let claim = &r.supports[0].claim;
        assert!(claim.contains("reported SERVFAIL"), "wording: {claim}");
        assert!(
            claim.contains("about the resolver, not about"),
            "must scope SERVFAIL to the resolver, not the queried name: {claim}"
        );
        assert_supports_are_bounded(&r);
    }

    #[test]
    fn evaluator_refused_is_admissible_with_scope_about_resolver_policy_not_name() {
        let db = make_db();
        seed_observation(&db, 100, ResponseKind::Refused);

        let r =
            evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple()).unwrap();
        assert_eq!(r.verdict, Verdict::AdmissibleWithScope);
        let claim = &r.supports[0].claim;
        assert!(claim.contains("refused query"), "wording: {claim}");
        assert!(
            claim.contains("resolver policy"),
            "REFUSED must testify about resolver policy, not the queried name: {claim}"
        );
        assert_supports_are_bounded(&r);
    }

    #[test]
    fn evaluator_timeout_is_insufficient_coverage_no_support() {
        let db = make_db();
        seed_observation(&db, 100, ResponseKind::Timeout);

        let r =
            evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple()).unwrap();
        assert_eq!(r.verdict, Verdict::InsufficientCoverage);
        assert!(
            r.supports.is_empty(),
            "timeout fresh row has no admitted support; silence is not affirmative testimony"
        );
        assert!(r.observed_at_min.is_none());
        assert!(r.observed_at_max.is_none());
        let cov = r
            .coverage
            .iter()
            .find(|c| c.witness == "dns_resolver")
            .expect("dns_resolver coverage entry");
        assert_eq!(cov.standing, "silent");
        assert_supports_are_bounded(&r);
    }

    #[test]
    fn evaluator_transport_error_is_cannot_testify_no_support() {
        let db = make_db();
        seed_observation(&db, 100, ResponseKind::TransportError);

        let r =
            evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple()).unwrap();
        assert_eq!(r.verdict, Verdict::CannotTestify);
        assert!(
            r.supports.is_empty(),
            "transport_error fresh row has no admitted support; the unknown is the vantage path"
        );
        assert!(r.observed_at_min.is_none());
        assert!(r.observed_at_max.is_none());
        let cov = r
            .coverage
            .iter()
            .find(|c| c.witness == "dns_resolver")
            .expect("dns_resolver coverage entry");
        assert_eq!(cov.standing, "unreachable");
        assert!(
            cov.note
                .as_deref()
                .unwrap_or("")
                .contains("connection refused"),
            "coverage note must surface the underlying error detail"
        );
        // verdict_note must name the vantage path as the unknown, not
        // the queried name — preserves the witness-standing refusal.
        assert!(
            r.verdict_note
                .as_deref()
                .unwrap_or("")
                .contains("path to the resolver"),
            "verdict_note must scope the unknown to the vantage path"
        );
        assert_supports_are_bounded(&r);
    }

    #[test]
    fn evaluator_validation_failure_reserved_routes_to_contradictory_testimony() {
        // V0 collectors do not emit `validation_failure`. The slot is
        // reserved so a later DNSSEC-validating probe is not a wire-
        // breaking change. If a row of this kind appears, the
        // evaluator must route to contradictory_testimony per the gap
        // doc, not silently coerce.
        let db = make_db();
        seed_observation(&db, 100, ResponseKind::ValidationFailure);

        let r =
            evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple()).unwrap();
        assert_eq!(r.verdict, Verdict::ContradictoryTestimony);
        assert_eq!(r.supports.len(), 1);
        let s = &r.supports[0];
        assert_eq!(s.finding_kind, "dns_validation_failure");
        assert!(
            r.verdict_note
                .as_deref()
                .unwrap_or("")
                .contains("laundering"),
            "verdict_note must name the laundering risk"
        );
        // cannot_testify still pins DNSSEC as out-of-scope for V0.
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("DNSSEC validation outcome")));
        assert_supports_are_bounded(&r);
    }

    #[test]
    fn evaluator_stale_row_yields_stale_testimony_with_age_in_note() {
        // A success row from far in the past must surface as
        // stale_testimony, not as a fresh AdmissibleWithScope. The
        // support is still carried so the operator sees what was
        // observed — but the verdict says it's stale.
        let db = make_db();
        ensure_generation(&db.conn, 100);
        let stale_at = "2020-01-01T00:00:00Z"; // far older than 300s
        let mut row = obs(
            100,
            "sushi-k",
            "8.8.8.8",
            "nq.neutral.zone",
            "A",
            ResponseKind::Success,
            stale_at,
        );
        row.answer_summary = Some("198.51.100.7".into());
        row.min_ttl_seconds = Some(60);
        insert_observation(&db.conn, &row).unwrap();

        let r =
            evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple()).unwrap();
        assert_eq!(r.verdict, Verdict::StaleTestimony);
        assert_eq!(r.supports.len(), 1, "stale row is carried as support");
        assert_eq!(r.observed_at_min.as_deref(), Some(stale_at));
        assert_eq!(r.observed_at_max.as_deref(), Some(stale_at));
        let cov = r
            .coverage
            .iter()
            .find(|c| c.witness == "dns_resolver")
            .expect("dns_resolver coverage entry");
        assert_eq!(cov.standing, "stale");
        let note = r.verdict_note.as_deref().unwrap_or("");
        assert!(
            note.contains("stale") && note.contains("threshold"),
            "verdict_note must name staleness and threshold: {note}"
        );
        assert_supports_are_bounded(&r);
    }

    // -----------------------------------------------------------------
    // Slice 1c — freshness_horizon on the dns_state evaluator path.
    // -----------------------------------------------------------------

    #[test]
    fn evaluator_emits_freshness_horizon_on_fresh_success() {
        let db = make_db();
        ensure_generation(&db.conn, 100);
        let observed = rfc3339_at_offset(-30); // 30s ago — well within 300s threshold
        let mut row = obs(
            100,
            "sushi-k",
            "8.8.8.8",
            "nq.neutral.zone",
            "A",
            ResponseKind::Success,
            &observed,
        );
        row.answer_summary = Some("198.51.100.7".into());
        row.min_ttl_seconds = Some(60);
        insert_observation(&db.conn, &row).unwrap();

        let r = evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple()).unwrap();
        assert!(matches!(
            r.verdict,
            Verdict::AdmissibleWithScope | Verdict::Admissible
        ));
        let horizon = r
            .freshness_horizon
            .as_deref()
            .expect("fresh dns_state result emits freshness_horizon");
        // Horizon must be strictly after observed_at_max.
        let obs_at = r.observed_at_max.as_deref().unwrap();
        assert!(
            horizon > obs_at,
            "horizon ({horizon}) must be after observed_at_max ({obs_at})"
        );
    }

    #[test]
    fn evaluator_emits_freshness_horizon_even_when_verdict_is_stale() {
        // The horizon is descriptive of when the testimony falls outside
        // policy. A stale verdict means the deadline already passed —
        // horizon is still meaningful and still emitted.
        let db = make_db();
        ensure_generation(&db.conn, 100);
        let stale_at = "2020-01-01T00:00:00Z"; // far past
        let mut row = obs(
            100,
            "sushi-k",
            "8.8.8.8",
            "nq.neutral.zone",
            "A",
            ResponseKind::Success,
            stale_at,
        );
        row.answer_summary = Some("198.51.100.7".into());
        row.min_ttl_seconds = Some(60);
        insert_observation(&db.conn, &row).unwrap();

        let r = evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple()).unwrap();
        assert_eq!(r.verdict, Verdict::StaleTestimony);
        // stale_at + 300s = 2020-01-01T00:05:00.
        assert_eq!(
            r.freshness_horizon.as_deref(),
            Some("2020-01-01T00:05:00Z"),
            "horizon is emitted alongside StaleTestimony verdict (deadline already passed)"
        );
    }

    #[test]
    fn evaluator_no_row_leaves_freshness_horizon_absent() {
        // insufficient_coverage: no observation row → no observed_at_max
        // → no horizon. Guard against anchoring to generated_at as a
        // fallback.
        let db = make_db();
        ensure_generation(&db.conn, 100);
        let r = evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple()).unwrap();
        assert_eq!(r.verdict, Verdict::InsufficientCoverage);
        assert!(r.observed_at_max.is_none());
        assert!(r.freshness_horizon.is_none());
    }

    #[test]
    fn evaluator_cannot_testify_is_populated_across_all_verdicts() {
        // Constitutional refusals must remain regardless of verdict.
        // Spot-check across the full taxonomy: no row, success, every
        // negative kind, both error kinds, validation_failure, and the
        // stale path. Each must carry the same constitutional surface.
        let cases: &[(&str, fn(&crate::WriteDb, i64))] = &[
            ("no_row", |_db, _id| {}),
            ("success", |db, id| {
                seed_observation(db, id, ResponseKind::Success);
            }),
            ("nodata", |db, id| {
                seed_observation(db, id, ResponseKind::Nodata);
            }),
            ("nxdomain", |db, id| {
                seed_observation(db, id, ResponseKind::Nxdomain);
            }),
            ("servfail", |db, id| {
                seed_observation(db, id, ResponseKind::Servfail);
            }),
            ("refused", |db, id| {
                seed_observation(db, id, ResponseKind::Refused);
            }),
            ("timeout", |db, id| {
                seed_observation(db, id, ResponseKind::Timeout);
            }),
            ("transport_error", |db, id| {
                seed_observation(db, id, ResponseKind::TransportError);
            }),
            ("validation_failure", |db, id| {
                seed_observation(db, id, ResponseKind::ValidationFailure);
            }),
            ("stale", |db, id| {
                ensure_generation(&db.conn, id);
                let row = obs(
                    id,
                    "sushi-k",
                    "8.8.8.8",
                    "nq.neutral.zone",
                    "A",
                    ResponseKind::Success,
                    "2020-01-01T00:00:00Z",
                );
                insert_observation(&db.conn, &row).unwrap();
            }),
        ];

        for (label, seed) in cases {
            let db = make_db();
            seed(&db, 100);
            let r = evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple())
                .unwrap_or_else(|e| panic!("{label}: {e:#}"));
            assert!(
                r.cannot_testify
                    .iter()
                    .any(|s| s.contains("Endpoint reachability")),
                "{label}: endpoint reachability refusal must be present"
            );
            assert!(
                r.cannot_testify
                    .iter()
                    .any(|s| s.contains("Global DNS truth")),
                "{label}: global DNS truth refusal must be present"
            );
            assert!(
                r.cannot_testify
                    .iter()
                    .any(|s| s.contains("Registrar / account")),
                "{label}: registrar/account refusal must be present"
            );
            assert!(
                r.cannot_testify
                    .iter()
                    .any(|s| s.contains("DNSSEC validation outcome")),
                "{label}: DNSSEC refusal must be present"
            );
            assert!(
                r.cannot_testify
                    .iter()
                    .any(|s| s.starts_with("Whether to repoint")),
                "{label}: consequence-claim refusal must be present"
            );
            assert_supports_are_bounded(&r);
        }
    }

    #[test]
    fn evaluator_reads_only_the_asked_tuple_not_a_sibling() {
        // The evaluator must never mistake testimony from another
        // tuple for testimony about the asked tuple — that would
        // launder a sibling probe's verdict into this one.
        let db = make_db();
        ensure_generation(&db.conn, 100);

        // Asked tuple has no observation; sibling tuple (different
        // resolver) has a fresh Nxdomain.
        let sibling = obs(
            100,
            "sushi-k",
            "1.1.1.1",
            "nq.neutral.zone",
            "A",
            ResponseKind::Nxdomain,
            &rfc3339_at_offset(-30),
        );
        insert_observation(&db.conn, &sibling).unwrap();

        let r =
            evaluate_dns_state_preflight_from_conn(&db.conn, &default_tuple()).unwrap();
        assert_eq!(
            r.verdict,
            Verdict::InsufficientCoverage,
            "asked tuple has no row; sibling tuple must not leak in"
        );
        assert!(r.supports.is_empty());
    }
}
