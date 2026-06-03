#!/usr/bin/env bash
#
# Enforce the "gap docs are design records, not shipped-state ledgers"
# doctrine recorded in docs/working/decisions/ARCHITECTURE_NOTES.md.
#
# A gap doc whose Status field's lifecycle label is one of
# (shipped, landed, retired, partial, partially resolved) MUST also
# reference docs/working/decisions/FEATURE_HISTORY.md somewhere in the doc.
# The repeated rot pattern (specimens documented in ARCHITECTURE_NOTES:
# FINDING_EXPORT, FINDING_DIAGNOSIS, DOMINANCE_PROJECTION,
# GENERATION_LINEAGE, EVIDENCE_LAYER, REGIME_FEATURES,
# COVERAGE_HONESTY, OPERATIONAL_INTENT_DECLARATION,
# TESTIMONY_DEPENDENCY, EVIDENCE_RETIREMENT, ZFS_COLLECTOR) was Status
# saying "built, shipped" while the FEATURE_HISTORY ledger never gained
# the entry. Each pickup forced a multi-day reconciliation pass that
# turned up real consumer/test gaps invisible from the front-matter.
#
# Rule: match only the lifecycle label at the start of the Status value
# (after optional `**` / `` ` `` wrappers). "shipped" appearing as prose
# later in the line ("currently shipped behavior", "shipped via X")
# does not trip the check — only the label position does.
#
# `resolved` is deliberately excluded: a Status of `resolved` may be a
# recognition closure (no work shipped, no ledger entry to make) as
# well as a ship event. The label set above covers the unambiguous
# ship/land/partial cases; recognition closures are reviewed by humans.
#
# Exit non-zero with a clear message when violated.

set -euo pipefail

cd "$(dirname "$0")/.."

# Lifecycle labels that indicate shipped/landed state.
# Order matters for the regex: longer phrases ("partially resolved")
# must precede the shorter prefix ("partial") so the alternation tries
# the full phrase first.
shipped_labels='partially resolved|shipped|landed|retired|partial'

violations=()
for f in docs/working/gaps/*.md; do
  [ -f "$f" ] || continue
  status_line=$(grep -m1 -E '^\*\*Status:\*\*' "$f" || true)
  [ -n "$status_line" ] || continue

  # Strip the **Status:** marker and any leading bold/code wrappers
  # to isolate the lifecycle label position. The label is the first
  # token of the resulting Status value.
  label_region=$(printf '%s' "$status_line" \
    | sed -E 's/^\*\*Status:\*\*[[:space:]]*//' \
    | sed -E 's/^[`*]+[[:space:]]*//' \
    | tr '[:upper:]' '[:lower:]')

  # Match the lifecycle label only at the start of the label region,
  # with a word-boundary terminator so "partial" matches "partial —"
  # but not "partially".
  if printf '%s' "$label_region" | grep -Eq "^(${shipped_labels})\\b"; then
    if ! grep -q 'FEATURE_HISTORY' "$f"; then
      violations+=("$f")
    fi
  fi
done

if [ ${#violations[@]} -gt 0 ]; then
  printf 'Gap docs claim shipped lifecycle without pointing at FEATURE_HISTORY:\n'
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
