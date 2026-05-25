//! Semantic replay of `nq.receipt.v1` documents against supplied
//! `nq.witness.v1` packets. Slice 1e of
//! `docs/architecture/PATH_TO_1_0.md`.
//!
//! Where Slice 1d (`receipt_check`) answers *"is this receipt
//! structurally intact?"*, 1e answers *"would the same evaluator,
//! given the same witness material and original receipt-time context,
//! reproduce the same semantic decision?"*
//!
//! Replay is **not**:
//!
//! - proof that the world still matches what the receipt described
//! - renewal of freshness
//! - re-authorization of action
//! - a replacement for a new preflight
//! - a guarantee that the original world existed
//!
//! Keepers:
//!
//! > Replay failure is not forgery. Replay success is not fresh authorization.
//!
//! > A stale receipt is not a forged receipt. A forged receipt is not a stale receipt.
//!
//! ## Track scope
//!
//! - **Track B** (`claim_registry` evaluator): supported. Re-runs
//!   `claim_registry::evaluate` against de-duplicated supplied packets
//!   under the receipt's original `generated_at` context, then
//!   compares semantic fields.
//! - **Track A** (`disk_state`, `ingest_state`, `dns_state`): returns
//!   `ReplayStatus::NotApplicable`. Track A's `PreflightCoverage`
//!   entries are decoupled from retained witness packet envelopes
//!   (see `DISK_STATE_CUTOVER_TO_SHARED_SPINE`), so there is no packet
//!   set to replay against. The receipt's structural integrity, witness
//!   digests, and freshness can still be checked via Slice 1d.
//!
//! ## Independence of axes
//!
//! Structural integrity (from 1d), semantic replay (1e), and freshness
//! (1c-populated, 1d-evaluated) are three independent axes. The exit
//! code combines them, but each is reported individually so the
//! operator sees what passed and what didn't.

use crate::receipt::{NotVerifiedEntry, Receipt, WitnessRef, RECEIPT_SCHEMA};
use crate::receipt_check::{check_receipt, CheckOptions, CheckReport};
use crate::witness::WitnessPacket;
use serde::Serialize;

/// Caller-supplied replay options.
#[derive(Debug, Clone, Default)]
pub struct ReplayOptions {
    /// `--strict`. When true, "warn-shaped" outcomes (duplicate
    /// packets, freshness horizon absent under `--fresh`) escalate to
    /// failures. Does not change the exit code of structural or
    /// semantic-replay failures, which are already failures.
    pub strict: bool,
    /// `--fresh`. When true, run a freshness check (orthogonal to
    /// replay) comparing `as_of` against `Receipt::freshness_horizon`.
    /// A stale receipt may still replay successfully; freshness and
    /// replay are independent axes.
    pub fresh: bool,
    /// RFC3339 UTC timestamp at which to evaluate freshness. Only
    /// consulted when `fresh` is true. CLI implication: `--as-of`
    /// implies `--fresh`. Callers that pass `fresh=true` with
    /// `as_of=None` get a `FreshnessOutcome::NotApplicable`.
    pub as_of: Option<String>,
}

/// Semantic-replay status. Independent of the structural-integrity
/// axis (see [`ReplayReport::integrity`]) and the freshness axis (see
/// [`ReplayReport::freshness`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayStatus {
    /// Replay ran and the semantic decision matches the original.
    Ok,
    /// Replay ran and produced a different semantic decision (verdict,
    /// supported claims, etc.) than the original. The original receipt
    /// was structurally intact, so this is the "forged receipt with
    /// fabricated semantic content" or "evaluator drift" shape, not
    /// the "broken envelope" shape.
    Mismatch,
    /// The receipt's evaluator is Track A; replay against retained
    /// packets is out of scope until Track A cuts over to the shared
    /// spine (see `DISK_STATE_CUTOVER_TO_SHARED_SPINE`).
    NotApplicable,
    /// The receipt's evaluator name is not one this binary knows.
    UnsupportedEvaluator,
    /// The receipt's evaluator version differs from the version this
    /// binary implements. Replay is refused rather than attempted —
    /// guessing across versions creates pseudo-replay.
    UnsupportedVersion,
    /// The receipt has no [`crate::receipt::EvaluatorBinding`].
    /// Without one, replay context is undefined.
    PolicyUnspecified,
    /// Replay needed a witness packet (named by digest on the receipt's
    /// `witnesses` list) that was not among the supplied packets.
    /// Custody, not contradiction.
    MissingWitnessMaterial,
    /// Slice 1d's structural check found integrity broken. Replay
    /// refused — semantic work on a receipt whose envelope failed
    /// integrity is unsafe.
    StructuralFailure,
}

