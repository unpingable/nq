#!/usr/bin/env bash
#
# check-coverage-manifest.sh — fail-closed coverage declaration (P0 #2).
#
# Makes absence DECLARED, not laundered. Fails closed when:
#   1. an implemented NQ-repo witness surface is missing from coverage/manifest
#      (a ClaimKind enum variant, or an active-witness *_probe.rs, or a lab
#      fixture dir not referenced by any entry) — silent absence;
#   2. a manifest entry references dead/unknown evidence (path does not exist);
#   3. a deferred / not_expected entry carries no rationale; or a bad status.
#
# Wired into CI as the `coverage-manifest` job. True exit codes (no pipe masking).
set -uo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
cd "$HERE"
MAN="coverage/manifest"
[ -f "$MAN" ] || { echo "COVERAGE MANIFEST: FAIL — missing $MAN"; exit 1; }

snake() { echo "$1" | sed -E 's/([a-z0-9])([A-Z])/\1_\2/g' | tr '[:upper:]' '[:lower:]'; }
manifest_has() { grep -qE "^$1\|$2\|" "$MAN"; }

fail=0

# (2)+(3): every entry's evidence must exist; deferred/not_expected need a rationale.
while IFS='|' read -r cat name status evidence rationale; do
  case "${cat:-}" in ''|\#*) continue ;; esac
  [ -e "$evidence" ] || { echo "FAIL: $cat/$name references dead/unknown evidence: '$evidence'"; fail=1; }
  case "$status" in
    implemented|lab_backed) ;;
    deferred|not_expected)
      [ -n "${rationale//[[:space:]]/}" ] || { echo "FAIL: $cat/$name is '$status' with no rationale"; fail=1; }
      ;;
    *) echo "FAIL: $cat/$name has unknown status '$status'"; fail=1 ;;
  esac
done < "$MAN"

# (1a): every ClaimKind enum variant is declared.
variants=$(sed -n '/pub enum ClaimKind/,/^}/p' crates/nq-core/src/preflight.rs \
  | grep -E '^[[:space:]]+[A-Z][A-Za-z]+,$' | tr -d ' ,')
for v in $variants; do
  s=$(snake "$v")
  manifest_has claim_kind "$s" || { echo "FAIL: ClaimKind '$v' ($s) not declared in $MAN"; fail=1; }
done

# (1b): every active-witness probe is declared.
for p in crates/nq-monitor/src/*_probe.rs; do
  name=$(basename "$p" _probe.rs)
  manifest_has active_probe "$name" || { echo "FAIL: active probe '$name' not declared in $MAN"; fail=1; }
done

# (1c): every lab fixture dir is referenced by some entry's evidence.
for d in crates/nq-monitor/tests/fixtures/*/; do
  dd="${d%/}"
  grep -qF "$dd" "$MAN" || { echo "FAIL: fixture dir '$dd' not referenced by any manifest entry"; fail=1; }
done

echo "---"
[ "$fail" -eq 0 ] || { echo "COVERAGE MANIFEST: FAIL"; exit 1; }
echo "COVERAGE MANIFEST: PASS"
