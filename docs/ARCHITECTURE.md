# Architecture

How rzr is built. If you change the structure (a new module, another thread, another file on disk), update this document in the same commit.

## One executable, three modes

`rzr.exe` is a single portable file. Depending on how it starts:

| Mode | Started by | What it does |
|---|---|---|
| **Panel** | double-click, `rzr`, `rzr --demo` | Opens the window. While open, it applies the profile when the headset connects (unless the background watcher already does). If the headset is already on, it first adopts the preset the headset is playing (its EQ button may have changed it while rzr was closed). It follows the EQ button and switches THX with it. |
| **Background** | `rzr --silent --watch` (what "Start with Windows" uses) | No window. Watches the dongle and applies the profile on every connect. Sends THX the profile's mic enhancements, which THX forgets on restart ([ADR 0007](adr/0007-mic-enhancements-in-the-profile.md)). With the panel closed, it follows the EQ button: saves the preset in the profile and switches THX ([ADR 0006](adr/0006-eq-like-synapse.md)). Only one can run. |
| **Command line** | `rzr apply`, `rzr import FILE`, `rzr diagnose`, `rzr help` | Does one thing and exits. |

The panel and the background watcher can run together: they share the dongle through a lock (see [Threads](#threads-and-concurrency)).

Only the **supported model** is ever sent a command. If the user picks another model in Settings, the panel and the watcher let go of the dongle, and only the read-only diagnostics run ([ADR 0008](adr/0008-read-only-diagnostics-for-other-models.md)).

## Layers

```
 ui/index.html + app.css + app.js       The page: draws and sends commands
          │  ▲
 commands │  │ state (JSON)             window.ipc.postMessage / rzr.state(...)
          ▼  │
 gui/window.rs                          Window (tao) + WebView (wry: WebView2 / WebKitGTK)
 gui/mod.rs  (App)                      Controller: config, commands → changes, state → page
          │  ▲
 DevCmd   │  │ DevEvent                 channels (mpsc) + a Notify wake-up
          ▼  │
 worker.rs                              Headset thread (polling, events, writes)
                                        Windows audio thread (volume, devices)
                                        THX thread (state and changes through its service)
          │
 device.rs                              Sequences: apply profile, pick preset, write curve
 protocol.rs                            HID frame format and command table
          │
 hidapi → dongle (USB, interface 3, 64 bytes) → headset
```

Each layer only knows the one below. `protocol.rs` knows nothing about threads; `device.rs` nothing about the interface; the page nothing about the protocol.

## Modules

| File | Job |
|---|---|
| `src/main.rs` | Reads the arguments and picks the mode. Holds the `--watch`, `apply` and `diagnose` loops. |
| `src/models.rs` | The headset models rzr knows, their USB IDs and whether they're supported. |
| `src/protocol.rs` | Builds and parses frames; command constants; presets, selectors and reference curves. No I/O. |
| `src/device.rs` | Opens the dongle and runs sequences with retries and read-back. Takes the bus lock. |
| `src/diagnostics.rs` | The read-only report for a model: Razer HID devices and report descriptors, input reports while listening, the supported model's queries, Windows audio and THX. |
| `src/worker.rs` | The panel's threads: headset (poll every 5 s, events every 250 ms, writes), Windows audio and THX. Also the simulated headset of `--demo`. |
| `src/gui/mod.rs` | `App`, the panel's controller: takes `Msg` from the page, changes the config, asks the headset thread to write and builds the state (`App::view`). Runs the diagnostics on their own thread. |
| `src/gui/window.rs` | Window, WebView, the `rzr://` protocol, event loop, delayed save. |
| `src/gui/assets.rs` | The `ui/` files built into the exe (or read from `RZR_UI_DIR`). |
| `src/config.rs` | Profiles and settings; loads, checks (`sanitize`) and saves `config.json`. |
| `src/mic.rs` | The profile's mic enhancements: EQ presets, curves and which THX parameter each one is. No I/O. |
| `src/synapse.rs` | Imports `.synapse4` profiles. |
| `src/winaudio.rs` | Windows volume, mute, device list and default device. |
| `src/thx.rs` | THX state (the JSON in the registry of the headset's output) and its changes: switches over the THX service's COM interface ([ADR 0004](adr/0004-thx-over-com.md)), normalization, levels and THX presets over ZeroMQ ([ADR 0005](adr/0005-thx-over-zeromq.md)), mic enhancements over `IVSSrvSettings` ([ADR 0007](adr/0007-mic-enhancements-in-the-profile.md)). |
| `src/thx/zmtp.rs` | The bit of ZeroMQ needed to talk to the THX service: ZMTP 3.1 greeting and `REQ` socket frames. No libraries. |
| `src/thx/proto.rs` | The THX service's protobuf messages (`Register`, `State`, `SetPreset`, the reply). No I/O. |
| `src/connlog.rs` | Log of link drops (`connection.log`). |
| `src/debuglog.rs` | Optional debug log (`debug.log`) and the `dlog!` macro. |
| `src/instance.rs` | Single-instance mutex for the watcher, "panel open" marker and bus lock. |
| `src/registry.rs` | "Start with Windows" and migration of the old registry settings. |
| `ui/` | The interface. `demo.js` is only used when the page is opened in a browser. |
| `tools/` | Research scripts: `capture-synapse.ps1` (guided capture of Synapse's logs) and `decode-synapse-log.py`. |

## How a change flows

Example: the user picks the CS2 preset.

1. `app.js` sends `{"cmd": "preset", "preset": "csgo"}` with `window.ipc.postMessage`.
2. `window.rs` gets the text, turns it into `Msg::Preset` and calls `App::handle`.
3. `App` changes the profile, schedules the save (500 ms later) and sends `DevCmd::Update(Target, Change::Eq)` to the headset thread. If THX is installed, it also sends `ThxCmd::Eq` to the THX thread with the matching THX preset and curve ([ADR 0006](adr/0006-eq-like-synapse.md)).
4. The headset thread calls `Device::set_eq`, which takes the lock, sends the sequence and reads the preset back.
5. The thread reports back (`DevEvent` + `Notify`). `window.rs` wakes up, `App::pump` collects the event and, since the state changed, the page gets `rzr.state({...})`.
6. `app.js` redraws. The page never assumes a change worked: it shows what the state says.

## The contract between page and Rust

- **Commands (page → Rust):** the `Msg` enum in `src/gui/mod.rs`. JSON with a `cmd` field in snake_case plus its arguments. Unknown commands are ignored and logged in `debug.log`.
- **State (Rust → page):** `App::view()` in `src/gui/mod.rs`. Sent whole every time something changes; the page keeps no state of its own except visual things (open tab, drag in progress, local dialogs).
- **Notifications:** `rzr.toast(text, error)`.
- **Bindings in the HTML:** `data-text`, `data-show`, `data-level-from`, `data-toggle`, `data-slider`, `data-send`, `data-args`, `data-open`, `data-tab` (described at the top of `ui/index.html`). With them, most new features need no JavaScript.

`ui/demo.js` imitates Rust with fixed data. If you change the state or the commands, update it too.

## Threads and concurrency

- **Main thread:** the window's event loop (tao). Everything in `App` runs here; it never blocks on headset I/O.
- **Headset thread:** sole owner of the `Device`. Takes `DevCmd` and batches those that arrive together (several changes are applied once).
- **Audio thread:** talks to Windows Core Audio (COM). Sends volume and device state every 2 s.
- **THX thread:** reads the THX state every 2 s. After changing an option or THX's EQ it waits (up to 6 s) for the service to save it; it's separate so volume doesn't lag meanwhile. Of several curves (or mic settings) arriving together only the last is applied. Sends the mic enhancements when it connects to the service and when they change.
- **Diagnostics thread:** started on demand from Settings; reports its progress through a channel.
- **Across processes:**
  - `Local\rzr_hid_bus`: a lock around every sequence. The panel and the watcher both keep the dongle open; without it, one's query can cut the other's write.
  - `Global\rzr_blackshark_v2_pro`: makes sure only one background watcher runs. The panel checks it so it doesn't duplicate the connection log.
  - `Local\rzr_panel`: exists while a panel is open (not the demo). The watcher checks it so it doesn't follow the EQ button at the same time as the panel.

## Files on disk

| Path | Content |
|---|---|
| `%APPDATA%\rzr\config.json` | Profiles and settings. Written to a `.tmp` and renamed, so it's never left half-written. |
| `%APPDATA%\rzr\connection.log` | Link drops (256 KB at most). |
| `%APPDATA%\rzr\debug.log` | Debug log, when on (4 MB at most; the previous one in `debug.old.log`). |
| `%APPDATA%\rzr\diagnostics\` | Diagnostics reports, one text file per run. |
| `%LOCALAPPDATA%\rzr\webview\` | WebView2 cache. |
| `HKCU\...\CurrentVersion\Run` | The "Start with Windows" entry. |
| `HKCU\SOFTWARE\rzr` | Settings of old versions; only read to migrate them. |

On Linux (development) the config lives in `~/.config/rzr/`.

## Build and CI

- `Cargo.toml`: WebView2 through `wry` + `tao` on Windows; WebKitGTK on Linux. The release profile is tuned for size (`opt-level = "s"`, LTO).
- `.github/workflows/build.yml` (Windows): format, clippy with no warnings, page syntax, tests and build. `rzr.exe` is kept as the `rzr-windows` artifact.
- `.github/workflows/release.yml`: on a `v*` tag, builds `rzr.exe` and publishes a draft GitHub release with it and its SHA-256.

## Tests

Unit tests without hardware (`cargo test`):

- **Protocol:** frames identical to the first release's (which worked) and replies captured from Synapse.
- **Configuration:** range checks, partial JSON, and old settings being ignored.
- **Models and diagnostics:** only the supported model is supported; IDs for the page and config.
- **Synapse importer** and **registry migration**.
- **Connection log.**
- **THX:** parsing the state (from a hand-written JSON) and each output's registry key; protobuf messages against a real reply from the service (`src/thx/testdata/`); ZeroMQ frames against a fake service.
- **Page commands** and **served files.**

What depends on the real headset is tested by hand and recorded in [STATUS.md](STATUS.md).

## Recipe: adding a headset feature

1. **Protocol:** the command constant and frame builder in `protocol.rs`, with a test comparing its bytes with a capture.
2. **Sequence:** a method in `device.rs` that takes the lock and, if possible, reads the value back.
3. **Profile:** a field in `Profile` (`config.rs`), with a default and limits in `sanitize`.
4. **Apply:** add it to `Device::apply_profile` and, if it can change on its own, a `Change` variant in `worker.rs`.
5. **Controller:** a `Msg` variant, its branch in `App::handle` and the value in `App::view`.
6. **Page:** the HTML with its `data-*` bindings, the command in `demo.js` and, if users will notice, a CHANGELOG line.
7. **Docs:** STATUS.md and, if needed, [PROTOCOL.md](PROTOCOL.md).

Adding a whole **model** is a bigger job; see [NEW-HEADSETS.md](NEW-HEADSETS.md).
