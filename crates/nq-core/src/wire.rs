//! Compatibility surface for the monitor-owned `GET /state` wire.
//!
//! New collection code imports these types from `nq-monitor-check`. This
//! reexport remains while database and API consumers migrate. Removal
//! condition: no production package imports `nq_core::wire`.

pub use nq::{ClaimRefusal, RefusalKind};
pub use nq_monitor_check::wire::*;

#[cfg(test)]
mod decision_wire_compatibility_tests {
    use super::*;
    use serde_json::{from_value, json, to_value, Value};

    #[test]
    fn claim_refusal_serializes_with_snake_case_kind() {
        let refusal = ClaimRefusal::new(
            RefusalKind::ConsequenceClaim,
            "Whether to restart, reconfigure, or deactivate a failing source",
        );
        let serialized: Value = to_value(&refusal).expect("serialize");
        assert_eq!(
            serialized,
            json!({
                "refusal_kind": "consequence_claim",
                "statement": "Whether to restart, reconfigure, or deactivate a failing source"
            })
        );
    }

    #[test]
    fn every_refusal_kind_roundtrips() {
        let kinds = [
            RefusalKind::ConsequenceClaim,
            RefusalKind::FutureStateClaim,
            RefusalKind::SelfAuditRefusal,
            RefusalKind::OutOfJurisdiction,
            RefusalKind::AboveSubstrate,
            RefusalKind::BelowSubstrate,
            RefusalKind::EnvironmentalContext,
            RefusalKind::AbsenceSemantics,
            RefusalKind::CompositionReEmission,
            RefusalKind::KindSpecific,
        ];
        for kind in kinds {
            let serialized = serde_json::to_string(&kind).expect("serialize");
            let parsed: RefusalKind = serde_json::from_str(&serialized).expect("deserialize");
            assert_eq!(parsed, kind);
        }
    }

    #[test]
    fn claim_refusal_roundtrips_through_value() {
        let original = ClaimRefusal::new(RefusalKind::AboveSubstrate, "semantic correctness");
        let value = to_value(&original).expect("to_value");
        let back: ClaimRefusal = from_value(value).expect("from_value");
        assert_eq!(back, original);
    }
}
