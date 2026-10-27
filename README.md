# Tower of Loot

## Run

```bash
cargo run -p megaloot
```

Settings are saved to `~/.config/io.github.xzerow.megatower/settings.toml`
(`$XDG_CONFIG_HOME` is honoured); the profile goes to `~/.local/share/megaloot/profile.json`
(`MEGALOOT_USER_DIR` overrides it).

## QA

```bash
cargo run -p megaloot --features dev
```

enables the Bevy Remote Protocol with `bevy_brp_extras` on port 15702 (screenshots,
key/mouse injection), e.g.

```bash
curl -s -X POST http://127.0.0.1:15702 -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"brp_extras/send_keys","params":{"keys":["Space"]}}'
```

The `MEGALOOT_*` variables in `src/dev/script.rs` drive scripted runs and screenshots.
