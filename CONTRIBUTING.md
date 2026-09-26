# Contributing

Thanks for helping. These rules apply to people and to AI assistants alike. If a rule really gets in the way, change it here first, with the reason.

- **Have a headset rzr doesn't support yet?** Start with [docs/NEW-HEADSETS.md](docs/NEW-HEADSETS.md): the read-only diagnostics report is the most useful thing you can send.
- **Found a bug?** Open an issue with the bug template. A `debug.log` (Settings › Diagnostics) helps a lot. Check it first: it lists every message to the headset, but no personal data.

## 1. Principles

1. **Do no harm to the headset.** rzr only sends sequences Synapse sends or OpenRazer verified on hardware. Never try commands "to see what happens" from the panel. Models that aren't supported yet get read-only diagnostics, nothing else.
2. **Tell the truth about what works.** Tested on a real headset is not the same as "it compiles". [docs/STATUS.md](docs/STATUS.md) marks each feature ✅ / 🟡 / ❌, and nothing gets ✅ without a hardware test.
3. **Simple before clever.** A clear file beats an abstraction used once.
4. **The panel shows what the headset says**, not what we hope: if a write isn't confirmed, the user is told.
5. **Small and verifiable.** Short changes, each tested, each with its commit.

## 2. Language and names

- Everything is in **English**: interface, docs, logs, code, comments and commits.
- Domain words come from [CONTEXT.md](CONTEXT.md). If one is missing, add it there before using it.

## 3. Setting up

**On Windows** (the real target):

1. Rust stable with the MSVC toolchain (`rustup default stable-x86_64-pc-windows-msvc`). It needs the Visual Studio Build Tools with "Desktop development with C++". This is what CI uses.
2. Node.js, only for `node --check`.
3. Close Razer Synapse before running rzr with a headset: both fight over the dongle.

**On Linux** (development only): `libwebkit2gtk-4.1-dev` for the panel. To check the Windows build, add the `x86_64-pc-windows-gnu` target and `mingw-w64`.

Useful commands:

```
cargo run -- --demo        # the panel with a simulated headset
cargo run --release        # the panel with a real headset
```

The page can also be opened on its own in a browser: `ui/index.html` loads `ui/demo.js` with made-up data. Add `#mic`, `#power`… to the address to open a tab. To edit the page inside rzr without rebuilding, set `RZR_UI_DIR` to the `ui` folder and press F5 after each change.

## 4. Code

- **Format:** `cargo fmt` (`rustfmt.toml`: 120 columns). CI enforces it.
- **Lint:** `cargo clippy --all-targets -- -D warnings` with no warnings. If one really makes no sense, allow it at that exact spot with a comment saying why.
- **Comments** explain *why* (a firmware quirk, a decision), not what the code already says. Each file starts with `//!` saying what it's for.
- **Errors:** no `unwrap()` on data from the headset, files or Windows. Errors reach the user in plain words and `debug.log` in detail.
- **Layers:** follow [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). `protocol.rs` does no I/O; headset I/O only happens on the headset thread; the page knows nothing about the protocol.
- **Headset:** every new sequence:
  - takes the bus lock;
  - starts with the remote-mode frame that wakes the link;
  - reads the value back when the firmware allows it;
  - writes what it did to `debug.log` (`dlog!`).
- **Dependencies:** only if they save real work and are light. Before adding one, measure how much `rzr.exe` grows and put it in the commit message.
- **Page (`ui/`):** plain HTML, CSS and JavaScript, no frameworks, no build step. Anything that changes state goes through a command to Rust. If the commands or the state change, update `demo.js` too.

## 5. Tests and checks

Before every push, all green:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
node --check ui/app.js && node --check ui/demo.js
cargo build --release          # on Linux: --target x86_64-pc-windows-gnu
```

- **Protocol:** each new command has a test comparing its bytes with a real capture.
- **Logic without hardware** (config, importer, page commands, diagnostics): unit tests.
- **Interface:** open `ui/index.html` in a browser and, if you can, `rzr --demo`.
- **Hardware:** what can't be tested without the headset goes to [docs/STATUS.md](docs/STATUS.md) as 🟡, with the steps to test it.
- **Fixes:** first reproduce the bug (a failing test, or steps), then fix it.

## 6. Git

- **Branches:** work on a branch, never straight on `main`.
- **Commits:** in English, imperative title under ~60 characters ("Add THX phase to the capture script"). The body says why, what was verified and what wasn't.
- **One commit, one idea.** Mass reformatting goes in its own commit.
- **Pull requests:** use the template. CI must be green.
- **Never commit** Razer or THX files (DLLs, drivers, installers), captures with personal data (full serial numbers, user paths) or an unreviewed `debug.log`.

## 7. Documentation

| When | Update |
|---|---|
| Always | [docs/STATUS.md](docs/STATUS.md) |
| The user will notice | [CHANGELOG.md](CHANGELOG.md) ("Unreleased") |
| Structure, a thread or a file on disk changes | [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) |
| A command or protocol detail is found | [docs/PROTOCOL.md](docs/PROTOCOL.md), with its source |
| Something is learned about Synapse, THX or Windows | [docs/RESEARCH.md](docs/RESEARCH.md), with its source |
| A term appears or changes | [CONTEXT.md](CONTEXT.md) |
| A decision that is hard to undo, with real alternatives | a new ADR in [docs/adr/](docs/adr/) |
| Usage changes | [README.md](README.md) |

## 8. Done means

A change is done when:

- CI is green;
- it was tested as far as possible without the headset, and the rest is in STATUS.md with steps;
- the docs in the table above are up to date;
- the commit says what was verified.

## 9. Releasing

1. Move the "Unreleased" entries in CHANGELOG.md under the new version and date, and bump `version` in `Cargo.toml`.
2. Commit, then tag: `git tag v0.2.0 && git push origin v0.2.0`.
3. The **Release** workflow builds `rzr.exe`, writes its SHA-256 and publishes a draft release with both. Paste the version's CHANGELOG section into the description and publish it.
