#!/usr/bin/env bash
#
# Enforce the "gap docs are design records, not shipped-state ledgers"
# doctrine recorded in docs/working/decisions/ARCHITECTURE_NOTES.md.
#
# A gap doc whose Status line claims shipped state must point at
# docs/working/decisions/FEATURE_HISTORY.md. The repeated rot pattern (specimens
# documented in ARCHITECTURE_NOTES: FINDING_EXPORT, FINDING_DIAGNOSIS,
# DOMINANCE_PROJECTION, GENERATION_LINEAGE, EVIDENCE_LAYER,
# REGIME_FEATURES, COVERAGE_HONESTY, OPERATIONAL_INTENT_DECLARATION,
# TESTIMONY_DEPENDENCY, EVIDENCE_RETIREMENT, ZFS_COLLECTOR) was
# Status saying "built, shipped" while the FEATURE_HISTORY ledger
# never gained the entry. Each pickup forced a multi-day reconciliation
# pass that turned up real consumer/test gaps invisible from the
# front-matter.
#
# Rule: a gap doc whose Status field contains 'shipped' (case-
# insensitive) MUST also reference docs/working/decisions/FEATURE_HISTORY.md somewhere
# in the doc.
#
# Exit non-zero with a clear message when violated.

set -euo pipefail

cd "$(dirname "$0")/.."

violations=()
for f in docs/working/gaps/*.md; do
  [ -f "$f" ] || continue
  status_line=$(grep -m1 -E '^\*\*Status:\*\*' "$f" || true)
  [ -n "$status_line" ] || continue

  # Lowercase comparison — handle 'shipped' / 'Shipped' / 'SHIPPED'.
  lower_status=$(printf '%s' "$status_line" | tr '[:upper:]' '[:lower:]')
  case "$lower_status" in
    *shipped*) ;;
    *) continue ;;
  esac

  # Looking for any reference to FEATURE_HISTORY in the doc.
  if ! grep -q 'FEATURE_HISTORY' "$f"; then
    violations+=("$f")
  fi
done

if [ ${#violations[@]} -gt 0 ]; then
  printf 'Gap docs claim shipped state without pointing at FEATURE_HISTORY:\n'
  for v in "${violations[@]}"; do
    printf '  %s\n' "$v"
  done
  printf '\nDoctrine: docs/working/decisions/ARCHITECTURE_NOTES.md §"Gap docs are design records,\n'
  printf 'not shipped-state ledgers."\n'
  printf '\nRepair: write a docs/working/decisions/FEATURE_HISTORY.md entry with explicit evidence\n'
  printf 'pointers (commits, paths, tests, what is unblocked), then trim the\n'
  printf 'gap-doc Status to a one-line pointer at the FEATURE_HISTORY anchor.\n'
  exit 1
fi