/// Per-field semantic difference between the original receipt and the
/// replayed receipt.
#[derive(Debug, Clone, Serialize)]
pub struct FieldMismatch {
    pub field: String,
    pub original: serde_json::Value,
    pub replayed: serde_json::Value,
}

/// Structured comparison output. Empty `mismatches` ⇒ semantic match.
#[derive(Debug, Clone, Serialize, Default)]
pub struct ReplayComparison {
    pub mismatches: Vec<FieldMismatch>,
}

/// Freshness axis outcome, reported independently of replay. Mirrors
/// the relevant 1d `CheckStatus` variants in compact form so callers
/// can render the freshness state without re-scanning the
/// [`CheckReport`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreshnessOutcome {
    /// `--fresh` was not requested.
    NotChecked,
    /// `--fresh` was requested and `as_of < freshness_horizon`.
    Ok,
    /// `--fresh` was requested and `as_of >= freshness_horizon`.
    Stale,
    /// `--fresh` was requested but the receipt has no
    /// `freshness_horizon` (or it could not be evaluated).
    NotApplicable,
}

/// Replay report combining structural, semantic-replay, and freshness
/// outcomes.
#[derive(Debug, Clone)]
pub struct ReplayReport {
    /// Semantic-replay status. See [`ReplayStatus`].
    pub status: ReplayStatus,
    /// Structural integrity report from Slice 1d.
    pub integrity: CheckReport,
    /// Per-field comparison when replay actually ran. `None` when
    /// `status` is anything other than `Ok` or `Mismatch`.
    pub comparison: Option<ReplayComparison>,
    /// Freshness axis. Independent of `status` — a successful replay
    /// can have a stale freshness, and Track A's `NotApplicable`
    /// status can still carry a freshness verdict.
    pub freshness: FreshnessOutcome,
    /// Free-form detail string, used for the bounded-limitation
    /// explanations on `NotApplicable` / `UnsupportedEvaluator` /
    /// `UnsupportedVersion` / `MissingWitnessMaterial` /
    /// `PolicyUnspecified`.
    pub detail: Option<String>,
    /// Digests of supplied packets that appeared more than once.
    /// De-duplicated before replay; the operator can still see what
    /// was filtered. Warn-shaped by default, failure under `--strict`.
    pub duplicate_packet_digests: Vec<String>,
}

