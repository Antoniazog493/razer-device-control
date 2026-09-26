# 0009. THX's installers are offered from a separate repository

- Status: Accepted
- Date: 2026-09-26
- Keeps [0003](0003-control-thx-through-its-settings.md): rzr itself ships nothing from THX or Razer.

## Context

Uninstalling Synapse also uninstalls THX Spatial Audio, and Razer offers no standalone THX installer for this headset. Reinstalling THX from the two installers Synapse leaves in `C:\ProgramData\Package Cache` works without Synapse ([RESEARCH.md](../RESEARCH.md#the-driver-package)), but most users no longer have them once Synapse is gone, and running `msiexec` by hand is not friendly. Community repositories already share THX driver packages for other Razer hardware, with disclaimers.

## Decision

- The two THX installers for this headset ("THX Spatial Audio USB 1532-0555" 3.2.3.0 and "THX V3 APO Presets" 3.2.18.0) are offered in a **separate repository**, [blackshark-v2-pro-thx-restore](https://github.com/Antoniazog493/blackshark-v2-pro-thx-restore), with a double-click installer (it asks for administrator rights, installs both, and offers to restart), an uninstaller, checksums and a clear disclaimer that the files belong to THX Ltd. and Razer Inc.
- rzr's README and its Enhancements tab link to it. rzr's own repository and releases contain no THX or Razer file.

## Alternatives

- **Attach the installers to rzr's releases:** more convenient, but if the rights holders object, a takedown would hit rzr's repository too.
- **Only document how to save them from Synapse before uninstalling it:** the safest legally, but useless to anyone who already uninstalled Synapse. The guide stays in the THX repository's README as the first option.
- **Ship the drivers exported with `pnputil`** instead of the MSIs: works, but leaves nothing in "Installed apps" to uninstall later. The MSIs are what Synapse itself runs.

## Consequences

- rzr's repository stays clean of third-party binaries; a takedown of the THX repository wouldn't affect rzr.
- The THX repository must say plainly what the files are, whose they are, which headset they're for (`1532:0555` only), and how to verify them (Microsoft-signed drivers, SHA-256).
- If Razer or THX ever publish a standalone installer, the links should point there instead.
