#!/usr/bin/env bash
# Runs the game once and saves a screenshot, then exits.
#   tools/qa-shot.sh OUT.png [VAR=value ...]
# e.g. tools/qa-shot.sh /tmp/merchant.png MEGALOOT_AUTOSTART=Fren MEGALOOT_SEED=7 \
#        MEGALOOT_START_ROOM=MERCHANT
# With MEGALOOT_CAPTURE_FRAMES=n the frames land in artifacts/frames/ as well.
set -euo pipefail
cd "$(dirname "$0")/.."
out="${1:?usage: tools/qa-shot.sh OUT.png [VAR=value ...]}"
shift
cargo build -q -p megatower-game
env MEGALOOT_AUTOSTART="${MEGALOOT_AUTOSTART:-Fren}" MEGALOOT_CAPTURE_SCREENSHOT=1 \
    MEGALOOT_CAPTURE_DELAY_SECS="${MEGALOOT_CAPTURE_DELAY_SECS:-2}" "$@" \
    timeout 120 target/debug/megatower >/dev/null 2>&1 || true
cp artifacts/latest.png "$out"
