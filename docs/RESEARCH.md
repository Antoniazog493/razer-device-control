# Research notes

What has been learned about Synapse, THX Spatial Audio and Windows while building rzr, where it came from and how sure it is. The headset's own USB protocol is in [PROTOCOL.md](PROTOCOL.md).

Sources: **Synapse capture** (Synapse 4's logs during a guided session, `tools/capture-synapse.ps1`), **type library** (read from the THX service's binary), **loopback capture** (Wireshark on the ZeroMQ ports), **hardware test** (rzr on a real headset, heard by a person). Dates are when each fact was confirmed.

## What Synapse does

- **THX, not the headset, does most of the "enhancements".** Synapse sends **no USB command** for Bass Boost, Sound Normalization, Voice Clarity, THX Spatial Audio or any mic enhancement; it calls `AudioEffectsTHXV3.setRender…` / `setCapture…` instead. _Synapse capture, 2026-09-25._
- **The EQ Synapse makes you hear is THX's.** With THX installed (all heard, 2026-09-26):
  - **Game / Movie / Music:** the headset gets the selector (`0x93` = 7 / 9 / 8) and the family (`0x9D` = 1); THX gets `SetPreset` (`Game Mode`, `Cinema Mode`, `Music Mode`) and that THX preset's curve.
  - **Custom:** the headset gets selector 255, the family and the curve (`0x95`) on every change; THX gets `SetPreset` `Custom` and **the same curve** (−5 to +5 dB). Reset sends zeros both ways.
  - **Esports:** THX preset `Custom` with the esports preset's curve, like Custom. Synapse only sends `SetRenderPreset` when THX's preset changes; the curve always.
  - **On start**, Synapse writes the curves of the five esports presets and Custom into their slots again and leaves the active preset selected.
  - **Spatial:** THX keeps another curve per preset with Spatial on. Turning it on switched `eqCurve` to flat and back when turned off. Synapse doesn't send the curve again, so its EQ changes when Spatial is toggled; rzr does send it again ([ADR 0006](adr/0006-eq-like-synapse.md)).
  - **EQ button with Synapse open:** Synapse copied the headset's new curve into THX's `Custom` preset, so the sound changes on both sides.
- **THX's presets are not the headset's.** THX has its own 10-band software EQ. Game = `[-3,-3,-4,0,5,5,4,1,0,-1]`, Movie = `[4,4,3,0,-3,-1,3,5,2,1]`, Music = `[2,2,1,1,2,3,3,3,1,0]`, Custom = flat. _`ThxV3NativeSubProcess.log` (`SetRenderPreset` + `SetRenderEQGains`)._
- **Mic monitoring with THX:** Synapse sends the headset `0x98`/`0x99` and also, to THX, `SetStartCaptureStatus(1)` and `SetCaptureSidetoneLevel(slider × 31.62)` (51 → 1613, 100 → 3162). The headset commands alone are enough to hear it with Synapse closed (hardware test, 2026-09-26).
- **Synapse's logs** live in `%LOCALAPPDATA%\Razer\RazerAppEngine\User Data\Logs`: `products_<product id in decimal>_mw*.log` for the headset (every HID frame) and `ThxV3NativeSubProcess.log` for THX. Synapse writes them in batches: use the time printed on each line, not when it appears.

## THX Spatial Audio

- **It's a Windows audio effect (APO)** that Synapse installs on the headset's output ("THX Spatial Audio (BlackShark V2 Pro)"), replacing Windows' own effects. It runs inside `audiodg.exe` (`THXOutAPO-SSE2-v3.dll`, `THXMicAPO-SSE2-v3.dll`).
- **To hear it**, the headset's output needs **audio enhancements on** and **Windows Sonic off** (Windows sometimes turns it on by itself).
- **It keeps working without Synapse.** Bass Boost was still heard with Synapse closed and Razer's services stopped (hardware test). The **THX service** (`VSSrv`) is not Razer's and keeps running without Synapse.

