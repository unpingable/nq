#!/usr/bin/env bash
#
# check-witness-boundaries.sh — structural (build-graph) enforcement that witness
# crates cannot NAME NQ's persistence/coercion surface. Packet #6.
#
# Doctrine:
#   A witness component should not be able to name the surface that would let it
#   coerce the state it claims to observe. A witness that could write `nq-db`
#   could manufacture the very findings it is meant to be raw testimony for.
#
# Mechanism: read the RESOLVED cargo dependency graph (`cargo tree -e normal`) —
# enforcement is the build graph, not convention or lint.
#   Forbidden: nq-witness, nq-witness-api MUST NOT have nq-db in their normal closure.
#   Control:   nq-monitor MUST have nq-db (proves the graph reader is not silently
#              returning empty — fail closed if a known-true edge is undetectable).
#   Self-test: a synthetic nq-witness->nq-db closure MUST be flagged (proves the
#              detector catches a violation; analog of a negative fixture).
#
# Verification discipline: each predicate's own result decides pass/fail (no
# pipe-masked exit codes). Allowed exceptions are documented in
# docs/working/decisions/WITNESS_PROBE_BOUNDARY.md.
set -uo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
cd "$HERE"

FORBIDDEN=( "nq-witness:nq-db" "nq-witness-api:nq-db" )
CONTROL=( "nq-monitor:nq-db" )

# closure CRATE -> the nq-* crates in its normal dependency closure, one per line
closure() { cargo tree -p "$1" -e normal --prefix none 2>/dev/null | grep -oE '^nq-[a-z-]+' | sort -u; }

# has_dep CONSUMER DEP [override-file] -> exit 0 if DEP is in CONSUMER's closure
has_dep() {
  local c="$1" d="$2" ov="${3:-}"
  if [ -n "$ov" ]; then grep -qx "$d" "$ov"; else closure "$c" | grep -qx "$d"; fi
}

selftest() {
  local tmp; tmp="$(mktemp)"; printf 'nq-core\nnq-db\nnq-witness-api\n' > "$tmp"
  if has_dep nq-witness nq-db "$tmp"; then echo "ok:   self-test — synthetic nq-witness->nq-db IS flagged"; rm -f "$tmp"; return 0
  else echo "FAIL: self-test — detector did not flag a synthetic violation"; rm -f "$tmp"; return 1; fi
}

[ "${1:-}" = "--self-test" ] && { selftest; exit $?; }

fail=0
selftest || fail=1   # built-in tripwire: detector must be able to catch a violation

for p in "${FORBIDDEN[@]}"; do c="${p%%:*}"; d="${p##*:}"
  if has_dep "$c" "$d"; then echo "FAIL: $c depends on forbidden $d (witness can name the coercion surface)"; fail=1
  else echo "ok:   $c  is independent of  $d"; fi
done
for p in "${CONTROL[@]}"; do c="${p%%:*}"; d="${p##*:}"
  if has_dep "$c" "$d"; then echo "ok:   control $c -> $d present (graph reader works)"
  else echo "FAIL: control $c should depend on $d — graph reader broken, failing closed"; fail=1; fi
done

echo "---"
[ "$fail" -eq 0 ] || { echo "WITNESS BOUNDARY GATE: FAIL"; exit 1; }
echo "WITNESS BOUNDARY GATE: PASS"
