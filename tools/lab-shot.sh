#!/usr/bin/env bash
# Runs megatower with the new rules once, saves a screenshot, then exits.
#   tools/lab-shot.sh OUT.png [VAR=value ...]
# e.g. tools/lab-shot.sh /tmp/fight.png GAME_SEED=1 GAME_MOVES='1f' GAME_LATE_MOVES='1' \
#        MEGALOOT_CAPTURE_DELAY_SECS=3
# GAME_MOVES / GAME_LATE_MOVES are documented on `scripted_moves` in
# crates/megatower-ui/src/labyrinth/mod.rs; GAME_LATE_AT (seconds, default 1)
# says when the late ones play, so combat text is still on screen at capture.
set -euo pipefail
cd "$(dirname "$0")/.."
out="${1:?usage: tools/lab-shot.sh OUT.png [VAR=value ...]}"
shift
cargo build -q -p megatower-client
# The binary is run directly, so it needs the asset root Cargo would set.
env BEVY_ASSET_ROOT="$PWD" GAME_AUTOSTART=1 MEGALOOT_CAPTURE_SCREENSHOT=1 \
    MEGALOOT_CAPTURE_DELAY_SECS="${MEGALOOT_CAPTURE_DELAY_SECS:-2}" "$@" \
    timeout 120 target/debug/megatower >/dev/null 2>&1 || true
cp artifacts/latest.png "$out"
git checkout -- artifacts/latest.png 2>/dev/null || true