### The driver package

- All signed by Microsoft, version 3.2.3.0: `thxrtapo.inf` (the effect: `THXOutAPO-SSE2-v3.dll`, `THXMicAPO-SSE2-v3.dll`), `thxrtscu.inf` (`spatial-config-util.exe`), `thxrtsvc.inf` (the service: `VSSrv.exe`, `VSHelper.exe`, `VSSrvInit.exe`) and `thxusbapo.inf` (binds it to USB `1532:0555`). Presets: "THX V3 APO Presets BlackSharkV2Pro2023 0555" 3.2.18.0.
- **`thxusbapo.inf` is the driver of the headset's audio interface** (`USB\VID_1532&PID_0555&MI_00`, class Media): it uses Windows' USB audio and adds three software components: the effect (`SWC\VEN_THX&CID_THXAPO`), the service (`VEN_THX&PID_THXSVC`) and the model's configuration (`VEN_THX&PID_THXSCU_15320555`). None of it depends on Synapse.
- **Synapse installs it as two separate programs**, listed in "Installed apps": "THX Spatial Audio USB 1532-0555" 3.2.3.0 (a WiX bundle with an MSI inside) and "THX V3 APO Presets" 3.2.18.0. Their installers are kept in `C:\ProgramData\Package Cache`.
- **Uninstalling Synapse also uninstalls THX**: both programs, the four drivers, `VSSrv.exe` and Razer's services. To keep THX without Synapse, save the two installers from `Package Cache` first (they're deleted on uninstall) and install them again afterwards.
- **Reinstalling THX without Synapse works** (2026-09-26): `msiexec /i` on both MSIs (`wixmsi.msi` with its `cab1.cab` next to it, then the presets' `installer.msi`), as administrator. They don't require Synapse; they run `pnputil /add-driver … /install` on the four INFs and `VSSrv` starts by itself. Afterwards:
  - the headset's output is a **new device with another `{id}`**;
  - the ZeroMQ ports change until the next restart (rzr reads them from `HKLM\SOFTWARE\THX\Discovery`);
  - **a restart is needed**: until then the new output has no THX state and rzr thinks THX isn't installed. After the restart the output changes `{id}` once more and has the state, starting from the copies in `HKCU\Software\THX\SpatialAudio\UserState` (which survive the uninstall).
  - The [separate installer package](https://github.com/Antoniazog493/blackshark-v2-pro-thx-restore) automates exactly this ([ADR 0009](adr/0009-thx-installer-in-a-separate-repository.md)).
- **The dongle has a Razer driver** (`rz0555dev.inf`, interface `MI_03`, the one rzr uses): it's only Windows' standard HID (`input.inf`, `HidUsb`, no filters) plus a co-installer that launches Razer's setup wizard. Without it Windows uses the same generic HID, so rzr sees no difference.

### Where THX keeps its settings

_Source: Synapse capture with registry snapshots at every step, 2026-09-25._

| Place | What's there |
|---|---|
| `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\{id}\Properties` (the headset's output) | `{d5e8f0ab-4de6-4d91-ab21-68868dda6a4a},6` (REG_SZ): **the whole THX state as JSON**. `,7` (REG_BINARY): the same state as protobuf. `,0` = `VSHP` (headphones profile), `,1` = path to `C:\ProgramData\THX\usb\1532\0555\thx_spatial.conf.json`. |
| `HKCU\Software\THX\SpatialAudio\UserState` | A copy of the state per **THX preset**, named `<preset>-<0/1 spatial>-Headphones-usb\1532\0555`, plus `CurrentPreset-…`, `EndpointScope-…` and `UserScope`. |
| `HKLM\SOFTWARE\THX\Discovery` | `thx:sa:service = tcp://127.0.0.1:49671` and `thx:sa:status:publisher = tcp://127.0.0.1:49670`: where the **THX service** listens. |

The JSON (rewritten whole on every change):

```json
{"sequenceNumber":125,"spatialEnabled":false,"hardwareId":"usb\\1532\\0555","outputDevice":"Headphones",
 "presetName":"Music Mode","eqCurve":[0,2,2,2,2,2,2,1,1,1,1,1,1,2,2,2,3,3,3,3,3,3,3,3,3,1,1,1,0,0,0],"tilt":0,
 "spatialProcessingMode":"Headphones5","drcEnabled":false,"drcLevel":100,"emitterPositions":{…},"room":{…},
 "bassBoostEnabled":true,"bassBoost":100,"dialogEnhancementEnabled":false,"dialogEnhancement":100,"upmix":{…}}
```

| Synapse option | Field | Notes |
|---|---|---|
| THX Spatial Audio / Stereo | `spatialEnabled` | also flips `-1-`/`-0-` in `CurrentPreset` |
| Bass Boost | `bassBoostEnabled`, `bassBoost` | level 0–100, kept while off |
| Sound Normalization | `drcEnabled`, `drcLevel` | 0–100 |
| Voice Clarity | `dialogEnhancementEnabled`, `dialogEnhancement` | 0–100 |
| Game / Movie / Music / Custom | `presetName` | `Game Mode`, `Cinema Mode`, `Music Mode`, `Custom`; `room` changes too |
| Equalizer | `eqCurve` | 31 values: a 0, then each of the 10 bands three times |
| — | `sequenceNumber` | goes up on every change |

Reading `,6` through the endpoint's `IPropertyStore` cuts the string at 259 characters (the JSON is ~1 KB), so rzr reads it straight from the registry, which any user can do. The `Properties` key is writable by `Users`, but rzr never writes it: the service owns that state.

### How settings reach THX

_Sources: type library and strings of `VSSrv.exe` and `ThxV3Native`, test calls, loopback capture and hardware tests, 2026-09-26._

- Synapse uses `ThxV3Native` 1.1.42.1, which bundles `thxv3lib` 3.2.0.0 and ZeroMQ 4.3.5, and sends calls such as `SetRenderBassBoost`, `SetRenderBassBoostLevel`, `SetRenderNormalization`, `SetRenderVocalClarity`, `SetRenderSpatialProcessing`, `SetRenderPreset`, `SetRenderEQGains` (wrapped in `SetRenderBatch` 1/0).
- **The THX service** is `VSSrv` (`C:\Windows\System32\VSSrv.exe`, 3.2.3.0): automatic start, runs as `LocalSystem`. It listens on `127.0.0.1:49671` (a ZeroMQ `ROUTER`) and `127.0.0.1:49670` (a `PUB`), stores the state and writes it to the device's registry. The effect itself talks to no one over the network.
- **The service is also a COM server** with a type library (`VSSrvLib` 1.0). `VSSrv.CVSSrvTHXSettings` (`{2CCFC059-A1C5-4408-BCE5-0B23690DA7A2}`) implements `IVSSrvTHXSettings` (`{3B3AF690-70A6-4DDA-BBEA-0B96493E9DA3}`) with `Init(procId)` and `Get…`/`Set…` pairs:

  | Method | Changes |
  |---|---|
  | `SetSpatialProcessingState(originator, enabled, …)` | `spatialEnabled` ✅ heard |
  | `SetBassBoostState(originator, enabled, …)` | `bassBoostEnabled` ✅ |
  | `SetDialogEnhanceState(originator, enabled, …)` | `dialogEnhancementEnabled` ✅ heard |
  | `SetDRCLevel(originator, level, …)` | only `drcLevel`; does **not** turn normalization on |
  | `SetCurrentModeEQGains(originator, float[31], …)` | the active THX preset's `eqCurve` ✅ heard at once. Same layout as `eqCurve`. `GetCurrentModeEQGains` reads it back. |
  | `SetProcessingMode`, `SetCustomRoomType`, `SetListeningMode`, `SetTiltEnabled`, `SetParam` | untested |

  - Any user can create the object and call it (no administrator). After a `Set…` the service rewrites `,6` and `,7` and bumps `sequenceNumber` 0.3–3 s later, and publishes it on the `PUB` socket (topic `thx:sa:state`).
  - COM can't turn normalization on or change the levels: that takes ZeroMQ.
  - Changes apply live: the effect registers with the service (`IVSSrvTHXOutMFXAPO::NotifyAPOInit`), the service signals it, and the effect asks for the state (`GetSystemState`).
- **Other interfaces** in the type library: `IVSSrvSettings` (`{392375FF-8204-4A26-B4C5-AF4B71ACA729}`, class `{087E4DB6-0519-4A63-9099-9201915E1371}`) for the microphone (see [Microphone](#microphone)); `IVSSrvTHXOutMFXAPO`, `IVSSrvInMFXAPO`, `IVSSrvOutMFXAPO` for the effects; `IVSSrvAudioSession`, `IVSSrvRS3DSettings`.
- **Array sizes** in the type library (`LoadTypeLibEx` → `ARRAYDESC`): `float[31]` for the output EQ, `float[10]` for the mic EQ, `inbandMessage` `byte[4096]`, `deviceId` `ushort[80]`. When calling from C# (`Add-Type`), declare arrays with `[MarshalAs(UnmanagedType.LPArray)]`; as `SAFEARRAY` the call corrupts memory.
- **ZeroMQ, as Synapse speaks it** (loopback capture):
  - ZMTP 3.1, `NULL` mechanism. The client is a `REQ` with an empty identity; the service answers as `REP`.
  - Each request has **four parts**: the empty `REQ` delimiter, `x-address:tcp://127.0.0.1:<port>`, `x-originator:<name>` and `x-payload:<THXMessage>`. `x-address` is required, but the service never connects to it; `x-originator:rzr` is accepted.
  - The payload is protobuf `thx.sa.THXMessage { Any msg = 1; string originator = 2; }`. The client registers with `thx.sa.Register { uint32 pid = 1; }` and gets the current state back.
  - A change is a whole `thx.sa.State` with the next `sequence_number`. Its fields: 1 `sequence_number`, 2 `spatial_enabled`, 3 `hardware_id`, 4 `output_device`, 5 `preset_name`, 6 `eq_curve` (31 packed doubles), 7 `tilt`, 8 `spatial_processing_mode`, 9 `drc_enabled`, 10 `drc_level`, 11 `emitter_positions`, 12 `room`, 13 `bass_boost`, 14 `dialog_enhancement`, 15 `upmix`, 16 `Headphones5`, 17 `bass_boost_enabled`, 18 `dialog_enhancement_enabled`. `,7` in the registry is the same message after an 8-byte `VT_BLOB` header.
  - The reply is `thx.sa.StateChangeResult { uint32 status = 1; string msg = 2; State state = 3; }` (`status` 0 = accepted, "State successfully changed").
  - To pick a preset: `thx.sa.SetPreset { sequence_number = 1; PresetKey key = 3; }` with `PresetKey { name = 1; user = 2; spatial_enabled = 3; output_device = 4; hardware_id = 5; }`, then a whole `State` with the curve. The definitions are embedded in `VSSrv.exe` (package `thx.sa`), which also has `PresetService`, `RoomService` and `EmitterService`.
  - To capture it again: `tshark -i \Device\NPF_Loopback -f "tcp port 49671 or tcp port 49670"` (Npcap; no administrator needed).
- **`spatial-config-util.exe` isn't for settings**: it's a Go program that generates the device configuration (`thx_spatial.conf.json`, filters and presets in `C:\ProgramData\THX`).
- **Sound Normalization is hard to notice with music**; it's clearly audible with high-contrast audio (quiet dialogue, loud effects).

## Microphone

- **Without THX**, Synapse's mic enhancements (EQ, normalization, vocal clarity, noise reduction, voice gate) send nothing at all. **With THX** they go to its engine: `SetCaptureNormalization(+Level)`, `SetCaptureVocalClarity(+Level)`, `SetCaptureNoiseReduction(+Level)`, `SetCaptureVoiceGate(+Level, dB −40 to −20)`, `SetCaptureEQGains` (10 bands), `SetCapturePreview`, `SetCaptureSidetoneLevel`, `SetCaptureVolumeBySystem`. _Synapse capture._
- **They travel over COM** to `IVSSrvSettings`, not ZeroMQ, and they're not in the registry: after a Synapse session, `GetMicEQGains` returned the last preset's curve. `GetMicParams` answers 0–17 as (switch, level) pairs:

  | Parameters | Enhancement | Level |
  |---|---|---|
  | 14 / 15 | Volume normalization | 0–100 |
  | 10 / 11 | Vocal clarity | 0–100 |
  | 6 / 7 | Noise reduction | 0–100 |
  | 2 / 3 | Voice gate | dB, −40 to −20 |

  Unidentified (never changed): 0/1 = (0, 200), 4/5 = (0, 10), 8/9 = (0, 2), 12/13 = (0, 40), 16/17 = (1, 40). Vocal clarity is switched off, set and switched on again to change its level.
- **Mic EQ presets:** Synapse sends the headset `0x96` (Default 0, Mic Boost 1, Broadcast 2, Conference 3, Custom 255) and THX the curve: Mic Boost `[0,2,3,4,5,5,5,4,3,1]`, Broadcast `[4,4,4,3,-2,-7,-4,-2,-3,-5]`, Conference `[-8,-7,-5,-3,-1,1,3,2,1,0]`, Default flat. Custom goes from −12 to +12 and only to THX.
- **Where THX keeps them:** nowhere found (not in `HKCU\Software\THX`, `HKLM\SOFTWARE\THX` or the microphone's registry); most likely only in the service's memory. Synapse keeps them per profile and sends them on start, which is why rzr keeps them in the profile ([ADR 0007](adr/0007-mic-enhancements-in-the-profile.md)). Confirmed: after a restart without Synapse, rzr's background watcher puts them back.
- **To hear them**, listen to the microphone live ("Listen to this device" in Windows, or any app): mic monitoring doesn't go through them.

## Windows' own enhancements

Captured in `FxProperties\{b13412ee-07af-4c57-b08b-e327f8db085b}\User` of the output device, with Synapse uninstalled. Not used by rzr yet.

| Option | Key | Values |
|---|---|---|
| Bass Boost | `{1864a4e0-efc1-45e6-a675-5786cbf3b9f0},4` (VT_UI4) | 2 on / 0 off; the first time it also writes `{61e8acb9-…},4 = 80` and `{ae7f0b2a-…},3 = 1` |
| Loudness Equalization | `{fc52a749-4be9-4510-896e-966ba6525980},3` (VT_BOOL) | release time in `{9c00eeed-…},3 = 4` |
| Virtual surround | — | changes the device's channel format, not FxProperties |
| Disable all enhancements | `PKEY_AudioEndpoint_Disable_SysFx` `{1da5d803-d492-4edd-8c23-e0c0ffee7f0e},5` (DWORD) | 1 disabled / 0 enabled |

For the microphone without THX, the alternatives are [Equalizer APO](https://sourceforge.net/projects/equalizerapo/) (EQ) or NVIDIA Broadcast / RNNoise (noise).

## Interface

- egui with OpenGL showed black frames on Windows; with Direct3D 12 the exe weighed 8.7 MB. Slint came to ~8.5 MB. WebView2 keeps the exe at ~1.5 MB ([ADR 0001](adr/0001-web-ui-in-webview2.md)).
- WebView2 leaves a ~30 MB cache in `%LOCALAPPDATA%\rzr\webview`.
- To screenshot the panel even when covered, use `PrintWindow` with `PW_RENDERFULLCONTENT` (2), after `SetProcessDPIAware` so scaled displays aren't cropped.
