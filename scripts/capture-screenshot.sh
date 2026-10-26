#!/usr/bin/env bash
set -euo pipefail
mkdir -p artifacts
rm -f artifacts/latest.png
if [[ -n "${MEGALOOT_CAPTURE_ROOM_SCREENSHOT:-}" ]]; then
  rm -f artifacts/room-render-debug.png
fi
# Bevy captures its own rendered primary window; no desktop screenshot utility
# or compositor is involved.
xvfb-run -a -s "-screen 0 1280x720x24" bash -c '
  MEGALOOT_CAPTURE_SCREENSHOT=1 cargo run --quiet & app=$!
  for _ in $(seq 1 ${MEGALOOT_CAPTURE_WAIT_ITERS:-90}); do
    files=(artifacts/latest.png)
    if [[ -n "${MEGALOOT_CAPTURE_ROOM_SCREENSHOT:-}" ]]; then
      files+=(artifacts/room-render-debug.png)
    fi
    complete=true
    for file in "${files[@]}"; do
      if ! test -s "$file" || ! python3 -c '\''from pathlib import Path; import sys; b=Path(sys.argv[1]).read_bytes(); raise SystemExit(0 if len(b) > 20 and b[-8:-4] == b"IEND" else 1)'\'' "$file"; then
        complete=false
        break
      fi
    done
    if $complete; then
      before=$(stat -c %s "${files[0]}")
      before_room=0
      if [[ ${#files[@]} -gt 1 ]]; then before_room=$(stat -c %s "${files[1]}"); fi
      sleep 1
      after=$(stat -c %s "${files[0]}")
      after_room=0
      if [[ ${#files[@]} -gt 1 ]]; then after_room=$(stat -c %s "${files[1]}"); fi
      if test "$before" = "$after" && test "$before_room" = "$after_room"; then break; fi
    fi
    sleep 1
  done
  kill "$app" || true
  wait "$app" 2>/dev/null || true
'
test -s artifacts/latest.png
