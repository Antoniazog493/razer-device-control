# 0006. The headset preset also picks THX's preset and curve

- Status: Accepted
- Date: 2026-09-26

## Context

The Custom curve is stored in the headset and reads back exactly, but on the test unit it **isn't heard** (see [STATUS.md](../STATUS.md)). The EQ Synapse makes you hear is THX's: a 10-band software EQ with its own presets (`Game Mode`, `Cinema Mode`, `Music Mode`, `Custom`).

Synapse's logs (`ThxV3NativeSubProcess.log`, 2026-09-25) show what it does when a headset preset is picked:

- **Game / Movie / Music:** picks the matching THX preset (`SetRenderPreset`) and sends its curve (`SetRenderEQGains`), which is the same as the headset's factory one.
- **Custom and the five esports presets:** picks THX's `Custom` preset and sends the preset's curve.
- It does **not** send the curve again when Spatial is toggled, although THX keeps another curve per preset with Spatial on. When Spatial changes, the EQ switches to that other curve (often flat).

Details in [RESEARCH.md](../RESEARCH.md#what-synapse-does).

## Decision

- Whenever the profile's EQ changes, rzr sends the headset the same as before and, if THX is installed, also THX's preset and curve, like Synapse. That happens when a preset is picked, the curve is dragged or reset, the profile changes or is applied, or the headset's EQ button is pressed.
- **The EQ button wins over the profile:** with the panel closed, the background watcher follows the button (saves the preset in the profile and switches THX). If no rzr was running, the panel adopts the headset's preset when it opens instead of going back to the profile's.
- **Roads:**
  - THX's preset over ZeroMQ (`thx.sa.SetPreset`, [ADR 0005](0005-thx-over-zeromq.md)), only when it changes;
  - the curve over COM (`SetCurrentModeEQGains`, already verified by ear), read back with `GetCurrentModeEQGains`.
- **Confirmation:** done when the registry JSON shows the preset and the curve. If it already does, nothing is sent.
- **Curves in a row:** if several arrive together (while dragging), only the last one is applied.
- **After toggling Spatial**, rzr applies the last requested curve again. It's the only deliberate difference from Synapse: that way the profile's curve is heard with Spatial on or off.

## Alternatives

- **Only the headset, as before:** the curve isn't heard on the test unit, for reasons still unknown.
- **Only THX, nothing to the headset:** the headset would stay on another preset, and its EQ button and what the panel shows would stop matching. Also, the headset's curve may be audible without THX installed.
- **Curve over ZeroMQ (`State` with `eq_curve`), like Synapse:** possible, but COM was already verified by ear and was preferred where it works ([ADR 0005](0005-thx-over-zeromq.md)).
- **Imitate Synapse with Spatial too** (don't resend the curve): the EQ "vanishes" when Spatial is turned on, which looks like a bug.

## Consequences

- **The EQ is heard** (verified on a real headset, 2026-09-26): standard and esports presets, dragging the Custom curve and toggling Spatial.
- **Custom and the esports presets share THX's `Custom` preset:** picking one overwrites the other's curve in THX. It doesn't matter, since rzr keeps the curves in the profile and sends them again. Synapse does the same.
- **Without THX or its service,** the EQ goes only to the headset, as before.
