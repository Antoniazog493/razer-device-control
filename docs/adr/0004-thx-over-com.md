# 0004. THX settings are changed through its service's COM interface

- Status: Accepted. Extended by [0005](0005-thx-over-zeromq.md): normalization and levels over ZeroMQ.
- Date: 2026-09-26
- Completes [0003](0003-control-thx-through-its-settings.md): picks how to write the settings.

## Context

[ADR 0003](0003-control-thx-through-its-settings.md) decided to control THX by writing its settings, shipping nothing from Razer or THX, and left the road open. A local session on a Windows PC with THX found that the THX service (`VSSrv.exe`, from THX's driver package, not Razer's) has two entrances:

- **ZeroMQ**, on `127.0.0.1:49671`: what Synapse uses.
- **A COM server** with a type library (`VSSrv.CVSSrvTHXSettings`, interface `IVSSrvTHXSettings`).

Details in [RESEARCH.md](../RESEARCH.md#how-settings-reach-thx).

## Decision

- rzr changes THX settings with `IVSSrvTHXSettings`: `Init(pid)` and the `Set…State` of Spatial, Bass Boost and Voice Clarity.
- Each change is confirmed by reading the THX state again (the `,6` JSON in the registry of the headset's output) until `sequenceNumber` goes up and the field matches.
- rzr reads that state straight from the registry and never writes it.

## Alternatives

- **ZeroMQ, like Synapse:** would allow everything (also normalization and levels, by sending a whole `thx.sa.State`). But the service didn't answer messages built from what was known then, and speaking it means imitating an undocumented protocol: protobuf, `x-…` prefixes and a registration first. Left for a real capture.
- **Write the registry (`,6`/`,7`):** needs no administrator, but bypasses the service, which owns the state and notifies its clients. They could get out of sync, or it could be overwritten.
- **`spatial-config-util.exe`:** only generates the device configuration; it doesn't change settings.

## Consequences

- **Works without Synapse and without administrator:** tested on a real headset with Synapse closed; Spatial and Voice Clarity were heard.
- **Depends on `VSSrv` 3.2.3.0's type library.** If THX changed the method order, calls would hit another method. To guard against it:
  - the interface is declared whole up to the last method used, in the type library's order;
  - every change is confirmed by reading the state, and the user is told if it isn't.
- **COM's limit:** it can't turn normalization on or change levels. (Solved by [0005](0005-thx-over-zeromq.md).)
- **Without THX or its service**, the Enhancements tab says so and doesn't fail.
