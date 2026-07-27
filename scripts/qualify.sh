#!/bin/sh
# qualify.sh — the one command that tells the truth about NQ's required gates.
#
# Why this exists: every check below already ran as a separate CI job, and
# `check-coverage-manifest` still sat red for weeks because a red job among
# green ones is easy to stop seeing. This is signal routing — a runner over
# existing checks, not a new gate framework. Nothing here is a new rule.
#
# Deliberately NOT included: `cargo fmt --check` and `cargo clippy`. The tree
# carries ~1479 formatting hunks and 202 undenied clippy warnings, and neither
# is a required gate today. Adding them would make this script red on arrival,
# which recreates exactly the problem it exists to fix: a gate whose failure is
# expected and therefore unread. The debt is real and is tracked elsewhere; it
# is excluded here by decision, not by oversight.
#
# Exit: 0 when every gate passed, 1 otherwise. Each gate's own exit code is
# observed directly — no pipeline masking, no eyeballing output tails.
set -u

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root" || exit 1

failed=0
results=""

run_gate() {
    name=$1
    shift
    "$@" >/dev/null 2>&1
    status=$?
    if [ "$status" -eq 0 ]; then
        results="${results}  PASS  ${name}\n"
    else
        results="${results}  FAIL  ${name} (exit ${status})\n"
        failed=1
    fi
}

# 1. The full suite. Contains the consumer-reliance, operational-health,
#    machine-transport, and Docket dossier tests as workspace targets.
run_gate "cargo test --all --locked" cargo test --all --locked

# 2-5. The scripted fail-closed gates.
run_gate "check_gap_status" ./scripts/check_gap_status.sh
run_gate "check-nq-receipts" ./scripts/check-nq-receipts.sh
run_gate "check-constellation-boundaries" ./scripts/check-constellation-boundaries.sh
run_gate "check-coverage-manifest" ./scripts/check-coverage-manifest.sh

# 6. Targeted integration suites, named explicitly. The full suite already runs
#    these; naming them means a rename or accidental deletion surfaces as a
#    missing target instead of hiding inside a slightly smaller total.
run_gate "docket dossier v1/v2/v3 profile" \
    cargo test -p nq-monitor --test docket_dossier_import
run_gate "consumer reliance" cargo test -p nq-core --lib reliance
run_gate "reliance conformance vectors" \
    cargo test -p nq-core --test reliance_conformance
run_gate "reliance machine transport" \
    cargo test -p nq-monitor --test reliance_transport

printf '%b' "$results"
if [ "$failed" -eq 0 ]; then
    echo "nq qualification: PASS"
else
    echo "nq qualification: FAIL" >&2
fi
exit "$failed"