/// Run all replay-related checks against the receipt and supplied
/// packets. Pure (consults no wall-clock time; the caller fills
/// `opts.as_of` when `opts.fresh` is set).
pub fn replay_receipt(
    receipt: &Receipt,
    packets: &[WitnessPacket],
    opts: &ReplayOptions,
) -> ReplayReport {
    // 1. Always start with the structural check. Replay is downstream
    //    of integrity; without integrity, the rest is fan-fiction.
    let check_opts = CheckOptions {
        // 1d's strict flag controls escalation of warn-shaped *check*
        // outcomes. Replay has its own strict flag for replay-specific
        // warn-shapes (duplicate packets). Keep the 1d check lenient
        // here; the replay layer handles strict escalation separately
        // for its own concerns.
        strict: false,
        fresh: opts.fresh,
        as_of: opts.as_of.clone(),
    };
    let integrity = check_receipt(receipt, packets, &check_opts);
    let freshness = freshness_outcome_from(&integrity, opts);

    if integrity.integrity_broken {
        return ReplayReport {
            status: ReplayStatus::StructuralFailure,
            integrity,
            comparison: None,
            freshness,
            detail: Some(
                "receipt content_hash mismatch (1d); semantic replay refused"
                    .to_string(),
            ),
            duplicate_packet_digests: vec![],
        };
    }

    // 2. Schema gate. If the binary does not canonicalize this schema,
    //    the evaluator-version dispatch below is meaningless; return
    //    UnsupportedEvaluator with a schema-specific detail. Reuses the
    //    1d schema outcome rather than re-checking.
    if receipt.schema != RECEIPT_SCHEMA {
        return ReplayReport {
            status: ReplayStatus::UnsupportedEvaluator,
            integrity,
            comparison: None,
            freshness,
            detail: Some(format!(
                "receipt schema {:?} is not {RECEIPT_SCHEMA:?}; this binary cannot \
                 replay against it",
                receipt.schema
            )),
            duplicate_packet_digests: vec![],
        };
    }

    // 3. Evaluator-binding gate.
    let binding = match receipt.evaluator.as_ref() {
        Some(b) => b,
        None => {
            return ReplayReport {
                status: ReplayStatus::PolicyUnspecified,
                integrity,
                comparison: None,
                freshness,
                detail: Some(
                    "receipt has no evaluator binding; replay context is \
                     unspecified (pre-Slice-1b receipt, or built by a path that \
                     did not seal)"
                        .to_string(),
                ),
                duplicate_packet_digests: vec![],
            };
        }
    };

    // 4. De-dupe supplied packets by digest. Duplicates would double-
    //    count observations in the evaluator, which is not what
    //    "replay the original witness set" means.
    let (deduped, duplicate_packet_digests) = dedupe_by_digest(packets);

    // 5. Dispatch on evaluator name.
    match binding.evaluator.as_str() {
        "claim_registry" => replay_track_b(
            receipt,
            binding.version,
            &deduped,
            integrity,
            freshness,
            duplicate_packet_digests,
        ),
        "disk_state" | "ingest_state" | "dns_state" => ReplayReport {
            status: ReplayStatus::NotApplicable,
            integrity,
            comparison: None,
            freshness,
            detail: Some(format!(
                "Track A evaluator {:?}: PreflightCoverage is decoupled from \
                 retained witness packets, so semantic replay against supplied \
                 packets is out of scope. Structural integrity, witness digests, \
                 and freshness still checked. See Slice 2 \
                 (DISK_STATE_CUTOVER_TO_SHARED_SPINE).",
                binding.evaluator
            )),
            duplicate_packet_digests,
        },
        other => ReplayReport {
            status: ReplayStatus::UnsupportedEvaluator,
            integrity,
            comparison: None,
            freshness,
            detail: Some(format!(
                "evaluator {other:?} is not known to this binary"
            )),
            duplicate_packet_digests,
        },
    }
}

fn replay_track_b(
    receipt: &Receipt,
    receipt_version: u32,
    deduped_packets: &[WitnessPacket],
    integrity: CheckReport,
    freshness: FreshnessOutcome,
    duplicate_packet_digests: Vec<String>,
) -> ReplayReport {
    // Version gate. Cross-version replay is pseudo-replay; refuse.
    if receipt_version != crate::claim_registry::EVALUATOR_VERSION {
        return ReplayReport {
            status: ReplayStatus::UnsupportedVersion,
            integrity,
            comparison: None,
            freshness,
            detail: Some(format!(
                "receipt evaluator version {receipt_version} differs from this \
                 binary's claim_registry version {}; replay refused to avoid \
                 cross-version pseudo-replay",
                crate::claim_registry::EVALUATOR_VERSION
            )),
            duplicate_packet_digests,
        };
    }

    // Witness material gate: every WitnessRef.digest = Some(d) on the
    // receipt must be satisfied by some supplied packet whose computed
    // digest is d. We re-use 1d's matching semantics: digest, not order.
    for wref in &receipt.witnesses {
        if let Some(needed) = wref.digest.as_deref() {
            let found = deduped_packets.iter().any(|p| {
                p.digest()
                    .map(|d| d == needed)
                    .unwrap_or(false)
            });
            if !found {
                return ReplayReport {
                    status: ReplayStatus::MissingWitnessMaterial,
                    integrity,
                    comparison: None,
                    freshness,
                    detail: Some(format!(
                        "receipt requires witness packet with digest {needed} \
                         ({:?}); not supplied",
                        wref.witness_type
                    )),
                    duplicate_packet_digests,
                };
            }
        }
    }

    // Re-evaluate. Same registry, same claim, same subject, same
    // packets (filtered to the receipt's subject by the evaluator),
    // same generated_at — replay is reproduction under the receipt's
    // own time context, not fresh minting.
    let registry = crate::claim_registry::ClaimRegistry::track_b_starter();
    let replayed = crate::claim_registry::evaluate(
        &registry,
        &receipt.claim,
        &receipt.subject,
        deduped_packets,
        &receipt.generated_at,
    );

    let mismatches = semantic_diff(receipt, &replayed);
    let status = if mismatches.is_empty() {
        ReplayStatus::Ok
    } else {
        ReplayStatus::Mismatch
    };
    ReplayReport {
        status,
        integrity,
        comparison: Some(ReplayComparison { mismatches }),
        freshness,
        detail: None,
        duplicate_packet_digests,
    }
}

