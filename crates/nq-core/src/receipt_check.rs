//! Structural verification of `nq.receipt.v1` documents against supplied
//! `nq.witness.v1` packets. Slice 1d of
//! `docs/architecture/PATH_TO_1_0.md`.
//!
//! This module verifies what the receipt *says about itself*. It does not
//! replay the evaluator, re-ratify the claim, or authorize any downstream
//! action. Its scope is:
//!
//! - **Receipt integrity.** Does `content_hash` match a recomputed
//!   canonical-form SHA-256 of the receipt body?
//! - **Witness anchoring.** For each `WitnessRef.digest = Some(d)` in the
//!   receipt, does some supplied [`WitnessPacket`] hash to `d`?
//! - **Freshness (optional).** If `opts.fresh` is set, is `as_of`
//!   before `freshness_horizon`?
//! - **Schema sanity.** Is the receipt's `schema` something this binary
//!   knows how to interpret canonically?
//!
//! Keepers:
//!
//! > A stale receipt is not a forged receipt. A forged receipt is not a stale receipt.
//!
//! > An unanchored receipt is not a broken receipt.
//!
//! The taxonomy reflects those keepers: `BROKEN_CONTENT_HASH` is the only
//! *broken* status; `STALE`, `NOT_ANCHORED`, `MISSING_WITNESS_PACKET`,
//! `EXTRA_WITNESS_PACKET`, `UNSUPPORTED_*` are all honest statements about
//! what the receipt can or cannot currently prove.

use crate::receipt::{Receipt, RECEIPT_SCHEMA};
use crate::witness::{WitnessPacket, DIGEST_ALGORITHM_PREFIX};

/// Caller-supplied verification options. The CLI maps `--strict` /
/// `--fresh` / `--as-of` onto this struct; programmatic consumers fill it
/// directly.
#[derive(Debug, Clone, Default)]
pub struct CheckOptions {
    /// `--strict`. When true, "warn-shaped" statuses (e.g. unanchored
    /// fields, missing packets, freshness horizon absent) escalate to
    /// failures. When false, those statuses are reported but do not
    /// push the exit code above 0.
    pub strict: bool,
    /// `--fresh`. When true, run a freshness check comparing `as_of`
    /// against `Receipt::freshness_horizon`. When false, freshness is
    /// not consulted (not even as a warning).
    pub fresh: bool,
    /// RFC3339 UTC timestamp to evaluate freshness against. Only
    /// consulted when `fresh` is true. CLI implication: `--as-of`
    /// implies `--fresh`. When `fresh` is true and `as_of` is `None`,
    /// the caller is responsible for substituting wall-clock now before
    /// invoking [`check_receipt`].
    pub as_of: Option<String>,
}

/// What was checked. Carried alongside [`CheckStatus`] so renderers can
/// produce specific operator-facing messages without re-deriving context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckKind {
    /// The receipt's `schema` field was inspected.
    Schema { schema: String },
    /// The receipt's `content_hash` integrity was checked.
    ContentHash,
    /// A specific `WitnessRef` was inspected. `digest` is the digest
    /// string carried on the WitnessRef (or `None` if the ref had no
    /// digest at all).
    WitnessDigest {
        witness_type: String,
        digest: Option<String>,
    },
    /// A supplied witness packet whose computed digest did not match any
    /// `WitnessRef` in the receipt.
    ExtraSuppliedPacket { computed_digest: String },
    /// Freshness horizon was checked (only emitted when `opts.fresh`).
    FreshnessHorizon,
    /// Informational record of the evaluator binding the receipt carries.
    /// Always status `Ok` — verification of evaluator semantics is
    /// outside 1d's scope.
    EvaluatorBinding { evaluator: String, version: u32 },
}

