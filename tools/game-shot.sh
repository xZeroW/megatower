#!/usr/bin/env bash
# Runs the new game client once, saves a screenshot, then exits.
#   tools/game-shot.sh OUT.png [VAR=value ...]
# e.g. tools/game-shot.sh /tmp/map.png GAME_SEED=7 GAME_FLOOR=3 GAME_MOVES='*'
# GAME_MOVES: N/E/S/W move, . resolve, > descend, * walk to the stairs.
set -euo pipefail
cd "$(dirname "$0")/.."
out="${1:?usage: tools/game-shot.sh OUT.png [VAR=value ...]}"
shift
cargo build -q -p game-client
out_abs="$(realpath -m "$out")"
env GAME_CAPTURE="$out_abs" "$@" timeout 60 target/debug/game >/dev/null 2>&1 || true
test -f "$out_abs" || { echo "no screenshot written" >&2; exit 1; }
