# 0007. Mic enhancements live in the profile and rzr sends them to THX again

- Status: Accepted
- Date: 2026-09-26

## Context

The mic enhancements (EQ with presets, normalization, vocal clarity, noise reduction and voice gate) aren't done by the headset: THX's microphone effect does them. Synapse sends them to the THX service (`VSSrv`), which takes them over COM in `IVSSrvSettings` (`SetMicEQGains`, `SetMicParams`). The headset only gets the mic EQ preset's index (`0x96`). Details in [RESEARCH.md](../RESEARCH.md#microphone).

Unlike the output settings (the JSON in the registry of the headset's output), **nobody found where THX keeps the microphone's**. They're not in `HKCU\Software\THX`, `HKLM\SOFTWARE\THX` or the microphone's registry. Most likely they live only in the service's memory and Synapse sends them every time it starts; without Synapse they'd be lost on restart. Synapse also keeps them **per profile** (`micVolumeNormalization`, `micVoiceClarity`, `ambientNoiseReduction`, `micSensitivity`, `micEqualizer`).

## Decision

- **Where they're kept:** in rzr's profile (`Profile::mic`), like Synapse.
- **When they're sent to THX:**
  - when they change, or the profile changes;
  - every time rzr connects (or reconnects) to the THX service: when the panel opens and in the background watcher, for example after Windows restarts.
- **What's sent:** rzr reads what the service has and sends only what differs, then reads it back to confirm. Like Synapse, it switches vocal clarity off and on again to change its level.
- **To the headset** goes `0x96` with the mic EQ preset (Default 0, Mic Boost 1, Broadcast 2, Conference 3, Custom 255), with Synapse's bytes.
- **Reconnection:** rzr notices the service restarted with a cheap call (`GetMicPreviewState`), reconnects and sends them again.
- **Not resent on every poll.** If Synapse were open, the two would fight over them.

## Alternatives

- **Only in THX, live** (like the output enhancements): simpler, but if THX doesn't keep them they're lost on restart, and without Synapse nobody puts them back.
- **Resend them on every poll:** would cover any loss, but would fight an open Synapse. Noticing a service restart is enough for the real case (a PC or service restart).
- **Keep them outside the profile** (one global setting): doesn't match Synapse, where each profile has its own.

## Consequences

- **They work without Synapse** (verified by ear on 2026-09-26, listening to the mic live): EQ presets, the custom curve (−12 to +12 dB), the voice gate and its threshold, normalization, vocal clarity and noise reduction. They're also put back after a restart without opening the panel.
- **When the panel opens or the watcher starts, the profile wins.** Whatever was set from Synapse is replaced by the profile's.
- **`IVSSrvSettings` is declared by hand**, with the sizes in `VSSrv` 3.2.3.0's type library (`float[10]` for the curve). If THX changes it, it must be checked, like `IVSSrvTHXSettings` ([ADR 0004](0004-thx-over-com.md)).
- **The `.synapse4` importer doesn't bring** the mic enhancements yet.