/// Status of one check. Designed so renderers can group by severity and
/// exit-code adapters can compute a worst-case integer without
/// reinterpreting each variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    /// Check passed. Default disposition for present-and-correct
    /// integrity fields, witness digests that matched, fresh receipts,
    /// recognized schema, etc.
    Ok,
    /// The receipt has no `content_hash` field at all (e.g., produced
    /// before Slice 1b, or by a path that did not call `Receipt::seal`).
    /// Not "broken" — no integrity claim was made, so no integrity
    /// claim was violated.
    ReceiptNotAnchored,
    /// **Broken.** `content_hash` is present and does not match the
    /// canonical bytes recomputed from the rest of the receipt. The
    /// receipt object is no longer trusted; other checks in the same
    /// report are diagnostic only.
    BrokenContentHash,
    /// A `WitnessRef` in the receipt has `digest: None`. The receipt
    /// did not claim integrity for that witness; no integrity check is
    /// possible. Not broken.
    WitnessNotAnchored,
    /// A `WitnessRef` has `digest: Some(d)` but no supplied packet
    /// hashes to `d`. The receipt's claim cannot be checked because the
    /// packet was not provided.
    MissingWitnessPacket,
    /// A supplied packet's computed digest did not match any
    /// `WitnessRef.digest` in the receipt. Default disposition: warn.
    ExtraWitnessPacket,
    /// A digest string on a `WitnessRef` does not have the expected
    /// `algorithm:hex` shape and cannot be interpreted.
    MalformedDigest,
    /// A digest string on a `WitnessRef` carries an algorithm prefix
    /// this binary does not implement (e.g. `blake3:`). The receipt
    /// is not broken — this binary cannot interpret it.
    UnsupportedDigestAlgorithm,
    /// `freshness_horizon` is present and `as_of >= freshness_horizon`.
    /// Receipt may be honest; testimony is past the evaluator's
    /// declared policy. Reported only when `opts.fresh`.
    Stale,
    /// `--fresh` was requested, but the receipt does not carry a
    /// `freshness_horizon`. The check is not applicable — the receipt
    /// itself does not declare a deadline this consumer can evaluate.
    FreshnessNotApplicable,
    /// The receipt's `schema` field is not the value this binary
    /// canonicalizes (`nq.receipt.v1`). Out-of-domain, not broken.
    UnsupportedReceiptVersion,
}

impl CheckStatus {
    /// True iff this status is structurally a contradiction — the
    /// receipt claimed something that proved untrue. Today only
    /// `BrokenContentHash` qualifies; `WitnessRef.digest` mismatches
    /// surface as `MissingWitnessPacket` / `ExtraWitnessPacket` because
    /// digest-based matching cannot distinguish "wrong packet supplied
    /// for this ref" from "right packet not supplied at all" without a
    /// second matching predicate.
    pub fn is_broken(self) -> bool {
        matches!(self, Self::BrokenContentHash)
    }
}

/// One check outcome.
#[derive(Debug, Clone)]
pub struct CheckOutcome {
    pub kind: CheckKind,
    pub status: CheckStatus,
    pub detail: Option<String>,
}

/// Report from [`check_receipt`]. The `outcomes` vector is the full
/// per-check list in evaluation order; `integrity_broken` is set when
/// any outcome's status is broken.
#[derive(Debug, Clone)]
pub struct CheckReport {
    pub outcomes: Vec<CheckOutcome>,
    /// True iff at least one outcome had `status.is_broken()`. When set,
    /// non-broken outcomes in the same report are *diagnostic only*: the
    /// receipt object is no longer trusted, so its other fields cannot
    /// be taken as evidence on their own.
    pub integrity_broken: bool,
}

impl CheckReport {
    fn push(&mut self, outcome: CheckOutcome) {
        if outcome.status.is_broken() {
            self.integrity_broken = true;
        }
        self.outcomes.push(outcome);
    }
}

