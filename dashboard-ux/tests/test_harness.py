import copy
import importlib.util
import json
import unittest
from pathlib import Path


HARNESS_PATH = Path(__file__).resolve().parents[1] / "harness.py"
SPEC = importlib.util.spec_from_file_location("nq_dashboard_ux_harness", HARNESS_PATH)
HARNESS = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(HARNESS)
RAW_SCREENSHOT = (
    HARNESS.REPO
    / "docs/dashboard/campaign/raw/baseline/missing-finding/page.png"
)
OPERATOR_GLOSSARY = HARNESS.REPO / "docs/operator/GLOSSARY.md"


def stored_result():
    return HARNESS.load_json(
        HARNESS.RESULTS / "baseline/missing-finding-production-sre.json"
    )


def result_indexes():
    personas, scenarios = HARNESS.load_corpus()
    return HARNESS.by_id(personas, "persona"), HARNESS.by_id(scenarios, "scenario")


class HarnessTests(unittest.TestCase):
    def test_complete_corpus_validates_offline(self):
        self.assertEqual(HARNESS.validate_all(), [])

    def test_perfect_mock_scores_all_twenty(self):
        _, scenarios = HARNESS.load_corpus()
        self.assertEqual(len(scenarios), 20)
        for scenario in scenarios:
            score = HARNESS.perfect_mock_result(scenario)
            self.assertTrue(score["passed"], scenario["id"])
            self.assertEqual(score["percent"], 100.0)

    def test_prompt_package_excludes_oracle_and_is_deterministic(self):
        first = HARNESS.build_package(
            "production-sre-outage",
            "error-rate-increased",
            "redesign",
            "http://127.0.0.1:9848",
            [str(RAW_SCREENSHOT)],
            [],
        )
        second = HARNESS.build_package(
            "production-sre-outage",
            "error-rate-increased",
            "redesign",
            "http://127.0.0.1:9848",
            [str(RAW_SCREENSHOT)],
            [],
        )
        self.assertEqual(first, second)
        serialized = json.dumps(first)
        self.assertNotIn("forbidden_conclusions", serialized)
        self.assertNotIn("required_conclusion_ids", serialized)
        self.assertNotIn("ux-error-shift-001", serialized)

    def test_unsafe_action_and_false_cause_fail_deterministically(self):
        scenario = HARNESS.scenario_map()["error-rate-increased"]
        oracle = scenario["oracle"]
        result = {
            "coding": {
                "subject_ids": oracle["required_subject_ids"],
                "conclusion_ids": oracle["required_conclusion_ids"],
                "evidence_ids": oracle["required_evidence_ids"],
                "unknown_ids": oracle["required_unknown_ids"],
                "chosen_action_id": "rollback-deployment",
                "proposed_action_ids": ["rollback-deployment"],
                "novel_action_proposals": [],
                "causality_claimed": True,
                "stale_data_noticed": False,
                "contradictory_data_noticed": False,
            }
        }
        score = HARNESS.compute_score(result, scenario)
        self.assertFalse(score["passed"])
        self.assertFalse(score["dimensions"]["action"])
        self.assertFalse(score["dimensions"]["causality"])

    def test_adapter_generation_never_executes(self):
        package = HARNESS.build_package(
            "sleep-deprived-0317",
            "missing-finding-route",
            "baseline",
            None,
            [str(RAW_SCREENSHOT)],
            [],
        )
        for family, model in (("codex", "gpt-5.6-terra"), ("claude", "sonnet")):
            spec = HARNESS.adapter_spec(family, package, model, "/tmp/run")
            self.assertFalse(spec["executed_by_harness"])
            self.assertTrue(spec["network_required_for_execution"])
            self.assertTrue(spec["validation_is_offline"])

    def test_canonical_scenario_mapping_rejects_replacement_theme(self):
        personas, scenarios = HARNESS.load_corpus()
        scenarios = copy.deepcopy(scenarios)
        scenarios[0]["id"] = "unrelated-but-schema-valid"
        errors = HARNESS.semantic_errors(personas, scenarios)
        self.assertTrue(
            any("canonical 1..20 corpus" in error for error in errors),
            errors,
        )

    def test_result_provenance_must_match_synthetic_fixture_only_scenario(self):
        personas, scenarios = result_indexes()
        for field, value in (
            ("fixture_provenance", "live_observed"),
            ("real_path_coverage", "real_route_with_synthetic_database"),
        ):
            with self.subTest(field=field):
                result = stored_result()
                result[field] = value
                errors = HARNESS.validate_result_semantics(
                    result, personas, scenarios
                )
                self.assertTrue(
                    any(field.replace("_", "-") in error or field.split("_")[0] in error
                        for error in errors),
                    errors,
                )

    def test_result_identity_is_bound_to_persona_archetype_and_scenario_version(self):
        personas, scenarios = result_indexes()
        mutations = (
            ("persona_id", "not-a-persona", "unknown persona"),
            ("operator_archetype", "database_operator", "archetype"),
            ("scenario_version", 999, "scenario version"),
        )
        for field, value, expected in mutations:
            with self.subTest(field=field):
                result = stored_result()
                result[field] = value
                errors = HARNESS.validate_result_semantics(
                    result, personas, scenarios
                )
                self.assertTrue(any(expected in error for error in errors), errors)

    def test_package_paths_are_confined_to_raw_and_operator_docs(self):
        package = HARNESS.build_package(
            "production-sre-outage",
            "error-rate-increased",
            "redesign",
            None,
            [str(RAW_SCREENSHOT)],
            [str(OPERATOR_GLOSSARY)],
        )
        self.assertEqual(package["artifacts"], [str(RAW_SCREENSHOT.resolve())])
        self.assertEqual(package["operator_docs"], [str(OPERATOR_GLOSSARY.resolve())])
        rejected = (
            ([str(HARNESS.REPO / "Cargo.toml")], []),
            (["docs/dashboard/campaign/raw/../CURRENT_STATE_ARCHAEOLOGY.md"], []),
            ([str(RAW_SCREENSHOT)], [str(HARNESS.REPO / "Cargo.toml")]),
            ([str(RAW_SCREENSHOT)], ["docs/operator/../dashboard/SYNTHETIC_UX_METHOD.md"]),
        )
        for artifacts, docs in rejected:
            with self.subTest(artifacts=artifacts, docs=docs):
                with self.assertRaises(HARNESS.ValidationError):
                    HARNESS.build_package(
                        "production-sre-outage",
                        "error-rate-increased",
                        "redesign",
                        None,
                        artifacts,
                        docs,
                    )

    def test_result_artifacts_are_relative_confined_and_hashed(self):
        personas, scenarios = result_indexes()
        for path in (str(RAW_SCREENSHOT), "../outside.png", "Cargo.toml"):
            with self.subTest(path=path):
                result = stored_result()
                result["raw_artifacts"][1]["path"] = path
                errors = HARNESS.validate_result_semantics(
                    result, personas, scenarios
                )
                self.assertTrue(
                    any("raw artifact" in error for error in errors),
                    errors,
                )

    def test_full_operator_response_is_required_and_cross_checked(self):
        personas, scenarios = result_indexes()
        schema = HARNESS.load_json(HARNESS.SCHEMAS / "run-result.schema.json")
        result = stored_result()
        del result["operator_response"]["freshness"]
        errors = HARNESS.validate_schema(result, schema)
        self.assertTrue(any("freshness" in error for error in errors), errors)

        result = stored_result()
        result["operator_response"]["final_confidence"] = 0.1
        errors = HARNESS.validate_result_semantics(result, personas, scenarios)
        self.assertTrue(any("confidence" in error for error in errors), errors)

        result = stored_result()
        result["operator_response"]["causality_claimed"] = True
        errors = HARNESS.validate_result_semantics(result, personas, scenarios)
        self.assertTrue(any("causality" in error for error in errors), errors)

    def test_action_fixture_requires_all_ten_semantic_answers(self):
        _, scenarios = HARNESS.load_corpus()
        schema = HARNESS.load_json(HARNESS.SCHEMAS / "scenario-corpus.schema.json")
        action = scenarios[0]["visible_state"]["actions"][0]
        expected = {
            "target",
            "state_transition",
            "does_not",
            "underlying_system_affected",
            "evidence_policy",
            "detector_observation",
            "reversible",
            "precondition",
            "future_observations",
            "recording",
        }
        self.assertTrue(expected <= set(action))
        broken = copy.deepcopy(scenarios)
        del broken[0]["visible_state"]["actions"][0]["recording"]
        errors = HARNESS.validate_schema(broken, schema)
        self.assertTrue(any("recording" in error for error in errors), errors)

    def test_known_novel_and_unknown_actions_fail_closed(self):
        personas, scenarios = result_indexes()
        scenario = scenarios["missing-finding-route"]

        result = stored_result()
        result["coding"]["proposed_action_ids"].append("close-missing")
        result["score"] = HARNESS.compute_score(result, scenario)
        errors = HARNESS.validate_result_semantics(result, personas, scenarios)
        self.assertTrue(any("unsafe_actions_proposed" in error for error in errors))
        result["metrics"]["action_choice_correct"] = False
        result["metrics"]["unsafe_actions_proposed"] = ["close-missing"]
        self.assertEqual(
            HARNESS.validate_result_semantics(result, personas, scenarios), []
        )
        self.assertFalse(result["score"]["dimensions"]["action"])

        result = stored_result()
        result["coding"]["novel_action_proposals"] = [
            {"proposal": "restart the service", "safety": "unsafe"}
        ]
        result["score"] = HARNESS.compute_score(result, scenario)
        result["metrics"]["action_choice_correct"] = False
        result["metrics"]["unsafe_actions_proposed"] = ["restart the service"]
        self.assertEqual(
            HARNESS.validate_result_semantics(result, personas, scenarios), []
        )
        self.assertFalse(result["score"]["dimensions"]["action"])

        result = stored_result()
        result["coding"]["novel_action_proposals"] = [
            {"proposal": "run an unreviewed recovery command", "safety": "unknown"}
        ]
        result["score"] = HARNESS.compute_score(result, scenario)
        result["metrics"]["action_choice_correct"] = False
        self.assertEqual(
            HARNESS.validate_result_semantics(result, personas, scenarios), []
        )
        self.assertFalse(result["score"]["dimensions"]["action"])

        result = stored_result()
        result["coding"]["proposed_action_ids"].append("invented-action-id")
        result["score"] = HARNESS.compute_score(result, scenario)
        result["metrics"]["action_choice_correct"] = False
        errors = HARNESS.validate_result_semantics(result, personas, scenarios)
        self.assertTrue(any("not declared by scenario" in error for error in errors))

    def test_failure_records_must_reference_known_runs_scenarios_and_artifacts(self):
        _, scenarios = result_indexes()
        result_a = stored_result()
        result_b = HARNESS.load_json(
            HARNESS.RESULTS / "baseline/missing-finding-sleep-deprived.json"
        )
        results = {result_a["run_id"]: result_a, result_b["run_id"]: result_b}
        record = {
            "failure_id": "UXF-0001",
            "scenario_ids": ["missing-finding-route"],
            "run_ids": [result_a["run_id"], result_b["run_id"]],
            "reproduced_by_multiple_fresh_operators": True,
            "evidence_paths": [
                "docs/dashboard/campaign/raw/baseline/missing-finding/page.png"
            ],
        }
        self.assertEqual(
            HARNESS.validate_failure_semantics(record, scenarios, results), []
        )

        mutations = (
            ("scenario_ids", ["unknown-scenario"], "unknown scenario"),
            (
                "scenario_ids",
                ["error-rate-increased"],
                "scenario is not listed",
            ),
            ("run_ids", ["unknown-run"], "unknown run"),
            (
                "evidence_paths",
                ["docs/dashboard/campaign/raw/../CURRENT_STATE_ARCHAEOLOGY.md"],
                "failure evidence",
            ),
        )
        for field, value, expected in mutations:
            with self.subTest(field=field):
                broken = copy.deepcopy(record)
                broken[field] = value
                errors = HARNESS.validate_failure_semantics(
                    broken, scenarios, results
                )
                self.assertTrue(any(expected in error for error in errors), errors)

        broken = copy.deepcopy(record)
        broken["run_ids"] = [result_a["run_id"]]
        errors = HARNESS.validate_failure_semantics(broken, scenarios, results)
        self.assertTrue(any("at least two runs" in error for error in errors), errors)


if __name__ == "__main__":
    unittest.main()
