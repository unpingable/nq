//! Transport-level tests for `nq-monitor reliance evaluate`.
//!
//! These test the *transport*, not the decision logic — the decision rules are
//! covered by `nq-core`'s reliance unit tests and golden vectors. What matters
//! here: strict decoding, the exit-code split between "decided no" and "could
//! not decide", stdout carrying JSON and nothing else in machine mode, and no
//! action/capability/authenticated token ever appearing in output.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use nq_core::receipt::{EvaluatorBinding, Receipt, Status, StatusReason, WitnessRef};
use nq_core::reliance::{CallerBinding, RelianceRequest, RELIANCE_REQUEST_SCHEMA};
use nq_core::wire::{ClaimRefusal, RefusalKind};

const NQ: &str = env!("CARGO_BIN_EXE_nq-monitor");
const NOW: &str = "2026-07-26T00:00:00Z";

fn profiles_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/reliance-profiles.json")
}

fn write(dir: &Path, name: &str, value: &impl serde::Serialize) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, serde_json::to_vec_pretty(value).unwrap()).unwrap();
    p
}

fn sealed(claim: &str, status: Status, reasons: Vec<StatusReason>, custody: &str) -> Receipt {
    let mut r = Receipt::new(claim, "attempt/1", NOW);
    r.status = status;
    r.status_reasons = reasons;
    r.witnesses = vec![WitnessRef {
        witness_type: "docket_dossier".into(),
        digest: Some("sha256:aa".into()),
        observed_at: Some(NOW.into()),
        custody_basis: Some(custody.into()),
    }];
    r.seal(EvaluatorBinding {
        evaluator: "claim_registry".into(),
        version: 1,
    })
    .unwrap();
    r
}

fn request(profile: &str, purpose: &str, claim: &str, receipt: &Receipt) -> RelianceRequest {
    RelianceRequest {
        schema: RELIANCE_REQUEST_SCHEMA.into(),
        consumer_profile_id: profile.into(),
        caller_binding: CallerBinding::Configured,
        purpose: purpose.into(),
        claim: claim.into(),
        receipt_content_hash: receipt.content_hash.clone().unwrap_or_default(),
        policy_version: "v1".into(),
        request_id: "req-transport".into(),
        supporting_receipt_hashes: vec![],
    }
}

fn run(dir: &Path, req: &RelianceRequest, rec: &Receipt) -> Output {
    let rp = write(dir, "request.json", req);
    let cp = write(dir, "receipt.json", rec);
    Command::new(NQ)
        .args(["reliance", "evaluate"])
        .arg("--request")
        .arg(&rp)
        .arg("--receipt")
        .arg(&cp)
        .arg("--profiles")
        .arg(profiles_path())
        .args(["--format", "json", "--generated-at", NOW])
        .output()
        .expect("run nq-monitor")
}

fn decision_of(out: &Output) -> String {
    assert_eq!(out.status.code(), Some(0), "expected a decision (exit 0)");
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("machine stdout must be exactly one JSON doc");
    v["decision"].as_str().unwrap().to_string()
}

#[test]
fn valid_reliance_for_the_nightshift_readonly_profile_is_authorized() {
    let d = tempfile::tempdir().unwrap();
    let rec = sealed(
        "docket_attempt_settled",
        Status::Verified,
        vec![],
        "native_observation",
    );
    let out = run(
        d.path(),
        &request(
            "nightshift-readonly",
            "continue_observing",
            "docket_attempt_settled",
            &rec,
        ),
        &rec,
    );
    assert_eq!(decision_of(&out), "authorized_reliance");
}

#[test]
fn binding_disclosure_survives_the_transport_and_never_says_authenticated() {
    let d = tempfile::tempdir().unwrap();
    let rec = sealed(
        "docket_attempt_settled",
        Status::Verified,
        vec![],
        "native_observation",
    );
    let out = run(
        d.path(),
        &request(
            "nightshift-readonly",
            "continue_observing",
            "docket_attempt_settled",
            &rec,
        ),
        &rec,
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["caller_binding"], "configured");
    assert!(v["caller_binding_disclosure"]
        .as_str()
        .unwrap()
        .contains("not an authenticated"));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(!text.contains("\"authenticated\""));
}