/// Run all checks against the receipt and the supplied packets. Pure;
/// does not consult wall-clock time (the caller must populate
/// `opts.as_of` if `opts.fresh` is set).
pub fn check_receipt(
    receipt: &Receipt,
    packets: &[WitnessPacket],
    opts: &CheckOptions,
) -> CheckReport {
    let mut report = CheckReport {
        outcomes: Vec::new(),
        integrity_broken: false,
    };

    // 1. Schema. If the schema is something this binary does not
    //    canonicalize, downstream checks (content_hash, digests) cannot
    //    be meaningfully run — their canonicalization rules may have
    //    changed. Emit the schema outcome and short-circuit those
    //    deeper checks, but DO still emit freshness / evaluator
    //    outcomes if the field shapes allow.
    let schema_supported = receipt.schema == RECEIPT_SCHEMA;
    report.push(CheckOutcome {
        kind: CheckKind::Schema {
            schema: receipt.schema.clone(),
        },
        status: if schema_supported {
            CheckStatus::Ok
        } else {
            CheckStatus::UnsupportedReceiptVersion
        },
        detail: if schema_supported {
            None
        } else {
            Some(format!(
                "receipt schema {:?} is not {RECEIPT_SCHEMA:?}; this binary cannot \
                 canonicalize it",
                receipt.schema
            ))
        },
    });

    // 2. Content hash. Only meaningful when the schema is recognized;
    //    otherwise the canonical form is undefined.
    if schema_supported {
        match (receipt.content_hash.as_deref(), receipt.compute_content_hash()) {
            (None, _) => {
                report.push(CheckOutcome {
                    kind: CheckKind::ContentHash,
                    status: CheckStatus::ReceiptNotAnchored,
                    detail: Some(
                        "receipt has no content_hash; no integrity claim to verify"
                            .to_string(),
                    ),
                });
            }
            (Some(stored), Ok(recomputed)) if stored == recomputed => {
                report.push(CheckOutcome {
                    kind: CheckKind::ContentHash,
                    status: CheckStatus::Ok,
                    detail: None,
                });
            }
            (Some(stored), Ok(recomputed)) => {
                report.push(CheckOutcome {
                    kind: CheckKind::ContentHash,
                    status: CheckStatus::BrokenContentHash,
                    detail: Some(format!(
                        "stored content_hash {stored} does not match recomputed \
                         {recomputed}"
                    )),
                });
            }
            (Some(stored), Err(e)) => {
                // Recomputation itself failed (canonicalization rejected
                // a value). Cannot prove the stored hash is right or
                // wrong — surface as broken since trust is gone either
                // way.
                report.push(CheckOutcome {
                    kind: CheckKind::ContentHash,
                    status: CheckStatus::BrokenContentHash,
                    detail: Some(format!(
                        "could not recompute hash to compare against stored {stored}: {e}"
                    )),
                });
            }
        }
    }

    // 3. Witness anchoring. Two passes:
    //    - one outcome per WitnessRef in the receipt
    //    - one outcome per supplied packet that wasn't claimed
    //
    //    Digest is the matching surface; witness_type-name matching is
    //    not consulted (digest covers the full canonical envelope
    //    including witness_type — a digest match implies a witness_type
    //    match, modulo SHA-256 collision).
    let packet_digests: Vec<(String, &WitnessPacket)> = packets
        .iter()
        .filter_map(|p| p.digest().ok().map(|d| (d, p)))
        .collect();
    let mut packet_used = vec![false; packet_digests.len()];

    for wref in &receipt.witnesses {
        match wref.digest.as_deref() {
            None => {
                report.push(CheckOutcome {
                    kind: CheckKind::WitnessDigest {
                        witness_type: wref.witness_type.clone(),
                        digest: None,
                    },
                    status: CheckStatus::WitnessNotAnchored,
                    detail: Some(format!(
                        "witness ref {:?} has no digest; the receipt did not anchor it",
                        wref.witness_type
                    )),
                });
            }
            Some(d) => match parse_digest(d) {
                DigestShape::Sha256 => {
                    let matched = packet_digests
                        .iter()
                        .enumerate()
                        .find(|(_, (pd, _))| pd.as_str() == d);
                    match matched {
                        Some((idx, _)) => {
                            packet_used[idx] = true;
                            report.push(CheckOutcome {
                                kind: CheckKind::WitnessDigest {
                                    witness_type: wref.witness_type.clone(),
                                    digest: Some(d.to_string()),
                                },
                                status: CheckStatus::Ok,
                                detail: None,
                            });
                        }
                        None => {
                            report.push(CheckOutcome {
                                kind: CheckKind::WitnessDigest {
                                    witness_type: wref.witness_type.clone(),
                                    digest: Some(d.to_string()),
                                },
                                status: CheckStatus::MissingWitnessPacket,
                                detail: Some(format!(
                                    "no supplied packet hashes to {d}"
                                )),
                            });
                        }
                    }
                }
                DigestShape::UnsupportedAlgorithm(prefix) => {
                    report.push(CheckOutcome {
                        kind: CheckKind::WitnessDigest {
                            witness_type: wref.witness_type.clone(),
                            digest: Some(d.to_string()),
                        },
                        status: CheckStatus::UnsupportedDigestAlgorithm,
                        detail: Some(format!(
                            "digest prefix {prefix:?} is not implemented by this binary"
                        )),
                    });
                }
                DigestShape::Malformed => {
                    report.push(CheckOutcome {
                        kind: CheckKind::WitnessDigest {
                            witness_type: wref.witness_type.clone(),
                            digest: Some(d.to_string()),
                        },
                        status: CheckStatus::MalformedDigest,
                        detail: Some(format!(
                            "digest string {d:?} does not parse as algorithm:hex"
                        )),
                    });
                }
            },
        }
    }

    for (idx, (digest, _packet)) in packet_digests.iter().enumerate() {
        if !packet_used[idx] {
            report.push(CheckOutcome {
                kind: CheckKind::ExtraSuppliedPacket {
                    computed_digest: digest.clone(),
                },
                status: CheckStatus::ExtraWitnessPacket,
                detail: Some(format!(
                    "supplied packet {digest} does not correspond to any WitnessRef in the receipt"
                )),
            });
        }
    }

    // 4. Freshness. Only when --fresh was requested.
    if opts.fresh {
        match (
            receipt.freshness_horizon.as_deref(),
            opts.as_of.as_deref(),
        ) {
            (None, _) => {
                report.push(CheckOutcome {
                    kind: CheckKind::FreshnessHorizon,
                    status: CheckStatus::FreshnessNotApplicable,
                    detail: Some(
                        "receipt has no freshness_horizon; --fresh cannot be evaluated"
                            .to_string(),
                    ),
                });
            }
            (Some(horizon), None) => {
                report.push(CheckOutcome {
                    kind: CheckKind::FreshnessHorizon,
                    status: CheckStatus::FreshnessNotApplicable,
                    detail: Some(format!(
                        "--fresh requested but no as_of supplied; horizon {horizon} not evaluated"
                    )),
                });
            }
            (Some(horizon), Some(as_of)) => {
                let parsed_horizon = parse_rfc3339(horizon);
                let parsed_as_of = parse_rfc3339(as_of);
                match (parsed_horizon, parsed_as_of) {
                    (Some(h), Some(a)) => {
                        let status = if a < h {
                            CheckStatus::Ok
                        } else {
                            CheckStatus::Stale
                        };
                        report.push(CheckOutcome {
                            kind: CheckKind::FreshnessHorizon,
                            status,
                            detail: Some(format!(
                                "as_of={as_of} horizon={horizon}"
                            )),
                        });
                    }
                    _ => {
                        report.push(CheckOutcome {
                            kind: CheckKind::FreshnessHorizon,
                            status: CheckStatus::FreshnessNotApplicable,
                            detail: Some(format!(
                                "could not parse as_of {as_of:?} or horizon {horizon:?} as RFC3339"
                            )),
                        });
                    }
                }
            }
        }
    }

    // 5. Evaluator binding. Informational only.
    if let Some(ev) = receipt.evaluator.as_ref() {
        report.push(CheckOutcome {
            kind: CheckKind::EvaluatorBinding {
                evaluator: ev.evaluator.clone(),
                version: ev.version,
            },
            status: CheckStatus::Ok,
            detail: None,
        });
    }

    report
}

