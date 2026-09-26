# Headset protocol

How rzr talks to the Razer BlackShark V2 Pro (2023) through its dongle (`1532:0555`). Every entry says where it comes from: a **Synapse capture** (Synapse 4's own logs, which print every HID frame), **OpenRazer** (its driver for this headset, verified on hardware: [openrazer/openrazer#2862](https://github.com/openrazer/openrazer/pull/2862)) or a **hardware test** with rzr.

## Transport

The dongle's USB interface 3 has two HID collections: a consumer-control one (usage page `000C`, the volume keys) and a vendor-defined one (usage page `FF00`, usage `0001`). rzr uses the vendor one: 64-byte reports, report ID `0x02`, both ways. Razer calls the protocol "Audio MXIC".

## Requests ("PA")

```
[0]  0x02       report id
[1]  0x80       direction (host → device)
[2]  total_len  8 + data length
[5]  0x50 'P'   [6] 0x41 'A'
[7]  inner_len  0x08 (0x0E in the remote-mode frame)
[9]  cmd_type   0x02 remote, 0x03 query, 0x04 write, 0x06 dongle, 0x0D EQ
[10] cmd_id
[11] flag       0 in a request (remote mode carries its value here)
[12] data_len
[13] data...
```

## Replies and events ("PI")

A report can pack several "PI" messages: `[0]` is the report ID and `[1]` the total length. Each message is `P` `I`, a type, a sequence number, 4 bytes, a **body size** (2 bytes, little-endian) and the body: command, flag and, in the headset's replies and events, a data length and the data.

- The flag is `01` in the reply to a command and `02` in an **event** the headset sends on its own: link (`20`), battery (`21`), preset (`13`), family (`1D`), Do Not Disturb (`27`), charging (`2A`) and mic mute (`55`).
- Walk the messages by the body size, never by the data length: some bodies have no length (the dongle's `E3` link notice is 2 bytes). _Source: a `debug.log` of 11 123 reports parsed without errors._
- The dongle's `E3` notice (type `0E`): `E3 00` when the link drops and `E3 01` when it's back, just before the matching `20` event.

## Commands

| Function | Write | Read | Data |
|---|---|---|---|
| Remote mode (before every sequence) | `02/E1` | — | 1 = software, 0 = headset (in the flag byte) |
| EQ preset selector | `04/93` | `03/13` | `07` Game, `08` Music, `09` Movie, `FF` Custom; esports: `FA` Apex, `FB` CS2, `FC` Valorant, `FD` Fortnite, `FE` CoD |
| Preset family | `04/9D` | — | 1 = standard, 2 = esports |
| Preset EQ status | `04/9E` | `03/1E` | Synapse sends 0 at start; no audible effect found |
| EQ curve (Custom and each esports preset) | `0D/95` | `03/15` | 10 signed bytes (dB), stored in the active preset's slot |
| Mic EQ preset | `04/96` | — | 0 Default, 1 Mic Boost, 2 Broadcast, 3 Conference, 255 Custom (THX applies the curve) |
| Mic monitoring on/off | `04/98` | `03/18` | 0/1 |
| Mic monitoring level | `04/99` | `03/19` | Synapse's 0–100 maps linearly to 0–14 (50 → 7) |
| Do Not Disturb | `04/A7` | `03/27` | 0/1 |
| Auto power-off | `04/AC` | `03/2C` | minutes (15–60), 0 = never |
| Wireless link | — | `03/20` | 1 = headset linked |
| Battery / charging | — | `03/21` / `03/2A` | 0–100 / ≠0 = charging |
| Mic mute button | — | `03/55` | 1 = muted |
| Firmware / serial | — | `03/02` / `03/00` | |
| Dongle firmware | — | `06/01` + `C2 03 F8 5F 04` | reply with flag `C2`: 4 bytes (e.g. 2.4.1.0) |

> **Corrections to the first rzr release:** the command once called "setVolume" (`0x93`) is the **preset selector** (`255` = `0xFF` = Custom, which is why it seemed to work). `0x9D` ("setEnhancement") only sets the preset family. The "SET_CONFIG" frame `06/01` just asks for the dongle's version.

## Firmware quirks

- **The link dozes** after ~0.3 s without traffic and drops the first frame it gets. Every sequence starts with a sacrificial remote-mode frame. _Source: OpenRazer._
- **Family changes:** a selector that crosses from standard to esports (or back) only switches the family; the headset lands on the preset it remembered for that family. rzr reads the preset back and retries. _Source: OpenRazer._
- **A curve goes into the active preset's slot**, whichever it is. rzr confirms the preset before writing, then sends the selector again so the new curve is heard at once (in one sequence the selector latches the slot's old content). _Source: OpenRazer._
- **Game, Movie and Music** play factory curves. Editing them in Synapse only changes THX's software EQ, so in rzr they're read-only. _Source: Synapse capture._
- **Esports presets:** Synapse writes each one's curve into its slot. rzr does the same.
- **A query ends with remote mode off.** If it lands in the middle of another process's write, the headset ignores the rest of that write; that's why the panel and the background watcher share a lock (`Local\rzr_hid_bus`).
- **Mic monitoring level:** Synapse's scale is linear (50 → 7, 31 → 4). Above 10, rzr reads the level back and falls back to 10 if the headset didn't keep it (10 is the highest OpenRazer saw accepted).

## The EQ button

A short press moves to the next preset of the family: standard 07 → 09 → 08 → FF → 07 (Game, Movie, Music, Custom); esports FA → FE → FB → FD → FC → FA (Apex, CoD, CS2, Fortnite, Valorant). A **long press** switches family and lands on the preset it remembered. A **double press** switches to Bluetooth. Each change comes as event `13` (the preset) and, when the family changes, also `1D` (1 standard, 2 esports). _Source: `debug.log` and hardware test._

A selector from rzr that briefly lands elsewhere also sends event `13` (going from CS2 to Fortnite, the headset stopped on Valorant for a moment before the retry). So the event only means "read the preset": right after a write of rzr's, what counts is what the headset says once the write is done.

## Link drops

Drops (event `20 [0]` then `20 [1]`) usually last ~7 s and the dongle stays connected over USB (Windows logs nothing). They happen with and without any software running, so they're a radio issue, not rzr's. rzr logs each one with its duration in `connection.log`.

## Open questions

- Why does a curve written into Custom read back exactly but isn't heard on some units (tested with and without the remote mode released, and with every sequence variant)? THX's EQ is heard, so rzr applies the curve there too ([ADR 0006](adr/0006-eq-like-synapse.md)). OpenRazer's testers did hear the headset's own EQ.
- What does `0x9E` (preset EQ status) really do? Changing it made no audible difference.