/// Every refusal is still a decision: exit 0, typed receipt on stdout.
#[test]
fn refusals_are_decisions_and_exit_zero() {
    let d = tempfile::tempdir().unwrap();
    let verified = sealed(
        "docket_attempt_settled",
        Status::Verified,
        vec![],
        "native_observation",
    );

    // unauthorized claim for this consumer
    let mut req = request(
        "nightshift-readonly",
        "continue_observing",
        "docket_attempt_settled",
        &verified,
    );
    req.claim = "nq_evaluator_state".into();
    let rec2 = {
        let mut r = sealed(
            "nq_evaluator_state",
            Status::Verified,
            vec![],
            "native_observation",
        );
        r.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        })
        .unwrap();
        r
    };
    req.receipt_content_hash = rec2.content_hash.clone().unwrap();
    assert_eq!(
        decision_of(&run(d.path(), &req, &rec2)),
        "claim_not_authorized_for_consumer"
    );

    // unauthorized purpose
    assert_eq!(
        decision_of(&run(
            d.path(),
            &request(
                "nightshift-readonly",
                "merge",
                "docket_attempt_settled",
                &verified
            ),
            &verified
        )),
        "purpose_not_authorized"
    );

    // unknown consumer
    assert_eq!(
        decision_of(&run(
            d.path(),
            &request(
                "no-such-consumer",
                "continue_observing",
                "docket_attempt_settled",
                &verified
            ),
            &verified
        )),
        "consumer_unknown"
    );

    // needs_more_evidence -> claim_not_verified, never a retry licence
    let nme = sealed(
        "docket_attempt_settled",
        Status::NeedsMoreEvidence,
        vec![],
        "native_observation",
    );
    assert_eq!(
        decision_of(&run(
            d.path(),
            &request(
                "nightshift-readonly",
                "continue_observing",
                "docket_attempt_settled",
                &nme
            ),
            &nme
        )),
        "claim_not_verified"
    );

    // cannot_testify is not success
    let ct = {
        let mut r = sealed(
            "docket_attempt_settled",
            Status::Verified,
            vec![],
            "native_observation",
        );
        r.cannot_testify = vec![ClaimRefusal::new(
            RefusalKind::ConsequenceClaim,
            "cannot testify to docket_attempt_settled as a consequence",
        )];
        r.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        })
        .unwrap();
        r
    };
    assert_eq!(
        decision_of(&run(
            d.path(),
            &request(
                "nightshift-readonly",
                "continue_observing",
                "docket_attempt_settled",
                &ct
            ),
            &ct
        )),
        "cannot_testify"
    );

    // custody basis outside the strict profile
    let ext = sealed(
        "docket_attempt_settled",
        Status::Verified,
        vec![],
        "external_projection",
    );
    assert_eq!(
        decision_of(&run(
            d.path(),
            &request(
                "nightshift-readonly",
                "continue_observing",
                "docket_attempt_settled",
                &ext
            ),
            &ext
        )),
        "custody_basis_not_accepted"
    );
}

#[test]
fn premise_contradiction_and_residual_refusals_survive_the_transport() {
    let d = tempfile::tempdir().unwrap();
    let rec = sealed(
        "docket_attempt_settled",
        Status::Verified,
        vec![],
        "native_observation",
    );
    let rp = write(
        d.path(),
        "request.json",
        &request(
            "nightshift-readonly",
            "wait",
            "docket_attempt_settled",
            &rec,
        ),
    );
    let cp = write(d.path(), "receipt.json", &rec);

    for (name, evidence, expected) in [
        (
            "premise",
            serde_json::json!({"premises":["clock_trusted"],"unenforceable_premises":["clock_trusted"]}),
            "premise_not_accepted",
        ),
        (
            "contradiction",
            serde_json::json!({"retained_contradictions":["A says committed, B says not"]}),
            "contradiction_retained",
        ),
        (
            "residual",
            serde_json::json!({"unresolved_residuals":["upstream review not discharged"]}),
            "residual_obligation_blocks",
        ),
        (
            "stale",
            serde_json::json!({"evidence_age_s": 100000}),
            "stale_evidence",
        ),
    ] {
        let ep = write(d.path(), &format!("evidence-{name}.json"), &evidence);
        let out = Command::new(NQ)
            .args(["reliance", "evaluate"])
            .arg("--request")
            .arg(&rp)
            .arg("--receipt")
            .arg(&cp)
            .arg("--evidence")
            .arg(&ep)
            .arg("--profiles")
            .arg(profiles_path())
            .args(["--format", "json", "--generated-at", NOW])
            .output()
            .unwrap();
        assert_eq!(decision_of(&out), expected, "{name}");
        // The carried facts are preserved, not summarised away.
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(v["does_not_establish"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d.as_str().unwrap().contains("grants no execution authority")));
    }
}