/// Map a [`CheckReport`] to a process exit code following the contract
/// in `docs/architecture/PATH_TO_1_0.md` Slice 1d:
///
/// - `0` — check completed; no broken integrity, no failures
/// - `1` — requested check failed without proving corruption (stale,
///         missing material under `--strict`, unsupported algorithm,
///         unsupported schema)
/// - `2` — broken integrity (`content_hash` mismatch)
///
/// `64` (malformed input) is the CLI's territory and is returned before
/// the report is built.
pub fn exit_code_for(report: &CheckReport, strict: bool) -> i32 {
    if report.integrity_broken {
        return 2;
    }
    let mut worst: i32 = 0;
    for outcome in &report.outcomes {
        let candidate: i32 = match outcome.status {
            CheckStatus::Ok => 0,
            CheckStatus::ReceiptNotAnchored
            | CheckStatus::WitnessNotAnchored
            | CheckStatus::MissingWitnessPacket
            | CheckStatus::ExtraWitnessPacket
            | CheckStatus::FreshnessNotApplicable => {
                if strict {
                    1
                } else {
                    0
                }
            }
            CheckStatus::Stale
            | CheckStatus::MalformedDigest
            | CheckStatus::UnsupportedDigestAlgorithm
            | CheckStatus::UnsupportedReceiptVersion => 1,
            CheckStatus::BrokenContentHash => 2,
        };
        if candidate > worst {
            worst = candidate;
        }
    }
    worst
}

