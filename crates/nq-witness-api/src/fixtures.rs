//! Witness-owned fixture surface for `nq_evaluator_state` liveness
//! probing.
//!
//! See `docs/working/decisions/preflights/NQ_EVALUATOR_STATE.md` §9.
//!
//! A `Fixture` is the canonical invocation specification the probe
//! uses to exercise a per-kind evaluator's substrate query path.
//! Fixtures live in this crate — not in `nq-db` / `nq-monitor` —
//! because the W/E forward guardrail requires the evaluator under
//! test to NOT author or mutate its own fixture. The contract crate
//! is the structural enforcement of that discipline.
//!
//! V0 fixture shape (per kind): canonical JSON describing the
//! evaluator's invocation parameters (target tuple, any per-kind
//! options). The probe's per-kind adapter parses this JSON into the
//! per-kind target struct in nq-db, then invokes the evaluator
//! against the production substrate. Synthetic JSON is sufficient;
//! procedural builders are deferred.
//!
//! **Fixture identity is content-addressed.** `Fixture::hash()`
//! returns `"sha256:<64-hex>"` over the canonical-JSON content. Two
//! observations against the same fixture produce the same hash; a
//! fixture modification produces a new hash, and prior observations
//! remain interpretable against their then-active hash.
//!
//! **Fixture coverage is narrow.** A passing probe means the
//! evaluator path responded to *this fixture* at time T. It does NOT
//! mean the evaluator handles all inputs, edge cases, or pathological
//! substrate states. Broader coverage is deferred; promotion would
//! require a fixture-shape specification this V0 does not author.
//!
//! **`NqEvaluatorState` has no fixture.** The probe loop skips its
//! own kind — preflight §2's self-witness-collapse refusal.

use nq_core::preflight::ClaimKind;
use sha2::{Digest, Sha256};

/// One witness-owned fixture: the canonical invocation specification
/// the probe applies to a per-kind evaluator. The `canonical_json`
/// field is the load-bearing identity; `id` is operator-facing prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fixture {
    /// Operator-facing identifier. Convention: `<kind>.v<version>.<variant>`
    /// (e.g., `"disk_state.v1.minimal"`). Stable across the lifetime
    /// of a `canonical_json` value.
    pub id: &'static str,

    /// The kind whose evaluator this fixture targets.
    pub claim_kind: ClaimKind,

    /// Canonical JSON describing the evaluator's invocation parameters.
    /// Content-addressed via `Fixture::hash()`. Stored as a `&'static str`
    /// (compile-time constant) so the hash is stable across processes.
    ///
    /// The shape is per-kind — see the per-kind `pub const` items below
    /// for the expected JSON structure. The probe's per-kind adapter
    /// in `nq-monitor::nq_evaluator_probe` parses this into the per-kind
    /// target struct.
    ///
    /// Conventions:
    /// - The fixture uses the placeholder host `"nq.fixture.local"`
    ///   wherever a host name appears. The probe substitutes the real
    ///   probe host at invocation time. The fixture's hash is over the
    ///   placeholder shape, not over the substituted invocation.
    /// - JSON is two-space-indented for human readability. Whitespace
    ///   is part of the hashed content; reformatting a fixture is a
    ///   hash-changing edit.
    pub canonical_json: &'static str,
}

impl Fixture {
    /// Content-addressed identity over `canonical_json`. Returns
    /// `"sha256:<64-hex>"` to match the migration-056 structural
    /// CHECK on `nq_evaluator_observations.fixture_hash`.
    pub fn hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.canonical_json.as_bytes());
        format!("sha256:{}", hex::encode(hasher.finalize()))
    }
}

// -----------------------------------------------------------------
// Per-kind fixtures (V0).
//
// Five fixtures covering the five per-kind evaluators currently
// supported by the probe. Each fixture is the smallest invocation
// specification that exercises the evaluator's substrate query path.
//
// Excluded from V0:
//   - `ClaimKind::NqEvaluatorState`         (self-witness collapse)
//   - `ClaimKind::ComponentTestimonyObservationLoopAlive`
//                                           (heartbeat shape; deferred
//                                            until a forcing case names
//                                            its fixture surface)
// -----------------------------------------------------------------

/// Fixture for `evaluate_disk_state_preflight_from_conn(conn, host, target=None)`.
///
/// The disk_state evaluator queries `findings` / substrate for any
/// disk-related signal scoped to `host`. The fixture's placeholder
/// host is substituted at probe time; the evaluator returns whatever
/// shape its substrate query produces. V0 cares only about
/// shape-validity, not about a specific verdict.
pub const DISK_STATE_V1_MINIMAL: Fixture = Fixture {
    id: "disk_state.v1.minimal",
    claim_kind: ClaimKind::DiskState,
    canonical_json: r#"{
  "host": "nq.fixture.local",
  "target": null
}"#,
};

