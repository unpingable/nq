#!/usr/bin/env python3
"""Offline-first corpus validator, prompt packager, adapter builder, and scorer."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any


BASE = Path(__file__).resolve().parent
REPO = BASE.parent
SCHEMAS = BASE / "schemas"
CORPUS = BASE / "corpus"
RESULTS = BASE / "results"
RAW_EVIDENCE_ROOT = REPO / "docs" / "dashboard" / "campaign" / "raw"
OPERATOR_DOCS_ROOT = REPO / "docs" / "operator"

CANONICAL_SCENARIOS = {
    1: "error-rate-increased",
    2: "disk-pressure-conflicting-views",
    3: "database-reclaimable-space",
    4: "current-finding-impact-incomplete",
    5: "stale-finding",
    6: "missing-finding-route",
    7: "resolved-finding-history",
    8: "nq-self-health-failure",
    9: "sources-disagree",
    10: "change-detected-cause-unknown",
    11: "low-sample-size",
    12: "no-current-issue",
    13: "multiple-issues-different-urgency",
    14: "severe-classification-low-impact",
    15: "mild-metric-immediate-evidence",
    16: "suppression-versus-resolution",
    17: "reset-semantics",
    18: "old-observation-current-inventory",
    19: "compare-two-periods",
    20: "safe-to-ignore",
}


class ValidationError(Exception):
    pass


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ValidationError(f"{path}: {exc}") from exc


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def confined_file(
    path_text: str,
    root: Path,
    label: str,
    *,
    require_relative: bool = False,
) -> Path:
    """Resolve an existing file without absolute/path-traversal escape."""
    supplied = Path(path_text)
    if ".." in supplied.parts:
        raise ValidationError(f"{label} path may not contain '..': {path_text}")
    if require_relative and supplied.is_absolute():
        raise ValidationError(f"{label} path must be repository-relative: {path_text}")
    candidate = supplied if supplied.is_absolute() else REPO / supplied
    try:
        resolved = candidate.resolve(strict=True)
    except OSError as exc:
        raise ValidationError(f"{label} file does not exist: {path_text}") from exc
    try:
        resolved.relative_to(root.resolve(strict=True))
    except (OSError, ValueError) as exc:
        raise ValidationError(f"{label} path escapes {root.relative_to(REPO)}: {path_text}") from exc
    if not resolved.is_file():
        raise ValidationError(f"{label} path is not a file: {path_text}")
    return resolved


def type_matches(value: Any, expected: str) -> bool:
    if expected == "object":
        return isinstance(value, dict)
    if expected == "array":
        return isinstance(value, list)
    if expected == "string":
        return isinstance(value, str)
    if expected == "boolean":
        return isinstance(value, bool)
    if expected == "integer":
        return isinstance(value, int) and not isinstance(value, bool)
    if expected == "number":
        return isinstance(value, (int, float)) and not isinstance(value, bool)
    if expected == "null":
        return value is None
    raise ValidationError(f"unsupported schema type {expected!r}")


def validate_schema(value: Any, schema: dict[str, Any], path: str = "$") -> list[str]:
    """Validate the deliberately small JSON-Schema subset used by this harness."""
    errors: list[str] = []
    expected = schema.get("type")
    if expected and not type_matches(value, expected):
        return [f"{path}: expected {expected}, got {type(value).__name__}"]
    if "const" in schema and value != schema["const"]:
        errors.append(f"{path}: expected constant {schema['const']!r}")
    if "enum" in schema and value not in schema["enum"]:
        errors.append(f"{path}: {value!r} not in enum {schema['enum']!r}")
    if isinstance(value, str):
        if len(value) < schema.get("minLength", 0):
            errors.append(f"{path}: string shorter than minLength")
        pattern = schema.get("pattern")
        if pattern and re.search(pattern, value) is None:
            errors.append(f"{path}: {value!r} does not match {pattern!r}")
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        if "minimum" in schema and value < schema["minimum"]:
            errors.append(f"{path}: {value} below minimum {schema['minimum']}")
        if "maximum" in schema and value > schema["maximum"]:
            errors.append(f"{path}: {value} above maximum {schema['maximum']}")
    if isinstance(value, list):
        if len(value) < schema.get("minItems", 0):
            errors.append(f"{path}: fewer than minItems")
        if "maxItems" in schema and len(value) > schema["maxItems"]:
            errors.append(f"{path}: more than maxItems")
        item_schema = schema.get("items")
        if item_schema:
            for index, item in enumerate(value):
                errors.extend(validate_schema(item, item_schema, f"{path}[{index}]"))
    if isinstance(value, dict):
        required = schema.get("required", [])
        for key in required:
            if key not in value:
                errors.append(f"{path}: missing required property {key!r}")
        properties = schema.get("properties", {})
        if schema.get("additionalProperties") is False:
            for key in value:
                if key not in properties:
                    errors.append(f"{path}: unexpected property {key!r}")
        for key, child_schema in properties.items():
            if key in value:
                errors.extend(validate_schema(value[key], child_schema, f"{path}.{key}"))
    return errors


def by_id(records: list[dict[str, Any]], label: str) -> dict[str, dict[str, Any]]:
    indexed: dict[str, dict[str, Any]] = {}
    for record in records:
        record_id = record["id"]
        if record_id in indexed:
            raise ValidationError(f"duplicate {label} id {record_id!r}")
        indexed[record_id] = record
    return indexed


def load_corpus() -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    personas = load_json(CORPUS / "personas.json")
    scenarios = load_json(CORPUS / "scenarios.json")
    return personas, scenarios


def semantic_errors(
    personas: list[dict[str, Any]], scenarios: list[dict[str, Any]]
) -> list[str]:
    errors: list[str] = []
    try:
        by_id(personas, "persona")
        by_id(scenarios, "scenario")
    except ValidationError as exc:
        errors.append(str(exc))
    required_archetypes = {
        "production_sre",
        "devops_engineer",
        "linux_systems_administrator",
        "database_operator",
        "junior_on_call",
        "incident_commander",
        "security_conscious_operator",
        "sleep_deprived_operator",
        "skeptical_evaluator",
        "domain_expert_unfamiliar_with_nq",
    }
    actual_archetypes = {p["archetype"] for p in personas}
    if actual_archetypes != required_archetypes:
        errors.append("persona archetypes do not exactly match the required ten")
    requirement_map = {s["requirement"]: s["id"] for s in scenarios}
    if requirement_map != CANONICAL_SCENARIOS:
        errors.append(
            "scenario requirement-to-theme mapping does not match the canonical 1..20 corpus"
        )
    for scenario in scenarios:
        sid = scenario["id"]
        if scenario["fixture"]["provenance"] != "synthetic":
            errors.append(f"{sid}: corpus fixtures must be explicitly synthetic")
        if scenario["fixture"]["real_path_coverage"] != "fixture_only":
            errors.append(f"{sid}: may not claim live, route, or publish-path coverage")
        if set(scenario["variants"]) != {"baseline", "redesign", "text"}:
            errors.append(f"{sid}: must support baseline/redesign/text variants")
        basis_ids = {b["id"] for b in scenario["fixture"]["observation_bases"]}
        evidence_ids = {e["id"] for e in scenario["visible_state"]["evidence"]}
        unknown_ids = {u["id"] for u in scenario["visible_state"]["unknowns"]}
        conflict_ids = {c["id"] for c in scenario["visible_state"]["conflicts"]}
        action_ids = {a["id"] for a in scenario["visible_state"]["actions"]}
        available_action_ids = {
            a["id"] for a in scenario["visible_state"]["actions"] if a["available"]
        }
        claim_ids = {c["id"] for c in scenario["visible_state"]["decision_claims"]}
        allowed_ids = {c["id"] for c in scenario["oracle"]["allowed_conclusions"]}
        for evidence in scenario["visible_state"]["evidence"]:
            if evidence["basis_id"] not in basis_ids:
                errors.append(f"{sid}: evidence {evidence['id']} references unknown basis")
        if not set(scenario["oracle"]["required_evidence_ids"]) <= evidence_ids:
            errors.append(f"{sid}: oracle references unknown evidence")
        if not set(scenario["oracle"]["required_unknown_ids"]) <= unknown_ids:
            errors.append(f"{sid}: oracle references unknown unknown-state id")
        if not set(scenario["oracle"]["required_conclusion_ids"]) <= (claim_ids | allowed_ids):
            errors.append(f"{sid}: oracle references unknown conclusion")
        if not set(scenario["oracle"]["safe_action_ids"]) <= action_ids:
            errors.append(f"{sid}: safe action is not rendered in visible_state")
        if not set(scenario["oracle"]["safe_action_ids"]) <= available_action_ids:
            errors.append(f"{sid}: safe action is rendered unavailable")
        if scenario["oracle"]["must_notice_conflict"] and not conflict_ids:
            errors.append(f"{sid}: conflict notice required but no conflict fixture exists")
        if scenario["oracle"]["must_notice_stale"] and not any(
            basis["tense"] in {"stale", "historical"}
            for basis in scenario["fixture"]["observation_bases"]
        ):
            errors.append(f"{sid}: stale notice required but no aged basis exists")
    return errors


def scenario_map() -> dict[str, dict[str, Any]]:
    return by_id(load_corpus()[1], "scenario")


def compute_score(result: dict[str, Any], scenario: dict[str, Any]) -> dict[str, Any]:
    coding = result["coding"]
    oracle = scenario["oracle"]
    forbidden = {item["id"] for item in oracle["forbidden_conclusions"]}
    unsafe = set(oracle["unsafe_action_ids"])
    novel_actions_safe = all(
        proposal["safety"] == "safe"
        for proposal in coding["novel_action_proposals"]
    )
    causality_ok = (
        coding["causality_claimed"]
        if oracle["causality"] == "established"
        else not coding["causality_claimed"]
    )
    dimensions = {
        "subject": set(oracle["required_subject_ids"]) <= set(coding["subject_ids"]),
        "conclusion": (
            set(oracle["required_conclusion_ids"]) <= set(coding["conclusion_ids"])
            and not (forbidden & set(coding["conclusion_ids"]))
        ),
        "evidence": set(oracle["required_evidence_ids"]) <= set(coding["evidence_ids"]),
        "unknowns": set(oracle["required_unknown_ids"]) <= set(coding["unknown_ids"]),
        "action": (
            coding["chosen_action_id"] in set(oracle["safe_action_ids"])
            and not (unsafe & set(coding["proposed_action_ids"]))
            and novel_actions_safe
        ),
        "causality": causality_ok,
        "stale": (
            coding["stale_data_noticed"]
            if oracle["must_notice_stale"]
            else True
        ),
        "conflict": (
            coding["contradictory_data_noticed"]
            if oracle["must_notice_conflict"]
            else True
        ),
    }
    points = sum(1 for passed in dimensions.values() if passed)
    possible = len(dimensions)
    return {
        "oracle_version": scenario["version"],
        "passed": all(dimensions.values()),
        "points": points,
        "possible": possible,
        "percent": round(points * 100.0 / possible, 1),
        "dimensions": dimensions,
    }


def expected_unsafe_actions(
    result: dict[str, Any], scenario: dict[str, Any]
) -> list[str]:
    unsafe_ids = set(scenario["oracle"]["unsafe_action_ids"])
    declared = unsafe_ids & set(result["coding"]["proposed_action_ids"])
    novel = {
        proposal["proposal"]
        for proposal in result["coding"]["novel_action_proposals"]
        if proposal["safety"] == "unsafe"
    }
    return sorted(declared | novel)


def expected_metrics(
    result: dict[str, Any], score: dict[str, Any], scenario: dict[str, Any]
) -> dict[str, Any]:
    coding = result["coding"]
    return {
        "operational_conclusion_correct": (
            score["dimensions"]["subject"] and score["dimensions"]["conclusion"]
        ),
        "action_choice_correct": score["dimensions"]["action"],
        "uncertainty_preserved": score["dimensions"]["unknowns"],
        "causality_falsely_inferred": not score["dimensions"]["causality"],
        "stale_data_noticed": coding["stale_data_noticed"],
        "contradictory_data_noticed": coding["contradictory_data_noticed"],
        "unsafe_actions_proposed": expected_unsafe_actions(result, scenario),
    }


def validate_result_semantics(
    result: dict[str, Any],
    personas: dict[str, dict[str, Any]],
    scenarios: dict[str, dict[str, Any]],
) -> list[str]:
    errors: list[str] = []
    sid = result["scenario_id"]
    if sid not in scenarios:
        return [f"{result['run_id']}: unknown scenario {sid!r}"]
    scenario = scenarios[sid]
    persona_id = result["persona_id"]
    if persona_id not in personas:
        errors.append(f"{result['run_id']}: unknown persona {persona_id!r}")
    elif result["operator_archetype"] != personas[persona_id]["archetype"]:
        errors.append(f"{result['run_id']}: operator archetype does not match persona")
    if result["scenario_version"] != scenario["version"]:
        errors.append(f"{result['run_id']}: scenario version does not match corpus")
    if result["fixture_provenance"] != scenario["fixture"]["provenance"]:
        errors.append(f"{result['run_id']}: fixture provenance does not match scenario")
    if result["real_path_coverage"] != scenario["fixture"]["real_path_coverage"]:
        errors.append(f"{result['run_id']}: real-path coverage does not match scenario")
    if result["ui_variant"] not in scenario["variants"]:
        errors.append(f"{result['run_id']}: UI variant is not supported by scenario")

    visible_action_ids = {
        action["id"] for action in scenario["visible_state"]["actions"]
    }
    known_action_ids = visible_action_ids | set(scenario["oracle"]["unsafe_action_ids"])
    unknown_proposed = set(result["coding"]["proposed_action_ids"]) - known_action_ids
    if unknown_proposed:
        errors.append(
            f"{result['run_id']}: proposed action IDs are not declared by scenario: "
            + ", ".join(sorted(unknown_proposed))
        )
    duplicate_novel = [
        proposal["proposal"] for proposal in result["coding"]["novel_action_proposals"]
    ]
    if len(duplicate_novel) != len(set(duplicate_novel)):
        errors.append(f"{result['run_id']}: duplicate novel action proposal")

    conclusion_ids = (
        {claim["id"] for claim in scenario["visible_state"]["decision_claims"]}
        | {claim["id"] for claim in scenario["oracle"]["allowed_conclusions"]}
        | {claim["id"] for claim in scenario["oracle"]["forbidden_conclusions"]}
    )
    unknown_conclusions = set(result["coding"]["conclusion_ids"]) - conclusion_ids
    if unknown_conclusions:
        errors.append(
            f"{result['run_id']}: conclusion IDs are not declared by scenario: "
            + ", ".join(sorted(unknown_conclusions))
        )

    score = compute_score(result, scenario)
    if result["score"] != score:
        errors.append(f"{result['run_id']}: stored score does not match deterministic score")
    for key, value in expected_metrics(result, score, scenario).items():
        if result["metrics"][key] != value:
            errors.append(f"{result['run_id']}: metrics.{key} does not match coded result")
    response_errors = validate_schema(
        result["operator_response"],
        load_json(SCHEMAS / "operator-response.schema.json"),
    )
    errors.extend(
        f"{result['run_id']}: operator_response {error}" for error in response_errors
    )
    if (
        result["operator_response"]["final_confidence"]
        != result["metrics"]["final_confidence"]
    ):
        errors.append(
            f"{result['run_id']}: response confidence does not match metrics"
        )
    if (
        result["operator_response"]["causality_claimed"]
        != result["coding"]["causality_claimed"]
    ):
        errors.append(
            f"{result['run_id']}: response causality does not match coded result"
        )
    for artifact in result["raw_artifacts"]:
        try:
            path = confined_file(
                artifact["path"],
                RAW_EVIDENCE_ROOT,
                "raw artifact",
                require_relative=True,
            )
        except ValidationError as exc:
            errors.append(f"{result['run_id']}: {exc}")
            continue
        if sha256_file(path) != artifact["sha256"]:
            errors.append(f"{result['run_id']}: digest mismatch for {artifact['path']}")
    return errors


def validate_failure_semantics(
    record: dict[str, Any],
    scenarios: dict[str, dict[str, Any]],
    results: dict[str, dict[str, Any]],
) -> list[str]:
    errors: list[str] = []
    failure_id = record["failure_id"]
    unknown_scenarios = set(record["scenario_ids"]) - set(scenarios)
    if unknown_scenarios:
        errors.append(
            f"{failure_id}: unknown scenario references: "
            + ", ".join(sorted(unknown_scenarios))
        )
    unknown_runs = set(record["run_ids"]) - set(results)
    if unknown_runs:
        errors.append(
            f"{failure_id}: unknown run references: " + ", ".join(sorted(unknown_runs))
        )
    known_runs = [results[run_id] for run_id in record["run_ids"] if run_id in results]
    for result in known_runs:
        if result["scenario_id"] not in record["scenario_ids"]:
            errors.append(
                f"{failure_id}: run {result['run_id']} scenario is not listed"
            )
    if record["reproduced_by_multiple_fresh_operators"]:
        if len({result["run_id"] for result in known_runs}) < 2:
            errors.append(f"{failure_id}: repeated failure requires at least two runs")
        if any(not result["execution"]["fresh_context"] for result in known_runs):
            errors.append(f"{failure_id}: repeated failure includes a non-fresh run")
    referenced_artifacts = {
        artifact["path"]
        for result in known_runs
        for artifact in result["raw_artifacts"]
    }
    for evidence_path in record["evidence_paths"]:
        try:
            confined_file(
                evidence_path,
                RAW_EVIDENCE_ROOT,
                "failure evidence",
                require_relative=True,
            )
        except ValidationError as exc:
            errors.append(f"{failure_id}: {exc}")
        if evidence_path not in referenced_artifacts:
            errors.append(
                f"{failure_id}: evidence is not an artifact of a referenced run: "
                f"{evidence_path}"
            )
    return errors


def validate_all() -> list[str]:
    personas, scenarios = load_corpus()
    errors: list[str] = []
    errors.extend(
        validate_schema(personas, load_json(SCHEMAS / "persona-corpus.schema.json"))
    )
    errors.extend(
        validate_schema(scenarios, load_json(SCHEMAS / "scenario-corpus.schema.json"))
    )
    errors.extend(semantic_errors(personas, scenarios))
    persona_index = by_id(personas, "persona")
    scenario_index = by_id(scenarios, "scenario")
    result_schema = load_json(SCHEMAS / "run-result.schema.json")
    result_index: dict[str, dict[str, Any]] = {}
    for path in sorted(RESULTS.glob("**/*.json")) if RESULTS.exists() else []:
        result = load_json(path)
        schema_errors = validate_schema(result, result_schema)
        errors.extend(f"{path}: {e}" for e in schema_errors)
        if not schema_errors:
            if result["run_id"] in result_index:
                errors.append(f"{path}: duplicate result run_id {result['run_id']!r}")
            result_index[result["run_id"]] = result
            errors.extend(validate_result_semantics(result, persona_index, scenario_index))
    failure_schema = load_json(SCHEMAS / "failure-record.schema.json")
    failure_dir = BASE / "failures"
    failure_ids: set[str] = set()
    for path in sorted(failure_dir.glob("*.json")) if failure_dir.exists() else []:
        record = load_json(path)
        schema_errors = validate_schema(record, failure_schema)
        errors.extend(f"{path}: {e}" for e in schema_errors)
        if not schema_errors:
            if record["failure_id"] in failure_ids:
                errors.append(f"{path}: duplicate failure_id {record['failure_id']!r}")
            failure_ids.add(record["failure_id"])
            errors.extend(validate_failure_semantics(record, scenario_index, result_index))
    return errors


def build_package(
    persona_id: str,
    scenario_id: str,
    variant: str,
    dashboard_url: str | None,
    artifacts: list[str],
    operator_docs: list[str],
) -> dict[str, Any]:
    personas, scenarios = load_corpus()
    persona = by_id(personas, "persona").get(persona_id)
    scenario = by_id(scenarios, "scenario").get(scenario_id)
    if persona is None:
        raise ValidationError(f"unknown persona {persona_id!r}")
    if scenario is None:
        raise ValidationError(f"unknown scenario {scenario_id!r}")
    if variant not in scenario["variants"]:
        raise ValidationError(f"{scenario_id} does not support variant {variant}")
    artifact_paths = [
        str(confined_file(path, RAW_EVIDENCE_ROOT, "dashboard artifact"))
        for path in artifacts
    ]
    operator_doc_paths = [
        str(confined_file(path, OPERATOR_DOCS_ROOT, "operator document"))
        for path in operator_docs
    ]
    core = (BASE / "prompts" / "core-evaluator.md").read_text(encoding="utf-8")
    access_lines = []
    if dashboard_url:
        access_lines.append(f"Dashboard start URL: {dashboard_url}{scenario['start_route']}")
    if artifact_paths:
        access_lines.append("Dashboard artifacts: " + ", ".join(artifact_paths))
    if operator_doc_paths:
        access_lines.append(
            "Visible operator documents: " + ", ".join(operator_doc_paths)
        )
    prompt = "\n\n".join(
        [
            core.strip(),
            "Operator archetype\n\n" + persona["brief"],
            "Scenario context\n\n" + scenario["operator_context"],
            "Task\n\n" + scenario["operator_task"],
            "Access\n\n" + ("\n".join(access_lines) or "No dashboard access supplied."),
        ]
    ) + "\n"
    return {
        "schema": "nq.dashboard.operator-package.v1",
        "persona_id": persona_id,
        "scenario_id": scenario_id,
        "scenario_version": scenario["version"],
        "ui_variant": variant,
        "generated_at": scenario["fixture"]["clock"],
        "dashboard_url": dashboard_url,
        "artifacts": artifact_paths,
        "operator_docs": operator_doc_paths,
        "prompt": prompt,
        "prompt_hash": "sha256:" + sha256_text(prompt),
        "response_schema": str(
            (SCHEMAS / "operator-response.schema.json").relative_to(REPO)
        ),
    }


def adapter_spec(
    family: str, package: dict[str, Any], model: str, run_dir: str
) -> dict[str, Any]:
    response_schema_path = str(SCHEMAS / "operator-response.schema.json")
    prompt = package["prompt"]
    artifacts = package["artifacts"]
    if family == "codex":
        argv = [
            "codex",
            "exec",
            "--ephemeral",
            "--skip-git-repo-check",
            "--sandbox",
            "read-only",
            "--json",
            "--output-schema",
            response_schema_path,
            "--model",
            model,
            "-C",
            run_dir,
        ]
        for artifact in artifacts:
            if artifact.lower().endswith((".png", ".jpg", ".jpeg", ".webp")):
                argv.extend(["-i", artifact])
        argv.append(prompt)
    elif family == "claude":
        schema_inline = canonical_json(load_json(Path(response_schema_path)))
        artifact_instruction = (
            "\n\nUse the Read tool to inspect these dashboard artifacts: "
            + ", ".join(artifacts)
            if artifacts
            else ""
        )
        argv = [
            "claude",
            "-p",
            "--model",
            model,
            "--output-format",
            "json",
            "--no-session-persistence",
            "--safe-mode",
            "--tools",
            "Read",
            "--permission-mode",
            "dontAsk",
            "--json-schema",
            schema_inline,
            prompt + artifact_instruction,
        ]
    else:
        raise ValidationError("family must be codex or claude")
    return {
        "schema": "nq.dashboard.model-adapter-command.v1",
        "family": family,
        "model": model,
        "cwd": run_dir,
        "argv": argv,
        "network_required_for_execution": True,
        "executed_by_harness": False,
        "validation_is_offline": True,
    }


def perfect_mock_result(scenario: dict[str, Any]) -> dict[str, Any]:
    oracle = scenario["oracle"]
    coding = {
        "provenance": "fixture_replay",
        "transcript_citations": ["mock:deterministic"],
        "subject_ids": oracle["required_subject_ids"],
        "conclusion_ids": oracle["required_conclusion_ids"],
        "evidence_ids": oracle["required_evidence_ids"],
        "unknown_ids": oracle["required_unknown_ids"],
        "chosen_action_id": oracle["safe_action_ids"][0],
        "proposed_action_ids": [oracle["safe_action_ids"][0]],
        "novel_action_proposals": [],
        "causality_claimed": oracle["causality"] == "established",
        "stale_data_noticed": oracle["must_notice_stale"],
        "contradictory_data_noticed": oracle["must_notice_conflict"],
    }
    result = {"coding": coding}
    return compute_score(result, scenario)


def write_or_print(value: Any, output: str | None) -> None:
    text = json.dumps(value, indent=2, ensure_ascii=False) + "\n"
    if output:
        Path(output).write_text(text, encoding="utf-8")
    else:
        sys.stdout.write(text)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("validate", help="offline schema, corpus, result, and digest validation")
    sub.add_parser("smoke", help="offline deterministic perfect-result scoring smoke")

    package = sub.add_parser("package", help="build an oracle-free operator prompt package")
    package.add_argument("--persona", required=True)
    package.add_argument("--scenario", required=True)
    package.add_argument("--variant", choices=["baseline", "redesign", "text"], required=True)
    package.add_argument("--dashboard-url")
    package.add_argument("--artifact", action="append", default=[])
    package.add_argument("--operator-doc", action="append", default=[])
    package.add_argument("--output")

    adapter = sub.add_parser("adapter-command", help="emit, but never execute, a model CLI command")
    adapter.add_argument("--family", choices=["codex", "claude"], required=True)
    adapter.add_argument("--package", required=True)
    adapter.add_argument("--model", required=True)
    adapter.add_argument("--run-dir", required=True)
    adapter.add_argument("--output")

    score = sub.add_parser("score", help="deterministically score one coded result")
    score.add_argument("--result", required=True)

    args = parser.parse_args(argv)
    try:
        if args.command == "validate":
            errors = validate_all()
            if errors:
                for error in errors:
                    print(f"ERROR {error}", file=sys.stderr)
                return 1
            personas, scenarios = load_corpus()
            result_count = len(list(RESULTS.glob("**/*.json"))) if RESULTS.exists() else 0
            print(
                f"VALID personas={len(personas)} scenarios={len(scenarios)} "
                f"results={result_count} network=unused"
            )
            return 0
        if args.command == "smoke":
            _, scenarios = load_corpus()
            scored = [perfect_mock_result(scenario) for scenario in scenarios]
            if not all(score["passed"] and score["percent"] == 100.0 for score in scored):
                raise ValidationError("perfect mock did not score 100%")
            print(f"SMOKE scenarios={len(scored)} score=100.0 network=unused")
            return 0
        if args.command == "package":
            value = build_package(
                args.persona,
                args.scenario,
                args.variant,
                args.dashboard_url,
                args.artifact,
                args.operator_doc,
            )
            write_or_print(value, args.output)
            return 0
        if args.command == "adapter-command":
            value = adapter_spec(
                args.family, load_json(Path(args.package)), args.model, args.run_dir
            )
            write_or_print(value, args.output)
            return 0
        if args.command == "score":
            result = load_json(Path(args.result))
            scenario = scenario_map().get(result["scenario_id"])
            if scenario is None:
                raise ValidationError(f"unknown scenario {result['scenario_id']!r}")
            write_or_print(compute_score(result, scenario), None)
            return 0
    except (ValidationError, KeyError) as exc:
        print(f"ERROR {exc}", file=sys.stderr)
        return 1
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