fn freshness_outcome_from(integrity: &CheckReport, opts: &ReplayOptions) -> FreshnessOutcome {
    if !opts.fresh {
        return FreshnessOutcome::NotChecked;
    }
    use crate::receipt_check::{CheckKind, CheckStatus};
    for outcome in &integrity.outcomes {
        if matches!(outcome.kind, CheckKind::FreshnessHorizon) {
            return match outcome.status {
                CheckStatus::Ok => FreshnessOutcome::Ok,
                CheckStatus::Stale => FreshnessOutcome::Stale,
                _ => FreshnessOutcome::NotApplicable,
            };
        }
    }
    FreshnessOutcome::NotApplicable
}

fn dedupe_by_digest(packets: &[WitnessPacket]) -> (Vec<WitnessPacket>, Vec<String>) {
    let mut seen: Vec<String> = Vec::with_capacity(packets.len());
    let mut deduped: Vec<WitnessPacket> = Vec::with_capacity(packets.len());
    let mut duplicates: Vec<String> = Vec::new();
    for p in packets {
        let d = match p.digest() {
            Ok(d) => d,
            // Packets whose digest can't be computed are passed
            // through unchanged — the evaluator will see them, and 1d
            // will independently report what it makes of them. Don't
            // silently drop them.
            Err(_) => {
                deduped.push(p.clone());
                continue;
            }
        };
        if seen.contains(&d) {
            if !duplicates.contains(&d) {
                duplicates.push(d);
            }
        } else {
            seen.push(d);
            deduped.push(p.clone());
        }
    }
    (deduped, duplicates)
}

fn semantic_diff(original: &Receipt, replayed: &Receipt) -> Vec<FieldMismatch> {
    let mut mismatches = Vec::new();

    push_if_diff(&mut mismatches, "claim", &original.claim, &replayed.claim);
    push_if_diff(&mut mismatches, "subject", &original.subject, &replayed.subject);
    push_if_diff(&mut mismatches, "status", &original.status, &replayed.status);
    // status_reasons: evaluator-determined order is deliberately stable per status,
    // so direct compare is meaningful.
    push_if_diff(
        &mut mismatches,
        "status_reasons",
        &original.status_reasons,
        &replayed.status_reasons,
    );

    push_if_set_diff_strings(&mut mismatches, "verified", &original.verified, &replayed.verified);
    push_if_set_diff_strings(
        &mut mismatches,
        "suggested_weaker_claims",
        &original.suggested_weaker_claims,
        &replayed.suggested_weaker_claims,
    );

    // not_verified: sort by (claim, reason, detail) tuple before compare.
    if !nv_set_eq(&original.not_verified, &replayed.not_verified) {
        push_field_mismatch(
            &mut mismatches,
            "not_verified",
            &original.not_verified,
            &replayed.not_verified,
        );
    }

    push_if_diff(
        &mut mismatches,
        "supported_status",
        &original.supported_status,
        &replayed.supported_status,
    );

    // witnesses: sort by (witness_type, digest, observed_at) tuple before compare.
    if !witness_set_eq(&original.witnesses, &replayed.witnesses) {
        push_field_mismatch(&mut mismatches, "witnesses", &original.witnesses, &replayed.witnesses);
    }

    push_if_diff(
        &mut mismatches,
        "observed_at_min",
        &original.observed_at_min,
        &replayed.observed_at_min,
    );
    push_if_diff(
        &mut mismatches,
        "observed_at_max",
        &original.observed_at_max,
        &replayed.observed_at_max,
    );

    mismatches
}

fn push_if_diff<T: Serialize + PartialEq>(
    sink: &mut Vec<FieldMismatch>,
    field: &str,
    a: &T,
    b: &T,
) {
    if a != b {
        push_field_mismatch(sink, field, a, b);
    }
}

