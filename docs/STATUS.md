# Status

What works, what doesn't and what comes next. **Updated with every change**, in the same commit. The history of how each feature was found and tested is in the git log, [CHANGELOG.md](../CHANGELOG.md) and [RESEARCH.md](RESEARCH.md).

Last update: 2026-09-26 · version 0.2.0 (unreleased)

**Goal:** rzr fully replaces Synapse for the BlackShark V2 Pro, with THX working **without Synapse installed**; then more Razer headsets.

Legend:

- ✅ **Verified on a real headset** (heard or seen by a person)
- 🟡 **Implemented, not confirmed on hardware** (tested in the demo, CI or unit tests)
- ❌ **Doesn't work** (confirmed)

## Supported models

| Model | Dongle | Status |
|---|---|---|
| Razer BlackShark V2 Pro (2023), 2.4 GHz + Bluetooth, USB-C | `1532:0555` | ✅ Supported |
| Razer BlackShark V2 Pro (2020), 2.4 GHz, micro-USB | `1532:0528` | 🟡 Read-only diagnostics; waiting for a report |
| Other Razer headsets | — | 🟡 Read-only diagnostics |

## Headset (2023 model)

| Feature | Status | Notes |
|---|---|---|
| Battery, charging, link, active preset | ✅ | |
| Headset events (EQ button, link, mute) | ✅ | The panel follows the EQ button in under 1 s, also right after a change from the PC ([PROTOCOL](PROTOCOL.md#the-eq-button)). |
| Game / Movie / Music presets | ✅ | Heard. |
| Esports presets (select and write their curve) | 🟡 | Heard through THX (see "EQ like Synapse"); not confirmed on the headset alone. |
| Custom curve on the headset itself | ❌ | Stored and read back exactly, but **not heard** on the test unit. With THX installed it doesn't matter: rzr applies the same curve in THX, which is heard. |
| Mic monitoring (sidetone) | ✅ | On, off and level heard, with the headset's own commands. |
| Do Not Disturb, auto power-off | 🟡 | Commands verified by OpenRazer on this model. |
| Firmware, serial number, dongle firmware | 🟡 | |
| Connection log | 🟡 | A real drop was logged; the late-detection bug it showed is fixed, waiting for the next drop to confirm. |

## Windows and THX

| Feature | Status | Notes |
|---|---|---|
| Windows volume | ✅ | |
| Windows mute | 🟡 | |
| Default output and input device | ✅ | Shows Windows' current default and changes it only when picked in the panel, never on its own. Survives a restart. |
| Profiles, `.synapse4` import (button or drag and drop) | ✅ | |
| Start with Windows / background watcher | 🟡 | |
| Panel (WebView2) | ✅ | Windows 11. |
| THX state in Enhancements (switches, levels, THX preset) | ✅ | Matches the registry; refreshed every 2 s. |
| THX Spatial Audio, Bass Boost, Voice Clarity switches | ✅ | Over COM, Synapse closed; each change confirmed in under 1 s. |
| THX levels and Sound Normalization | ✅ | Over ZeroMQ ([ADR 0005](adr/0005-thx-over-zeromq.md)); 0 ↔ 100 heard. |
| EQ like Synapse: the headset preset also picks THX's preset and curve | ✅ | [ADR 0006](adr/0006-eq-like-synapse.md). Standard and esports presets and dragging the Custom curve, heard; also with the EQ button, with the panel open or closed. The curve is put back after toggling Spatial. |
| Mic enhancements through THX (EQ with presets and Custom, normalization, vocal clarity, noise reduction, voice gate) | ✅ | [ADR 0007](adr/0007-mic-enhancements-in-the-profile.md). Heard by listening to the mic live; put back after a restart without opening the panel. |
| Mic EQ preset on the headset (`0x96`) | 🟡 | Same bytes as Synapse; its own effect isn't noticeable (the EQ heard is THX's). |
| THX without Synapse (reinstalled from its own installers) | ✅ | Everything above keeps working after uninstalling Synapse and restarting. [Installer package](https://github.com/Antoniazog493/blackshark-v2-pro-thx-restore). |
| Debug log | ✅ | Also logs THX changes. |
| Diagnostics report (Settings, `rzr diagnose`) | ✅ | Run against the 2023 model: descriptors, readings, input reports, audio and THX. For another model it sends nothing (checked: no queries to the 2023 dongle). Not yet run on a 2020 model. |
| English interface and new look | 🟡 | Checked in the demo and on Windows 11; waiting for a full pass with the headset on. |

## Next

1. **BlackShark V2 Pro (2020):** collect a diagnostics report and a Synapse capture from a 2020 unit ([NEW-HEADSETS.md](NEW-HEADSETS.md)), then decide whether it speaks the same "PA" protocol or Razer's older 90-byte one.
2. **Release 0.2.0** with `rzr.exe` on GitHub Releases.
3. **Check Windows' audio setup:** warn when audio enhancements are off or Windows Sonic is on, and offer to fix it.
4. **Confirm on hardware** what is 🟡 above.
5. **Windows' own enhancements without THX** (Bass Boost, Loudness), with the registry keys already captured ([RESEARCH.md](RESEARCH.md#windows-own-enhancements)).
6. **A lighter native interface** instead of WebView2 (its cache is ~30 MB). Needs an ADR that replaces [0001](adr/0001-web-ui-in-webview2.md).

## Ideas

- Keep THX's output settings in the profile and apply them on connect (today they're Windows settings, outside the profile).
- Tray icon for the background watcher (battery, preset, open the panel).
- Other models (BlackShark V3, HyperSpeed): OpenRazer describes each with a per-model table; rzr's `models.rs` could grow the same way.
- Mic enhancements with Equalizer APO or RNNoise, for those without THX.
- Import the mic enhancements from `.synapse4` too (`micVolumeNormalization`, `micVoiceClarity`, `ambientNoiseReduction`, `micSensitivity`, `micEqualizer`).
- Low battery warning.

## Technical debt

- Only unit tests; the page ↔ Rust path has no automated test (it's checked by hand and with headless screenshots). A Playwright test over `ui/` + `demo.js` would cover the page.
- `demo.js` copies the shape of `App::view` by hand; if they drift, the demo lies. A sample state could be generated from Rust.
- The default device is changed through PowerShell (an undocumented Windows interface).
- The THX COM interfaces are declared by hand from `VSSrv` 3.2.3.0's type library; if THX changes them, they must be checked again ([ADR 0004](adr/0004-thx-over-com.md)).
- The ZeroMQ client and THX's protobuf are rzr's own and minimal; if THX changes its messages, capture them again ([ADR 0005](adr/0005-thx-over-zeromq.md)).

## Open questions

- Why isn't the headset's own Custom curve heard on the test unit, when OpenRazer's testers heard it? Does it need remote mode left on, or is it only heard without THX installed?
- What does `0x9E` (preset EQ status) really do?
- Does THX keep working if its service (`VSSrv`) is stopped?
- What are `GetParam` parameters 1–3 (COM)?
