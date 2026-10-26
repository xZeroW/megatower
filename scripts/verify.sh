#!/usr/bin/env bash
set -euo pipefail
python3 tools/export_stats.py --check
python3 tools/export_containers.py --check
python3 tools/export_status_effects.py --check
python3 tools/export_character_content.py --check
python3 tools/export_items.py --check
python3 tools/export_enemies.py --check
python3 tools/export_adventurers.py --check
python3 tools/export_battle_actions.py --check
python3 tools/probe_format.py --check
python3 tools/probe_market_quote.py --check
python3 tools/export_room_scenes.py --check
python3 tools/export_bb_scripts.py --check
python3 tools/audit_script_ports.py --check
cargo fmt --check
cargo test
cargo check