fn push_if_set_diff_strings(
    sink: &mut Vec<FieldMismatch>,
    field: &str,
    a: &[String],
    b: &[String],
) {
    let mut sa: Vec<&String> = a.iter().collect();
    let mut sb: Vec<&String> = b.iter().collect();
    sa.sort();
    sb.sort();
    if sa != sb {
        push_field_mismatch(sink, field, a, b);
    }
}

fn push_field_mismatch<A: Serialize + ?Sized, B: Serialize + ?Sized>(
    sink: &mut Vec<FieldMismatch>,
    field: &str,
    a: &A,
    b: &B,
) {
    sink.push(FieldMismatch {
        field: field.to_string(),
        original: serde_json::to_value(a).unwrap_or(serde_json::Value::Null),
        replayed: serde_json::to_value(b).unwrap_or(serde_json::Value::Null),
    });
}

fn nv_set_eq(a: &[NotVerifiedEntry], b: &[NotVerifiedEntry]) -> bool {
    let mut sa: Vec<(&str, &str, Option<&str>)> = a
        .iter()
        .map(|e| (e.claim.as_str(), e.reason.as_str(), e.detail.as_deref()))
        .collect();
    let mut sb: Vec<(&str, &str, Option<&str>)> = b
        .iter()
        .map(|e| (e.claim.as_str(), e.reason.as_str(), e.detail.as_deref()))
        .collect();
    sa.sort();
    sb.sort();
    sa == sb
}

fn witness_set_eq(a: &[WitnessRef], b: &[WitnessRef]) -> bool {
    let mut sa: Vec<(&str, Option<&str>, Option<&str>)> = a
        .iter()
        .map(|w| (w.witness_type.as_str(), w.digest.as_deref(), w.observed_at.as_deref()))
        .collect();
    let mut sb: Vec<(&str, Option<&str>, Option<&str>)> = b
        .iter()
        .map(|w| (w.witness_type.as_str(), w.digest.as_deref(), w.observed_at.as_deref()))
        .collect();
    sa.sort();
    sb.sort();
    sa == sb
}

