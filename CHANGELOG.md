# Changelog

Changes users notice. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow [SemVer](https://semver.org/). New changes go under "Unreleased".

## [Unreleased]

## [0.2.0] — 2026-09-26

### Added
- A control panel: Sound, Enhancements, Microphone, Power and Settings.
- Equalizer: Game / Movie / Music presets, the esports presets (Apex Legends, Call of Duty, CS2, Fortnite, Valorant) and a draggable 10-band Custom curve.
- Mic monitoring, Do Not Disturb, auto power-off, battery, charging, mute button state, firmware and serial number.
- Windows volume and mute; picking Windows' default output and input device.
- Several profiles; importing Synapse profiles (`.synapse4`) with a button or by dropping the file on the window.
- A connection log (`connection.log`) of link drops, with a summary in Power.
- THX Spatial Audio without Synapse open: Bass Boost, Sound Normalization, Voice Clarity and THX Spatial Audio switches, and the Bass Boost, Normalization and Voice Clarity levels.
- The equalizer is heard: with THX installed, every preset (Game, Movie, Music, Custom and the esports ones) also applies its curve in THX, like Synapse. The curve survives toggling THX Spatial Audio.
- Microphone enhancements without Synapse (THX): EQ with presets (Default, Mic Boost, Broadcast, Conference) and a Custom curve from −12 to +12 dB, volume normalization, vocal clarity, noise reduction and voice gate, each with its level. They're saved in the profile and put back when rzr starts.
- Settings › Headset model: pick your headset. The BlackShark V2 Pro (2023) is supported; the original BlackShark V2 Pro (2020) and other Razer headsets get read-only diagnostics, and rzr never sends them a setting.
- Settings › Diagnostics and `rzr diagnose`: a read-only report (USB descriptors, what the headset sends while you use it, Windows audio and THX) to send when asking for a new model or reporting a problem.
- A debug log (`debug.log`) in Settings › Diagnostics.
- `tools/capture-synapse.ps1`: a guided capture of what Synapse sends, for adding new headsets.

### Changed
- The whole app and its docs are in English.
- A new look of its own: graphite panels, a mint-green accent, Bahnschrift headings, and the tabs in the top bar.
- The panel is a web page in WebView2: `rzr.exe` drops from 8.7 MB to ~1.6 MB and the black flickering is gone.
- Settings are saved in `%APPDATA%\rzr\config.json` (migrated from the registry automatically).
- Double-click opens the panel; `--silent --watch` no longer shows a console.
- rzr no longer changes Windows' default device when the headset connects or at startup: only when you pick one in the panel.

### Removed
- The guided EQ test and the Advanced settings (EQ sending method, releasing remote mode, Synapse start-up frames). rzr always uses the verified sequence.

### Fixed
- The command thought to be "volume" (`0x93`) is the preset selector; `0x9D` is the preset family.
- The panel and the background watcher no longer interrupt each other on the dongle.
- A single lost reply no longer counts as a disconnect.
- The headset's EQ button also switches THX's equalizer with the panel closed, and the panel opens on the preset the headset is playing instead of the profile's.
- Pressing the EQ button right after changing the preset from the panel no longer shows a red "didn't take the change" warning.
- Link drops are detected at once. The headset's notice used to be lost when it arrived in the same packet as a short dongle message, so drops were logged late and shorter than they were.
- Disabled sliders now look disabled.

### Known issues
- Without THX installed, the Custom curve is stored in the headset but isn't heard on some units (see `docs/STATUS.md`).

## [0.1.0] — 2026-03-16

### Added
- First release: sends the headset the EQ preset and curve Synapse would send.
- `--watch`: watches for the headset and applies the settings on connect, single instance.
- `--silent` for Windows startup; battery in the config.
- Default output and input device.
