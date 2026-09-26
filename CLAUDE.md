# rzr: working in this repository

A control panel for Razer BlackShark V2 Pro headsets that replaces Razer Synapse. Rust plus a web page (WebView2) on Windows.

## Before you start

1. Read **[docs/STATUS.md](docs/STATUS.md)**: what's done, what's missing and what's next. It's the source of truth for the work.
2. Use the terms in **[CONTEXT.md](CONTEXT.md)** (preset, slot, curve, remote mode, model…) in code, commits and conversation.
3. Follow **[CONTRIBUTING.md](CONTRIBUTING.md)**. The essentials are below.
4. If `local/HANDOFF.md` exists, read it too: the maintainer's notes for working on their own PC (never published; `local/` is ignored by git).

More: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) (how the app is built), [docs/PROTOCOL.md](docs/PROTOCOL.md) (the headset's USB protocol), [docs/RESEARCH.md](docs/RESEARCH.md) (Synapse, THX and Windows findings), [docs/NEW-HEADSETS.md](docs/NEW-HEADSETS.md), [docs/adr/](docs/adr/) (decisions and why), [CHANGELOG.md](CHANGELOG.md).

## Commands

```
cargo fmt --check                          # format (rustfmt.toml: 120 columns)
cargo clippy --all-targets -- -D warnings  # no warnings
cargo test                                 # tests
node --check ui/app.js && node --check ui/demo.js
cargo build --release                      # on Linux add --target x86_64-pc-windows-gnu
cargo run -- --demo                        # the panel with a simulated headset
```

On Linux you need `libwebkit2gtk-4.1-dev` (panel) and, to build for Windows, the `x86_64-pc-windows-gnu` target with `mingw-w64`. The page can be opened alone in a browser (`ui/index.html` loads `ui/demo.js`; `#mic`, `#settings`… open a tab).

## Key rules

- **Language:** everything in English: interface, docs, logs, code, comments, commits.
- **Never write to a headset without a verified source:** every new sequence follows what was captured from Synapse or verified by OpenRazer, goes through the bus lock (see `device.rs`) and is read back when the firmware allows. Unsupported models get read-only diagnostics only.
- **Tell verified-on-hardware apart from the rest.** Nothing is marked ✅ in STATUS.md without a test on a real headset.
- **Every change updates the docs:** STATUS.md always, CHANGELOG.md if users notice, ARCHITECTURE.md if the structure changes, an ADR for a decision that is hard to undo.
- **No Razer or THX files in the repo** (DLLs, drivers, installers). THX's installers live in their own repository ([ADR 0009](docs/adr/0009-thx-installer-in-a-separate-repository.md)).
- **Before pushing:** format, clippy, tests and the Windows build all green.
