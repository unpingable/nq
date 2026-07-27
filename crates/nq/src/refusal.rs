use serde::{Deserialize, Serialize};

/// One bounded refusal carried by an evaluator claim.
///
/// `refusal_kind` is stable machine identity. `statement` is explanatory
/// prose. Distinct statements with the same kind must remain distinct.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimRefusal {
    pub refusal_kind: RefusalKind,
    pub statement: String,
}

impl ClaimRefusal {
    /// Construct a refusal without strengthening its statement.
    pub fn new(refusal_kind: RefusalKind, statement: impl Into<String>) -> Self {
        Self {
            refusal_kind,
            statement: statement.into(),
        }
    }
}

impl std::fmt::Display for ClaimRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.statement)
    }
}

/// Frozen machine vocabulary for constitutional claim refusals.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RefusalKind {
    ConsequenceClaim,
    FutureStateClaim,
    SelfAuditRefusal,
    OutOfJurisdiction,
    AboveSubstrate,
    BelowSubstrate,
    EnvironmentalContext,
    AbsenceSemantics,
    CompositionReEmission,
    KindSpecific,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn refusal_wire_shape_is_frozen() {
        let refusal = ClaimRefusal::new(
            RefusalKind::ConsequenceClaim,
            "Whether to mutate the monitored system",
        );
        assert_eq!(
            serde_json::to_value(refusal).unwrap(),
            json!({
                "refusal_kind": "consequence_claim",
                "statement": "Whether to mutate the monitored system"
            })
        );
    }

    #[test]
    fn every_kind_keeps_its_wire_spelling() {
        let pairs = [
            (RefusalKind::ConsequenceClaim, "consequence_claim"),
            (RefusalKind::FutureStateClaim, "future_state_claim"),
            (RefusalKind::SelfAuditRefusal, "self_audit_refusal"),
            (RefusalKind::OutOfJurisdiction, "out_of_jurisdiction"),
            (RefusalKind::AboveSubstrate, "above_substrate"),
            (RefusalKind::BelowSubstrate, "below_substrate"),
            (RefusalKind::EnvironmentalContext, "environmental_context"),
            (RefusalKind::AbsenceSemantics, "absence_semantics"),
            (
                RefusalKind::CompositionReEmission,
                "composition_re_emission",
            ),
            (RefusalKind::KindSpecific, "kind_specific"),
        ];
        for (kind, spelling) in pairs {
            assert_eq!(
                serde_json::to_string(&kind).unwrap(),
                format!("\"{spelling}\"")
            );
        }
    }

    #[test]
    fn unknown_kind_is_refused() {
        assert!(serde_json::from_str::<RefusalKind>("\"generic\"").is_err());
    }
}
