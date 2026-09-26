# 0003. THX is controlled through its settings, without shipping its driver

- Status: Accepted. How to write the settings: [ADR 0004](0004-thx-over-com.md). Where users get the driver: [ADR 0009](0009-thx-installer-in-a-separate-repository.md).
- Date: 2026-09-26

## Context

Bass Boost, Sound Normalization, Voice Clarity and THX Spatial Audio aren't done by the headset: the THX engine does them, a Windows audio effect (APO) that Synapse installs on the headset's output. The headset is sold with THX and people want to use it without depending on Synapse. The effect stays installed in Windows and runs inside `audiodg.exe`; what's missing is a way to change its settings.

## Decision

- rzr **doesn't include or ship** THX or Razer files (DLLs, drivers, installers). The effect is installed by Synapse or by the user from its own installers.
- rzr controls THX **by writing the same settings Synapse writes**, where the effect reads them.
- That place was found with a guided Synapse capture (registry snapshots at every step):
  - **Result (2026-09-25):** the whole state is JSON in the registry of the headset's output (`{d5e8f0ab-4de6-4d91-ab21-68868dda6a4a},6`), with a copy per THX preset in `HKCU\Software\THX\SpatialAudio\UserState`. Synapse doesn't write it directly: it asks the THX service (`VSSrv.exe`, part of the driver package) over ZeroMQ. Details in [RESEARCH.md](../RESEARCH.md#where-thx-keeps-its-settings).
  - **How to write**, in order of preference: `spatial-config-util.exe` if it takes options; otherwise the same messages Synapse sends to the service; as a last resort, the registry (needs administrator).
- Until then, the Enhancements tab explains what THX is and how to keep it working (audio enhancements on, Windows Sonic off).

## Alternatives

- **Ship the THX driver with rzr:** breaks Razer's and THX's license, and rzr would stop being a small portable exe.
- **Call THX's DLL directly** (its internal functions): fragile; every Synapse version may change it, and it needs reverse engineering of the binary.
- **Replace THX with our own effect** (e.g. Equalizer APO): possible for the EQ, but it doesn't reproduce THX's spatial sound. Kept as a separate idea.

## Consequences

- **Installation:** THX must have been installed once, by Synapse or from its installers. Without it, rzr can only offer Windows' own enhancements.
- **THX changes:** if Razer moves the settings, the capture has to be repeated.
- **Without Synapse:** Bass Boost kept playing with Synapse closed and Razer's services stopped. The THX service comes with THX's driver package, not Synapse, so relying on it doesn't contradict this decision.
- **Open:** whether the effect needs the THX service running. If it needed a **Razer** service, this decision would be revisited.
