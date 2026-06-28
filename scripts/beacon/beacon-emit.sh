#!/usr/bin/env bash
#
# beacon-emit.sh — runs on a LAN host (sushi-k). Emits one egress beacon to the
# external vantage over the EXISTING SSH channel (no new inbound port; the linode
# firewall blocks those anyway). Carries ONLY a fresh nonce + a declared, benign
# source label — never a LAN IP / MAC / hostname / path. Packet #7b.
#
# Low-rate + short timeout + single-shot (cadence + backoff belong to the timer
# wrapper, nq-beacon-emit.timer). Exits non-zero when the beacon does NOT arrive,
# so a supervising timer can back off.
#
# Env knobs (defaults are safe):
#   BEACON_VANTAGE  ssh target          (default root@labelwatch.neutral.zone)
#   BEACON_KEY      ssh identity         (default ~/git/claude/ssh/linode — stays on LAN)
#   BEACON_LABEL    declared source label(default nq-lan-egress; must be benign)
#   BEACON_RECV     receiver path on VM  (default /root/beacon/beacon-receive.sh)
set -uo pipefail
VANTAGE="${BEACON_VANTAGE:-root@labelwatch.neutral.zone}"
KEY="${BEACON_KEY:-$HOME/git/claude/ssh/linode}"
LABEL="${BEACON_LABEL:-nq-lan-egress}"
RECV="${BEACON_RECV:-/root/beacon/beacon-receive.sh}"

nonce=$(head -c16 /dev/urandom | od -An -tx1 | tr -d ' \n')
ssh -i "$KEY" -o IdentitiesOnly=yes -o ConnectTimeout=8 -o BatchMode=yes "$VANTAGE" \
    "$RECV $nonce $LABEL" 2>/dev/null
rc=$?
if [ "$rc" -eq 0 ]; then
  echo "beacon emitted + arrival witnessed (nonce=$nonce label=$LABEL)"
else
  echo "beacon emit FAILED rc=$rc — external arrival NOT witnessed (absence-at-vantage if persistent; NOT 'WAN down' by itself)" >&2
fi
exit "$rc"
