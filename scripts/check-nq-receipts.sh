#!/usr/bin/env bash
#
# check-nq-receipts.sh — fail-closed re-attestation of the committed nq.receipt.v1
# specimen population (Packet #5). Runs the EXISTING `nq-monitor receipt check`
# engine over each declared receipt with its required witness packets.
#
# Doctrine:
#   The gate does not certify that the underlying claim is true.
#   It certifies only that the receipt remains admissible under its own
#   documented claims (content-hash integrity, witness anchoring, supported
#   schema; freshness when demanded).
#
# Fail-closed on: missing receipt, missing witness packet, digest drift,
# anchoring failure, unsupported schema, freshness-unprovable-when-demanded,
# verifier/build failure, or a negative fixture whose refusal is NOT witnessed.
#
# Verification discipline: each check's TRUE exit code is the observed exit
# (no pipe to tail/grep). Pass/fail is decided by that exit code.
#
# Usage: scripts/check-nq-receipts.sh   (build nq-monitor first, or set NQ_MONITOR_BIN)
set -uo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$HERE/specimens/receipts"
MANIFEST="$ROOT/MANIFEST"

BIN="${NQ_MONITOR_BIN:-}"
if [ -z "$BIN" ]; then
  for c in "$HERE/target/release/nq-monitor" "$HERE/target/debug/nq-monitor"; do
    [ -x "$c" ] && BIN="$c" && break
  done
fi
[ -n "$BIN" ] && [ -x "$BIN" ] || { echo "RECEIPT GATE: FAIL — nq-monitor binary not found (set NQ_MONITOR_BIN or 'cargo build --release -p nq-monitor')"; exit 1; }
[ -f "$MANIFEST" ] || { echo "RECEIPT GATE: FAIL — manifest missing: $MANIFEST"; exit 1; }

fail=0; ok=0; checked=0
while IFS='|' read -r mode name receipt witness flags cond; do
  case "${mode:-}" in ''|\#*) continue ;; esac
  rp="$ROOT/$receipt"
  if [ ! -f "$rp" ]; then echo "FAIL [$name]: missing receipt $receipt"; fail=$((fail+1)); continue; fi

  args=(receipt check)
  # shellcheck disable=SC2206  -- intentional word-split of the flags column
  [ -n "${flags:-}" ] && args+=(${flags})
  args+=(--receipt "$rp")
  if [ -n "${witness:-}" ]; then
    miss=0
    for w in ${witness//,/ }; do
      wp="$ROOT/$w"
      if [ ! -f "$wp" ]; then echo "FAIL [$name]: missing witness $w"; fail=$((fail+1)); miss=1; fi
      args+=(--witness "$wp")
    done
    [ "$miss" -eq 1 ] && continue
  fi

  "$BIN" "${args[@]}" >/dev/null 2>&1
  rc=$?
  checked=$((checked+1))

  if [ "${mode}" = positive ]; then
    if [ "$rc" -eq 0 ]; then ok=$((ok+1)); echo "ok   positive[$name] admissible (exit 0; ${cond:-?})"
    else echo "FAIL positive[$name] expected admissible, got exit=$rc"; fail=$((fail+1)); fi
  else
    if [ "$rc" -ne 0 ]; then ok=$((ok+1)); echo "ok   negative[$name] refusal witnessed (exit $rc; ${cond:-?})"
    else echo "FAIL negative[$name] expected refusal (${cond:-?}) but exit=0 — failure NOT witnessed"; fail=$((fail+1)); fi
  fi
done < "$MANIFEST"

echo "---"
echo "checked=$checked ok=$ok fail=$fail  (binary: $BIN)"
[ "$checked" -gt 0 ] || { echo "RECEIPT GATE: FAIL — manifest attested nothing"; exit 1; }
[ "$fail" -eq 0 ]   || { echo "RECEIPT GATE: FAIL — $fail violation(s)"; exit 1; }
echo "RECEIPT GATE: PASS"