enum DigestShape<'a> {
    Sha256,
    UnsupportedAlgorithm(&'a str), // algorithm prefix including trailing colon
    Malformed,
}

fn parse_digest(s: &str) -> DigestShape<'_> {
    let Some(colon) = s.find(':') else {
        return DigestShape::Malformed;
    };
    let (algo_with_colon, rest) = s.split_at(colon + 1);
    if algo_with_colon == DIGEST_ALGORITHM_PREFIX {
        // sha256:<hex>; require 64 lowercase hex chars to be considered well-formed.
        if rest.len() != 64
            || !rest
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return DigestShape::Malformed;
        }
        DigestShape::Sha256
    } else {
        DigestShape::UnsupportedAlgorithm(algo_with_colon)
    }
}

fn parse_rfc3339(s: &str) -> Option<time::OffsetDateTime> {
    time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::receipt::{EvaluatorBinding, WitnessRef};
    use crate::witness::WITNESS_SCHEMA;

    fn sample_packet(witness_type: &str, subject: &str, observed_at: &str) -> WitnessPacket {
        WitnessPacket {
            schema: WITNESS_SCHEMA.into(),
            witness_type: witness_type.into(),
            subject: subject.into(),
            access_path: "local_command".into(),
            observed_at: observed_at.into(),
            generated_at: observed_at.into(),
            observations: vec![serde_json::json!({"type": "x"})],
            coverage_limits: vec![],
            dependencies: vec![],
            custody_basis: None,
            source_finding_ref: None,
            projection_limits: vec![],
        }
    }

    fn sample_receipt() -> Receipt {
        let mut r = Receipt::new("tests_passed", "repo:.", "2026-05-15T14:00:00Z");
        r.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        })
        .unwrap();
        r
    }

    fn opts() -> CheckOptions {
        CheckOptions::default()
    }

    fn opts_fresh(as_of: &str) -> CheckOptions {
        CheckOptions {
            strict: false,
            fresh: true,
            as_of: Some(as_of.into()),
        }
    }

    fn opts_strict() -> CheckOptions {
        CheckOptions {
            strict: true,
            ..Default::default()
        }
    }

    // -- content hash --------------------------------------------------

    #[test]
    fn valid_sealed_receipt_passes_content_hash_check() {
        let r = sample_receipt();
        let report = check_receipt(&r, &[], &opts());
        assert!(!report.integrity_broken);
        let outcome = report
            .outcomes
            .iter()
            .find(|o| matches!(o.kind, CheckKind::ContentHash))
            .expect("content_hash outcome present");
        assert_eq!(outcome.status, CheckStatus::Ok);
        assert_eq!(exit_code_for(&report, false), 0);
    }

    #[test]
    fn unsealed_receipt_reports_receipt_not_anchored() {
        let r = Receipt::new("c", "s", "2026-05-15T14:00:00Z");
        let report = check_receipt(&r, &[], &opts());
        assert!(!report.integrity_broken);
        let outcome = report
            .outcomes
            .iter()
            .find(|o| matches!(o.kind, CheckKind::ContentHash))
            .unwrap();
        assert_eq!(outcome.status, CheckStatus::ReceiptNotAnchored);
        assert_eq!(exit_code_for(&report, false), 0);
        assert_eq!(exit_code_for(&report, true), 1);
    }

    #[test]
    fn tampered_receipt_reports_broken_content_hash() {
        let mut r = sample_receipt();
        // Tamper: change a field without re-sealing.
        r.supported_status = "evidence of fraud".into();
        let report = check_receipt(&r, &[], &opts());
        assert!(report.integrity_broken);
        let outcome = report
            .outcomes
            .iter()
            .find(|o| matches!(o.kind, CheckKind::ContentHash))
            .unwrap();
        assert_eq!(outcome.status, CheckStatus::BrokenContentHash);
        assert_eq!(exit_code_for(&report, false), 2);
        assert_eq!(exit_code_for(&report, true), 2);
    }

    // -- witness anchoring --------------------------------------------

    #[test]
    fn witness_with_matching_supplied_packet_passes() {
        let packet = sample_packet("pytest", "repo:.", "2026-05-15T14:00:00Z");
        let digest = packet.digest().unwrap();
        let mut r = Receipt::new("c", "s", "2026-05-15T14:00:00Z");
        r.witnesses = vec![WitnessRef {
            witness_type: "pytest".into(),
            digest: Some(digest.clone()),
            observed_at: Some("2026-05-15T14:00:00Z".into()),
        }];
        r.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        })
        .unwrap();

        let report = check_receipt(&r, &[packet], &opts());
        let wd = report
            .outcomes
            .iter()
            .find(|o| matches!(o.kind, CheckKind::WitnessDigest { .. }))
            .unwrap();
        assert_eq!(wd.status, CheckStatus::Ok);
        // No "extra" outcome should appear; the packet matched.
        assert!(!report
            .outcomes
            .iter()
            .any(|o| matches!(o.kind, CheckKind::ExtraSuppliedPacket { .. })));
        assert_eq!(exit_code_for(&report, false), 0);
    }

    #[test]
    fn witness_with_missing_packet_reports_missing_under_default_no_packets_supplied() {
        let mut r = sample_receipt();
        r.witnesses = vec![WitnessRef {
            witness_type: "pytest".into(),
            digest: Some("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()),
            observed_at: None,
        }];
        // Re-seal so content_hash matches the witness vec.
        r.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        })
        .unwrap();

        let report = check_receipt(&r, &[], &opts());
        let wd = report
            .outcomes
            .iter()
            .find(|o| matches!(o.kind, CheckKind::WitnessDigest { .. }))
            .unwrap();
        assert_eq!(wd.status, CheckStatus::MissingWitnessPacket);
        assert_eq!(exit_code_for(&report, false), 0);
        assert_eq!(exit_code_for(&report, true), 1);
    }

    #[test]
    fn unanchored_witness_ref_reports_witness_not_anchored() {
        let mut r = sample_receipt();
        r.witnesses = vec![WitnessRef {
            witness_type: "pytest".into(),
            digest: None,
            observed_at: None,
        }];
        r.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        })
        .unwrap();

        let report = check_receipt(&r, &[], &opts());
        let wd = report
            .outcomes
            .iter()
            .find(|o| matches!(o.kind, CheckKind::WitnessDigest { .. }))
            .unwrap();
        assert_eq!(wd.status, CheckStatus::WitnessNotAnchored);
        assert_eq!(exit_code_for(&report, false), 0);
        assert_eq!(exit_code_for(&report, true), 1);
    }

    #[test]
    fn extra_supplied_packet_reports_extra_witness_packet() {
        let packet = sample_packet("pytest", "repo:.", "2026-05-15T14:00:00Z");
        let r = sample_receipt(); // empty witnesses
        let report = check_receipt(&r, &[packet], &opts());
        let extra = report
            .outcomes
            .iter()
            .find(|o| matches!(o.kind, CheckKind::ExtraSuppliedPacket { .. }))
            .unwrap();
        assert_eq!(extra.status, CheckStatus::ExtraWitnessPacket);
        assert_eq!(exit_code_for(&report, false), 0);
        assert_eq!(exit_code_for(&report, true), 1);
    }

    #[test]
    fn malformed_digest_string_is_reported() {
        let mut r = sample_receipt();
        r.witnesses = vec![WitnessRef {
            witness_type: "pytest".into(),
            digest: Some("not-a-digest".into()),
            observed_at: None,
        }];
        r.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        })
        .unwrap();

        let report = check_receipt(&r, &[], &opts());
        let wd = report
            .outcomes
            .iter()
            .find(|o| matches!(o.kind, CheckKind::WitnessDigest { .. }))
            .unwrap();
        assert_eq!(wd.status, CheckStatus::MalformedDigest);
        assert_eq!(exit_code_for(&report, false), 1);
    }

    #[test]
    fn unsupported_digest_algorithm_is_reported() {
        let mut r = sample_receipt();
        r.witnesses = vec![WitnessRef {
            witness_type: "pytest".into(),
            digest: Some("blake3:cafebabe".into()),
            observed_at: None,
        }];
        r.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        })
        .unwrap();

        let report = check_receipt(&r, &[], &opts());
        let wd = report
            .outcomes
            .iter()
            .find(|o| matches!(o.kind, CheckKind::WitnessDigest { .. }))
            .unwrap();
        assert_eq!(wd.status, CheckStatus::UnsupportedDigestAlgorithm);
        assert_eq!(exit_code_for(&report, false), 1);
    }

    // -- freshness ----------------------------------------------------

    #[test]
    fn fresh_receipt_passes_freshness_check() {
        let mut r = sample_receipt();
        r.freshness_horizon = Some("2026-05-15T14:05:00Z".into());
        r.seal(EvaluatorBinding {
            evaluator: "dns_state".into(),
            version: 1,
        })
        .unwrap();

        let report = check_receipt(&r, &[], &opts_fresh("2026-05-15T14:02:00Z"));
        let fh = report
            .outcomes
            .iter()
            .find(|o| matches!(o.kind, CheckKind::FreshnessHorizon))
            .unwrap();
        assert_eq!(fh.status, CheckStatus::Ok);
        assert_eq!(exit_code_for(&report, false), 0);
    }

    #[test]
    fn stale_receipt_reports_stale_under_fresh() {
        let mut r = sample_receipt();
        r.freshness_horizon = Some("2026-05-15T14:05:00Z".into());
        r.seal(EvaluatorBinding {
            evaluator: "dns_state".into(),
            version: 1,
        })
        .unwrap();

        let report = check_receipt(&r, &[], &opts_fresh("2026-05-15T15:00:00Z"));
        let fh = report
            .outcomes
            .iter()
            .find(|o| matches!(o.kind, CheckKind::FreshnessHorizon))
            .unwrap();
        assert_eq!(fh.status, CheckStatus::Stale);
        // Stale exits 1 regardless of --strict.
        assert_eq!(exit_code_for(&report, false), 1);
        assert_eq!(exit_code_for(&report, true), 1);
    }

    #[test]
    fn no_horizon_with_fresh_flag_reports_freshness_not_applicable() {
        let r = sample_receipt(); // no horizon
        let report = check_receipt(&r, &[], &opts_fresh("2026-05-15T14:00:00Z"));
        let fh = report
            .outcomes
            .iter()
            .find(|o| matches!(o.kind, CheckKind::FreshnessHorizon))
            .unwrap();
        assert_eq!(fh.status, CheckStatus::FreshnessNotApplicable);
        assert_eq!(exit_code_for(&report, false), 0);
        assert_eq!(exit_code_for(&report, true), 1);
    }

    #[test]
    fn fresh_flag_off_does_not_emit_freshness_outcome() {
        let mut r = sample_receipt();
        r.freshness_horizon = Some("2026-05-15T14:05:00Z".into());
        r.seal(EvaluatorBinding {
            evaluator: "dns_state".into(),
            version: 1,
        })
        .unwrap();

        let report = check_receipt(&r, &[], &opts());
        assert!(!report
            .outcomes
            .iter()
            .any(|o| matches!(o.kind, CheckKind::FreshnessHorizon)));
    }

    // -- schema --------------------------------------------------------

    #[test]
    fn unrecognized_schema_reports_unsupported_receipt_version() {
        let mut r = sample_receipt();
        r.schema = "nq.receipt.v99".into();
        let report = check_receipt(&r, &[], &opts());
        let sch = report
            .outcomes
            .iter()
            .find(|o| matches!(o.kind, CheckKind::Schema { .. }))
            .unwrap();
        assert_eq!(sch.status, CheckStatus::UnsupportedReceiptVersion);
        // Content hash is not checked when schema is unsupported —
        // canonicalization rules may have changed.
        assert!(!report
            .outcomes
            .iter()
            .any(|o| matches!(o.kind, CheckKind::ContentHash)));
        assert_eq!(exit_code_for(&report, false), 1);
        assert_eq!(exit_code_for(&report, true), 1);
    }

    // -- evaluator binding --------------------------------------------

    #[test]
    fn evaluator_binding_is_reported_informationally() {
        let r = sample_receipt();
        let report = check_receipt(&r, &[], &opts());
        let ev = report
            .outcomes
            .iter()
            .find(|o| matches!(o.kind, CheckKind::EvaluatorBinding { .. }))
            .unwrap();
        assert_eq!(ev.status, CheckStatus::Ok);
    }

    // -- integrity-broken cascade -------------------------------------

    #[test]
    fn integrity_broken_short_circuits_overall_exit_code_to_two() {
        // Even if downstream outcomes are "Ok", a broken content hash
        // dominates the exit code.
        let mut r = sample_receipt();
        r.supported_status = "tampered".into();
        let report = check_receipt(&r, &[], &opts());
        assert!(report.integrity_broken);
        assert_eq!(exit_code_for(&report, false), 2);
    }

    #[test]
    fn strict_pushes_unanchored_to_failure_but_not_to_broken() {
        let mut r = Receipt::new("c", "s", "2026-05-15T14:00:00Z");
        r.witnesses = vec![WitnessRef {
            witness_type: "pytest".into(),
            digest: None,
            observed_at: None,
        }];
        // Intentionally NOT sealed — receipt is unanchored too.
        let report = check_receipt(&r, &[], &opts_strict());
        // Both ReceiptNotAnchored and WitnessNotAnchored should appear.
        assert!(report
            .outcomes
            .iter()
            .any(|o| o.status == CheckStatus::ReceiptNotAnchored));
        assert!(report
            .outcomes
            .iter()
            .any(|o| o.status == CheckStatus::WitnessNotAnchored));
        assert!(!report.integrity_broken);
        assert_eq!(exit_code_for(&report, true), 1);
        // Without --strict, both are warn-shaped.
        assert_eq!(exit_code_for(&report, false), 0);
    }
}
