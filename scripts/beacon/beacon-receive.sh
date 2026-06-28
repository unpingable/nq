#!/usr/bin/env bash
#
# beacon-receive.sh — runs ON the external vantage (the public VM). Witnesses the
# ARRIVAL of a LAN-originated beacon and records it at the vantage's own clock.
# Packet #7b (egress-liveness external witness, minimal slice).
#
# Doctrine:
#   A received beacon witnesses external arrival of a LAN-originated signal.
#   A missing beacon witnesses absence-at-vantage, not the cause of absence.
#
# Custody: records ONLY a vantage-clock UTC timestamp, a nonce, and a declared
# source label. It deliberately sanitizes both inputs so no LAN topology (IP / MAC /
# hostname / path) can be smuggled into the log on the shared public host.
set -uo pipefail
LOG="${BEACON_LOG:-$HOME/beacon/arrivals.jsonl}"
mkdir -p "$(dirname "$LOG")"

nonce=$(printf '%s' "${1:-}" | tr -cd 'a-zA-Z0-9' | head -c 32)
label=$(printf '%s' "${2:-}" | tr -cd 'a-zA-Z0-9_-' | head -c 32)
[ -n "$nonce" ] && [ -n "$label" ] || { echo "beacon-receive: missing/invalid nonce or label" >&2; exit 64; }

now=$(date -u +%Y-%m-%dT%H:%M:%SZ)   # vantage clock IS the witness clock
printf '{"schema":"nq.beacon_arrival.v0","witnessed_at_vantage":"%s","nonce":"%s","source_label":"%s"}\n' \
  "$now" "$nonce" "$label" >> "$LOG"
echo "arrival witnessed at vantage $now (nonce=$nonce label=$label)"