/// Fixture for `evaluate_ingest_state_preflight_from_conn(conn)`.
///
/// The ingest_state evaluator takes no positional parameters — it
/// reads the latest generation row and projects an ingest receipt.
/// The fixture body is an empty object; the placeholder is recorded
/// for symmetry with the other kinds (future hash-stamping logic may
/// reuse the same canonical-JSON parsing path).
pub const INGEST_STATE_V1_MINIMAL: Fixture = Fixture {
    id: "ingest_state.v1.minimal",
    claim_kind: ClaimKind::IngestState,
    canonical_json: r#"{}"#,
};

/// Fixture for `evaluate_dns_state_preflight_from_conn(conn, &DnsObservationTuple)`.
///
/// V0 uses a known-stable DNS tuple. The vantage host is the
/// placeholder; resolver/query_name/query_type carry canonical
/// values. The probe substitutes the real vantage host; the
/// evaluator queries `dns_observations` for that tuple and returns
/// whatever shape its substrate produces (typically
/// `InsufficientCoverage` for the placeholder tuple in production).
pub const DNS_STATE_V1_MINIMAL: Fixture = Fixture {
    id: "dns_state.v1.minimal",
    claim_kind: ClaimKind::DnsState,
    canonical_json: r#"{
  "vantage_host": "nq.fixture.local",
  "resolver": "1.1.1.1",
  "query_name": "nq.fixture.local",
  "query_type": "A"
}"#,
};

/// Fixture for `evaluate_sqlite_wal_state_preflight_at(conn, &SqliteWalTarget, now)`.
///
/// V0 targets a placeholder DB path under `/var/lib/nq.fixture/`.
/// The path is fictional; the evaluator returns `InsufficientCoverage`
/// (no rows for the placeholder tuple) — which IS a shape-valid
/// result for V0 liveness purposes.
pub const SQLITE_WAL_STATE_V1_MINIMAL: Fixture = Fixture {
    id: "sqlite_wal_state.v1.minimal",
    claim_kind: ClaimKind::SqliteWalState,
    canonical_json: r#"{
  "host": "nq.fixture.local",
  "db_file_path": "/var/lib/nq.fixture/fixture.db"
}"#,
};

/// Fixture for `evaluate_nq_binary_mtime_state_preflight_at(conn, &NqBinaryMtimeStateTarget, now)`.
///
/// V0 targets a placeholder binary path. The evaluator returns
/// `InsufficientCoverage` for the placeholder; the probe checks
/// shape, not verdict.
pub const NQ_BINARY_MTIME_STATE_V1_MINIMAL: Fixture = Fixture {
    id: "nq_binary_mtime_state.v1.minimal",
    claim_kind: ClaimKind::NqBinaryMtimeState,
    canonical_json: r#"{
  "host": "nq.fixture.local",
  "binary_path": "/usr/local/bin/nq.fixture"
}"#,
};

/// Every V0 fixture in iteration order. The probe loop walks this
/// slice; per-kind adapters in `nq-monitor::nq_evaluator_probe`
/// dispatch on `claim_kind`.
///
/// Excludes `NqEvaluatorState` (self-witness collapse) and
/// `ComponentTestimonyObservationLoopAlive` (heartbeat shape;
/// deferred). Adding a new kind is a two-step edit: extend this
/// slice AND extend the per-kind adapter dispatch in nq-monitor.
pub const ALL_FIXTURES: &[Fixture] = &[
    DISK_STATE_V1_MINIMAL,
    INGEST_STATE_V1_MINIMAL,
    DNS_STATE_V1_MINIMAL,
    SQLITE_WAL_STATE_V1_MINIMAL,
    NQ_BINARY_MTIME_STATE_V1_MINIMAL,
];

