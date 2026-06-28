#!/usr/bin/env bash
#
# beacon-status.sh — runs ON the vantage (VM). Classifies external-arrival liveness
# from the arrival log. Narrow verdicts only. Packet #7b.
#
# DOCTRINE (the whole point of this packet):
#   A received beacon witnesses external ARRIVAL of a LAN-originated signal.
#   A missing beacon witnesses ABSENCE-AT-VANTAGE, not the cause. This is NOT a
#   "WAN down" oracle — absence-at-vantage can be emitter down, key/SSH change,
#   VM load, route change, or WAN egress loss. The witness reports position, not cause.
#
# Verdicts: arrival_witnessed | absence_at_vantage | cannot_classify_no_arrivals_basis
set -uo pipefail
LOG="${BEACON_LOG:-$HOME/beacon/arrivals.jsonl}"
THRESHOLD="${BEACON_STALE_S:-900}"   # 15 min default

if [ ! -s "$LOG" ]; then
  echo '{"schema":"nq.beacon_status.v0","verdict":"cannot_classify_no_arrivals_basis"}'
  exit 0
fi

last=$(tail -n1 "$LOG")
last_ts=$(printf '%s' "$last" | grep -oE '"witnessed_at_vantage":"[^"]+"' | cut -d'"' -f4)
now_s=$(date -u +%s)
last_s=$(date -u -d "$last_ts" +%s 2>/dev/null || echo 0)
age=$(( now_s - last_s ))

if [ "$last_s" -gt 0 ] && [ "$age" -le "$THRESHOLD" ]; then
  printf '{"schema":"nq.beacon_status.v0","verdict":"arrival_witnessed","last_arrival_at_vantage":"%s","age_s":%d,"threshold_s":%d}\n' \
    "$last_ts" "$age" "$THRESHOLD"
else
  printf '{"schema":"nq.beacon_status.v0","verdict":"absence_at_vantage","note":"external arrival not witnessed within threshold; NOT wan_down by itself","last_arrival_at_vantage":"%s","age_s":%d,"threshold_s":%d}\n' \
    "$last_ts" "$age" "$THRESHOLD"
fi