#[test]
fn substituted_receipt_under_the_same_request_refuses_as_a_decision() {
    let d = tempfile::tempdir().unwrap();
    let a = sealed(
        "docket_attempt_settled",
        Status::Verified,
        vec![],
        "native_observation",
    );
    let mut b = a.clone();
    b.subject = "attempt/2".into();
    b.seal(EvaluatorBinding {
        evaluator: "claim_registry".into(),
        version: 1,
    })
    .unwrap();
    let req = request(
        "nightshift-readonly",
        "continue_observing",
        "docket_attempt_settled",
        &a,
    );
    let out = run(d.path(), &req, &b);
    assert_eq!(decision_of(&out), "malformed_request");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["refusal_reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r.as_str().unwrap().contains("substituted")));
}

/// The load-bearing distinction: undecodable input exits 1 with **no stdout**,
/// so a caller can never mistake a tool failure for testimony.
#[test]
fn undecodable_input_exits_one_with_empty_stdout() {
    let d = tempfile::tempdir().unwrap();
    let rec = sealed(
        "docket_attempt_settled",
        Status::Verified,
        vec![],
        "native_observation",
    );
    let cp = write(d.path(), "receipt.json", &rec);

    // malformed request JSON
    let bad = d.path().join("bad.json");
    std::fs::write(&bad, b"{not json").unwrap();
    let out = Command::new(NQ)
        .args(["reliance", "evaluate"])
        .arg("--request")
        .arg(&bad)
        .arg("--receipt")
        .arg(&cp)
        .arg("--profiles")
        .arg(profiles_path())
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty(), "no decision means no stdout");
    assert!(!out.stderr.is_empty(), "diagnostics belong on stderr");

    // unsupported request schema is *also* undecodable at this boundary only
    // if the shape differs; a wrong schema string decodes and becomes a typed
    // malformed_request decision instead.
    let mut wrong = request(
        "nightshift-readonly",
        "continue_observing",
        "docket_attempt_settled",
        &rec,
    );
    wrong.schema = "nq.reliance.request.v99".into();
    assert_eq!(decision_of(&run(d.path(), &wrong, &rec)), "malformed_request");

    // missing file
    let out = Command::new(NQ)
        .args(["reliance", "evaluate"])
        .arg("--request")
        .arg(d.path().join("absent.json"))
        .arg("--receipt")
        .arg(&cp)
        .arg("--profiles")
        .arg(profiles_path())
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());

    // unusable profile catalog
    let badcat = d.path().join("cat.json");
    std::fs::write(
        &badcat,
        br#"{"schema":"nq.reliance.profiles.v99","policy_version":"v1","profiles":[]}"#,
    )
    .unwrap();
    let rp = write(
        d.path(),
        "request.json",
        &request(
            "nightshift-readonly",
            "continue_observing",
            "docket_attempt_settled",
            &rec,
        ),
    );
    let out = Command::new(NQ)
        .args(["reliance", "evaluate"])
        .arg("--request")
        .arg(&rp)
        .arg("--receipt")
        .arg(&cp)
        .arg("--profiles")
        .arg(&badcat)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
}

#[test]
fn exact_duplicate_requests_are_idempotent_over_the_transport() {
    let d = tempfile::tempdir().unwrap();
    let rec = sealed(
        "docket_attempt_settled",
        Status::Verified,
        vec![],
        "native_observation",
    );
    let req = request(
        "nightshift-readonly",
        "continue_observing",
        "docket_attempt_settled",
        &rec,
    );
    let a = run(d.path(), &req, &rec);
    let b = run(d.path(), &req, &rec);
    assert_eq!(a.stdout, b.stdout, "pinned generated_at ⇒ identical bytes");
}

#[test]
fn machine_stdout_is_json_only_and_carries_no_action_or_capability() {
    let d = tempfile::tempdir().unwrap();
    let rec = sealed(
        "docket_attempt_settled",
        Status::Verified,
        vec![],
        "native_observation",
    );
    let out = run(
        d.path(),
        &request(
            "nightshift-readonly",
            "continue_observing",
            "docket_attempt_settled",
            &rec,
        ),
        &rec,
    );
    let text = String::from_utf8(out.stdout).unwrap();
    // Exactly one JSON document, no prose framing.
    assert!(text.trim_start().starts_with('{'));
    assert!(text.trim_end().ends_with('}'));
    serde_json::from_str::<serde_json::Value>(text.trim()).expect("stdout parses as one JSON doc");
    for forbidden in [
        "\"action\"",
        "\"capability\"",
        "\"grant\"",
        "\"lease\"",
        "\"execute\"",
    ] {
        assert!(!text.contains(forbidden), "stdout must not carry {forbidden}");
    }
}

#[test]
fn stdin_is_accepted_for_the_request() {
    use std::io::Write as _;
    use std::process::Stdio;
    let d = tempfile::tempdir().unwrap();
    let rec = sealed(
        "docket_attempt_settled",
        Status::Verified,
        vec![],
        "native_observation",
    );
    let cp = write(d.path(), "receipt.json", &rec);
    let req = request(
        "nightshift-readonly",
        "continue_observing",
        "docket_attempt_settled",
        &rec,
    );
    let mut child = Command::new(NQ)
        .args(["reliance", "evaluate", "--request", "-"])
        .arg("--receipt")
        .arg(&cp)
        .arg("--profiles")
        .arg(profiles_path())
        .args(["--format", "json", "--generated-at", NOW])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(&serde_json::to_vec(&req).unwrap())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(decision_of(&out), "authorized_reliance");
}