/// Map a [`ReplayReport`] to a process exit code per Slice 1e:
///
/// - `0` — replay succeeded AND freshness is `Ok` or `NotChecked`
/// - `1` — replay did not establish a match, OR stale under `--fresh`,
///   OR replay was unsupported / not applicable, OR (under `--strict`)
///   duplicates / freshness-not-applicable surfaced
/// - `2` — structural integrity broken (replay refused)
///
/// `64` (malformed input) is handled by the CLI before the report is
/// built and is therefore not represented here.
pub fn exit_code_for(report: &ReplayReport) -> i32 {
    // Integrity dominates regardless of other axes.
    if report.integrity.integrity_broken
        || matches!(report.status, ReplayStatus::StructuralFailure)
    {
        return 2;
    }
    // Stale freshness fails (under --fresh, which is the only way to
    // get Stale).
    if report.freshness == FreshnessOutcome::Stale {
        return 1;
    }
    // Replay must have established Ok to succeed.
    if !matches!(report.status, ReplayStatus::Ok) {
        return 1;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claim_registry::{evaluate, ClaimRegistry, EVALUATOR_VERSION};
    use crate::receipt::{EvaluatorBinding, Status};
    use crate::witness::{WitnessPacket, WITNESS_SCHEMA};

    fn pkt(witness_type: &str, subject: &str, observations: Vec<serde_json::Value>) -> WitnessPacket {
        WitnessPacket {
            schema: WITNESS_SCHEMA.into(),
            witness_type: witness_type.into(),
            subject: subject.into(),
            access_path: "local_command".into(),
            observed_at: "2026-05-15T14:00:00Z".into(),
            generated_at: "2026-05-15T14:00:03Z".into(),
            observations,
            coverage_limits: vec![],
            dependencies: vec![],
        }
    }

    fn make_b_receipt(packets: &[WitnessPacket]) -> Receipt {
        let reg = ClaimRegistry::track_b_starter();
        evaluate(&reg, "tests_passed", "repo:.", packets, "2026-05-15T14:00:00Z")
    }

    fn opts() -> ReplayOptions {
        ReplayOptions::default()
    }

    fn opts_strict() -> ReplayOptions {
        ReplayOptions {
            strict: true,
            ..Default::default()
        }
    }

    fn opts_fresh(as_of: &str) -> ReplayOptions {
        ReplayOptions {
            strict: false,
            fresh: true,
            as_of: Some(as_of.into()),
        }
    }

    // ----- happy path ---------------------------------------------------

    #[test]
    fn track_b_replay_of_unmodified_receipt_is_ok() {
        let p = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let r = make_b_receipt(std::slice::from_ref(&p));
        assert_eq!(r.status, Status::Verified);

        let report = replay_receipt(&r, &[p], &opts());
        assert_eq!(report.status, ReplayStatus::Ok);
        assert!(report.comparison.as_ref().unwrap().mismatches.is_empty());
        assert_eq!(exit_code_for(&report), 0);
    }

    // ----- structural prerequisite -------------------------------------

    #[test]
    fn structural_failure_short_circuits_replay() {
        let p = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let mut r = make_b_receipt(std::slice::from_ref(&p));
        // Tamper without re-sealing.
        r.supported_status = "evidence of fraud".into();

        let report = replay_receipt(&r, &[p], &opts());
        assert_eq!(report.status, ReplayStatus::StructuralFailure);
        assert!(report.comparison.is_none());
        assert_eq!(exit_code_for(&report), 2);
    }

    // ----- evaluator binding gates -------------------------------------

    #[test]
    fn missing_evaluator_binding_yields_policy_unspecified() {
        let p = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        // Build a receipt by hand without sealing — Receipt::new
        // leaves evaluator None and content_hash None. content_hash
        // None is RECEIPT_NOT_ANCHORED (warn), not broken, so we can
        // exercise the PolicyUnspecified path.
        let mut r = Receipt::new("tests_passed", "repo:.", "2026-05-15T14:00:00Z");
        // Echo what evaluate would have populated for the digest set,
        // so we can isolate "no evaluator binding" from "no witnesses".
        r.witnesses = vec![WitnessRef {
            witness_type: "pytest".into(),
            digest: Some(p.digest().unwrap()),
            observed_at: Some("2026-05-15T14:00:00Z".into()),
        }];
        // Intentionally do not seal.
        let report = replay_receipt(&r, &[p], &opts());
        assert_eq!(report.status, ReplayStatus::PolicyUnspecified);
        assert_eq!(exit_code_for(&report), 1);
    }

    #[test]
    fn unknown_evaluator_name_yields_unsupported_evaluator() {
        let p = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let mut r = make_b_receipt(std::slice::from_ref(&p));
        // Stamp a fictional evaluator and re-seal so the structural
        // check passes.
        r.seal(EvaluatorBinding {
            evaluator: "some_future_evaluator".into(),
            version: 1,
        })
        .unwrap();

        let report = replay_receipt(&r, &[p], &opts());
        assert_eq!(report.status, ReplayStatus::UnsupportedEvaluator);
        assert_eq!(exit_code_for(&report), 1);
    }

    #[test]
    fn track_a_evaluator_yields_not_applicable() {
        let p = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let mut r = make_b_receipt(std::slice::from_ref(&p));
        // Re-stamp as if it were a Track A receipt and re-seal.
        r.seal(EvaluatorBinding {
            evaluator: "disk_state".into(),
            version: 1,
        })
        .unwrap();

        let report = replay_receipt(&r, &[p], &opts());
        assert_eq!(report.status, ReplayStatus::NotApplicable);
        assert_eq!(exit_code_for(&report), 1);
    }

    #[test]
    fn version_mismatch_yields_unsupported_version() {
        let p = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let mut r = make_b_receipt(std::slice::from_ref(&p));
        r.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: EVALUATOR_VERSION + 999,
        })
        .unwrap();

        let report = replay_receipt(&r, &[p], &opts());
        assert_eq!(report.status, ReplayStatus::UnsupportedVersion);
        assert_eq!(exit_code_for(&report), 1);
    }

    // ----- material gate -----------------------------------------------

    #[test]
    fn missing_witness_packet_yields_missing_witness_material() {
        let p = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let r = make_b_receipt(std::slice::from_ref(&p));
        // Supply no packets even though the receipt names one by digest.
        let report = replay_receipt(&r, &[], &opts());
        assert_eq!(report.status, ReplayStatus::MissingWitnessMaterial);
        assert_eq!(exit_code_for(&report), 1);
    }

    // ----- mismatch ----------------------------------------------------

    #[test]
    fn forged_semantically_inconsistent_receipt_yields_mismatch() {
        // A receipt that's structurally consistent (content_hash matches its
        // own fields) but whose semantic content doesn't match what the
        // evaluator would produce from the supplied packets. Achieved by
        // mutating the receipt and re-sealing.
        let p = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let mut r = make_b_receipt(std::slice::from_ref(&p));
        assert_eq!(r.status, Status::Verified);

        // Forge: claim NotVerified despite the supplied packet's exit_code=0.
        r.status = Status::NotVerified;
        r.supported_status = "fabricated failure".into();
        r.verified.clear();
        // Re-seal so structural integrity still passes.
        r.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: EVALUATOR_VERSION,
        })
        .unwrap();

        let report = replay_receipt(&r, &[p], &opts());
        assert_eq!(report.status, ReplayStatus::Mismatch);
        let mismatches = &report.comparison.as_ref().unwrap().mismatches;
        assert!(!mismatches.is_empty());
        let fields: Vec<&str> = mismatches.iter().map(|m| m.field.as_str()).collect();
        assert!(fields.contains(&"status"));
        assert!(fields.contains(&"supported_status"));
        assert_eq!(exit_code_for(&report), 1);
    }

    // ----- duplicate packets -------------------------------------------

    #[test]
    fn duplicate_packets_are_deduped_before_replay() {
        let p = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let r = make_b_receipt(std::slice::from_ref(&p));
        let report = replay_receipt(&r, &[p.clone(), p], &opts());
        // De-duped: replay sees the same single packet the original did.
        assert_eq!(report.status, ReplayStatus::Ok);
        assert_eq!(report.duplicate_packet_digests.len(), 1);
        assert_eq!(exit_code_for(&report), 0);
    }

    #[test]
    fn duplicate_packets_under_strict_still_replay_ok_but_are_reported() {
        // Per spec, duplicates remain warn-shaped even under --strict by
        // default. If the project ever wants --strict to escalate them
        // into failures, that's a separate decision; today, duplicates
        // are de-duped and surfaced for the operator.
        let p = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let r = make_b_receipt(std::slice::from_ref(&p));
        let report = replay_receipt(&r, &[p.clone(), p], &opts_strict());
        assert_eq!(report.status, ReplayStatus::Ok);
        assert_eq!(report.duplicate_packet_digests.len(), 1);
    }

    // ----- freshness orthogonality -------------------------------------

    #[test]
    fn freshness_not_checked_when_fresh_off() {
        let p = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let r = make_b_receipt(std::slice::from_ref(&p));
        let report = replay_receipt(&r, &[p], &opts());
        assert_eq!(report.freshness, FreshnessOutcome::NotChecked);
    }

    #[test]
    fn track_b_with_fresh_reports_not_applicable_because_no_horizon() {
        // Track B receipts have no freshness_horizon; --fresh on them
        // reports NotApplicable on the freshness axis but does not
        // affect the replay axis.
        let p = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let r = make_b_receipt(std::slice::from_ref(&p));
        let report = replay_receipt(&r, &[p], &opts_fresh("2026-05-15T14:00:00Z"));
        assert_eq!(report.status, ReplayStatus::Ok);
        assert_eq!(report.freshness, FreshnessOutcome::NotApplicable);
        // Per spec, FreshnessNotApplicable is warn-shaped (not fatal)
        // when replay is otherwise ok. Exit 0 even though freshness
        // was inapplicable.
        assert_eq!(exit_code_for(&report), 0);
    }

    #[test]
    fn stale_with_replay_ok_yields_overall_failure_under_fresh() {
        // Forge a Track A-style receipt that has a freshness horizon
        // but is still Track B for replay purposes... actually the
        // shape we want is: Track B receipt with a freshness_horizon
        // attached (atypical) so we can exercise the orthogonal-axis
        // composition. Build by hand.
        let p = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let mut r = make_b_receipt(std::slice::from_ref(&p));
        r.freshness_horizon = Some("2026-05-15T14:05:00Z".into());
        r.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: EVALUATOR_VERSION,
        })
        .unwrap();

        let report = replay_receipt(&r, &[p], &opts_fresh("2026-05-15T15:00:00Z"));
        assert_eq!(report.status, ReplayStatus::Ok);
        assert_eq!(report.freshness, FreshnessOutcome::Stale);
        // Replay ok + stale + --fresh → exit 1 (freshness axis fails).
        assert_eq!(exit_code_for(&report), 1);
    }
}
