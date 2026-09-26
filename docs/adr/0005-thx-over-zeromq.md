# 0005. What COM can't change in THX goes over ZeroMQ, with our own client

- Status: Accepted
- Date: 2026-09-26
- Extends [0004](0004-thx-over-com.md): COM stays for what it already did.

## Context

[ADR 0004](0004-thx-over-com.md) chose the THX service's COM interface, but it can't turn normalization on or change the levels (Bass Boost, Normalization, Voice Clarity). Without the level, Bass Boost at 50 wasn't audible.

Synapse changes all that through the service's other entrance: ZeroMQ on `127.0.0.1:49671`. In 0004 that road was dropped because the service didn't answer. A capture of Synapse's real conversation (2026-09-26) showed what was missing: every request has four parts, including `x-address:`. With it, a script changed the Bass Boost level and it was heard. Details in [RESEARCH.md](../RESEARCH.md#how-settings-reach-thx).

rzr only needs a sliver of ZeroMQ: one `REQ` socket, one service on the same PC, no security (`NULL`) and two messages (`Register` and `State`, plus `SetPreset` for THX presets).

## Decision

- **Split of roads:**
  - Over COM, as before (already verified by ear): the Spatial, Bass Boost and Voice Clarity switches.
  - Over ZeroMQ: normalization and the three levels; later also the THX preset.
- **Our own client, no libraries:**
  - `src/thx/zmtp.rs`: ZMTP 3.1 greeting with `NULL`, the `READY` command and `REQ` socket frames.
  - `src/thx/proto.rs`: the minimal protobuf of those messages.
- **How a setting changes:**
  1. One connection per change, sending `Register` (the service answers with its state).
  2. **That same state** goes back with the next sequence number and the field changed. Fields rzr doesn't understand (room, emitters, curve…) travel byte for byte.
  3. The change is confirmed in the service's reply, then in the registry JSON, as with COM.
- **Service address:** read from its discovery key (`HKLM\SOFTWARE\THX\Discovery`, `thx:sa:service`). Only addresses on this PC are accepted. If the key can't be read, `127.0.0.1:49671` is used.
- **`x-address`:** carries the connection's local address. The service requires the part but never connects to it (checked: it answers even with nobody listening on that port).
- **Originator:** `rzr`, as with COM (the service accepts it).

## Alternatives

- **Everything over ZeroMQ, like Synapse:** one road. But what already works over COM would need re-verifying by ear, and COM is still needed for the microphone (`IVSSrvSettings`). Keeping COM where it works was preferred.
- **The `zeromq` crate (pure Rust):** asynchronous, so it drags in `tokio` or `async-std`. Too heavy for two messages.
- **The `zmq` crate (links C libzmq):** needs a C build (cmake) and complicates cross-compiling for Windows.
- **`prost` for protobuf:** needs `.proto` files and a build step. Here six fields are read or changed and the rest is copied as is.
- **Writing the registry (`,6`/`,7`):** dropped in 0004 and still dropped.

## Consequences

- **No new dependencies:** the exe went from 1 467 392 to 1 536 512 bytes (+68 KB).
- **Undocumented protocol:** if THX changes its messages or field numbers, it has to be captured again. To guard against it:
  - tests use a real reply from the service (`src/thx/testdata/register-reply.bin`);
  - every change is confirmed in the reply and in the registry, and the user is told if it isn't.
- **Whole state on every change:** milliseconds pass between `Register` and `State`, but if another client (an open Synapse) changes something right then, it would be overwritten. The service rejects old sequence numbers, so no half state is left.
- **Only the ZMTP needed:** one request at a time, no automatic reconnection, no security (the service uses `NULL`), and commands between messages (heartbeats) are ignored. If the service asked for another mechanism, the connection fails with a clear message.