/// Returns the V0 fixture for `claim_kind`, or `None` if no fixture
/// is defined. Callers MUST treat `None` as "kind not probeable in
/// V0" rather than "kind is fine"; the absence is structural, not
/// a verdict.
///
/// Returns `None` for `ClaimKind::NqEvaluatorState` (self-witness
/// collapse) and `ClaimKind::ComponentTestimonyObservationLoopAlive`
/// (V0 scope).
pub fn fixture_for(claim_kind: ClaimKind) -> Option<&'static Fixture> {
    ALL_FIXTURES.iter().find(|f| f.claim_kind == claim_kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_hash_is_sha256_prefixed_64_hex() {
        // Migration 056's structural CHECK pins the
        // "sha256:<64-hex>" shape. The fixture surface must
        // produce a hash compatible with that CHECK.
        for fixture in ALL_FIXTURES {
            let hash = fixture.hash();
            assert!(
                hash.starts_with("sha256:"),
                "fixture {} hash must start with sha256: prefix, got: {hash}",
                fixture.id
            );
            assert_eq!(
                hash.len(),
                71,
                "fixture {} hash must be 7 (\"sha256:\") + 64 hex chars, got len {}",
                fixture.id,
                hash.len()
            );
            assert!(
                hash[7..].chars().all(|c| c.is_ascii_hexdigit()),
                "fixture {} hash body must be hex, got: {}",
                fixture.id,
                &hash[7..]
            );
        }
    }

    #[test]
    fn fixture_hash_is_stable_across_calls() {
        // Content-addressed identity: two hash() calls on the same
        // fixture return identical strings. If this fails, something
        // non-deterministic crept into the hash path (timestamping,
        // RNG, etc.).
        for fixture in ALL_FIXTURES {
            assert_eq!(fixture.hash(), fixture.hash(), "fixture {}", fixture.id);
        }
    }

    #[test]
    fn fixture_hashes_are_distinct_across_fixtures() {
        // Two different fixtures must hash to different values —
        // otherwise an observation could not distinguish which
        // fixture produced it.
        let hashes: Vec<String> = ALL_FIXTURES.iter().map(|f| f.hash()).collect();
        for (i, h_i) in hashes.iter().enumerate() {
            for (j, h_j) in hashes.iter().enumerate().skip(i + 1) {
                assert_ne!(
                    h_i, h_j,
                    "fixtures {} and {} collide on hash {h_i}",
                    ALL_FIXTURES[i].id, ALL_FIXTURES[j].id
                );
            }
        }
    }

    #[test]
    fn fixture_ids_follow_kind_v_variant_convention() {
        // The id convention is `<kind>.v<version>.<variant>`. The
        // <kind> prefix must match the fixture's ClaimKind as_str()
        // form — otherwise a consumer reading the id cannot recover
        // the kind structurally.
        for fixture in ALL_FIXTURES {
            let kind_prefix = fixture.claim_kind.as_str();
            assert!(
                fixture.id.starts_with(kind_prefix),
                "fixture id {} must start with claim_kind prefix {}",
                fixture.id,
                kind_prefix
            );
            let rest = &fixture.id[kind_prefix.len()..];
            assert!(
                rest.starts_with(".v"),
                "fixture id {} must have .v<version> after kind, got rest: {rest}",
                fixture.id
            );
        }
    }

    #[test]
    fn fixture_canonical_json_parses_cleanly() {
        // The canonical_json is content-addressed but must remain
        // valid JSON — otherwise the probe's per-kind adapter
        // cannot recover the invocation parameters.
        for fixture in ALL_FIXTURES {
            let parsed: serde_json::Value = serde_json::from_str(fixture.canonical_json)
                .unwrap_or_else(|e| panic!("fixture {} JSON malformed: {e}", fixture.id));
            // Must be an object (per the V0 shape convention). The
            // ingest_state fixture is an empty object; the others
            // carry per-kind invocation tuples.
            assert!(
                parsed.is_object(),
                "fixture {} canonical_json must be a JSON object, got: {parsed}",
                fixture.id
            );
        }
    }

    #[test]
    fn fixture_for_returns_known_kinds_and_none_for_unmodeled_kinds() {
        // The five supported kinds must have fixtures.
        assert!(fixture_for(ClaimKind::DiskState).is_some());
        assert!(fixture_for(ClaimKind::IngestState).is_some());
        assert!(fixture_for(ClaimKind::DnsState).is_some());
        assert!(fixture_for(ClaimKind::SqliteWalState).is_some());
        assert!(fixture_for(ClaimKind::NqBinaryMtimeState).is_some());

        // The probe loop's V0 scope excludes these — keep them
        // structurally None so callers can't accidentally probe
        // their own kind (self-witness collapse) or the heartbeat
        // shape (deferred).
        assert!(
            fixture_for(ClaimKind::NqEvaluatorState).is_none(),
            "self-witness collapse refusal — preflight §2"
        );
        assert!(
            fixture_for(ClaimKind::ComponentTestimonyObservationLoopAlive).is_none(),
            "heartbeat shape deferred from V0 fixture surface"
        );
    }

    #[test]
    fn fixture_for_round_trips_through_claim_kind() {
        // Each returned fixture carries the requested claim_kind.
        // Loose-end check: a copy-paste error that ships a fixture
        // with the wrong claim_kind tag would silently misroute the
        // probe's per-kind dispatch.
        for fixture in ALL_FIXTURES {
            let looked_up = fixture_for(fixture.claim_kind)
                .expect("fixture lookup must succeed for its own kind");
            assert_eq!(looked_up.claim_kind, fixture.claim_kind);
            assert_eq!(looked_up.id, fixture.id);
        }
    }

    #[test]
    fn fixtures_use_placeholder_host_not_real_hostname() {
        // The "nq.fixture.local" placeholder appears in every
        // fixture that names a host. The probe substitutes the
        // real probe host at invocation time. Hard-coding a real
        // hostname here would couple the fixture's hash to a
        // specific deployment, defeating the canonical-fixture
        // discipline.
        for fixture in ALL_FIXTURES {
            if fixture.canonical_json.contains("host")
                || fixture.canonical_json.contains("vantage_host")
            {
                assert!(
                    fixture.canonical_json.contains("nq.fixture.local"),
                    "fixture {} mentions host but does not use the \
                     nq.fixture.local placeholder: {}",
                    fixture.id,
                    fixture.canonical_json
                );
            }
        }
    }
}
