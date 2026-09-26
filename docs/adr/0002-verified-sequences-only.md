# 0002. Only Synapse/OpenRazer sequences, with a lock and read-back

- Status: Accepted. The guided EQ test it mentions was retired by [0008](0008-read-only-diagnostics-for-other-models.md).
- Date: 2026-09-26 (records decisions made on 2026-09-25 and 26)

## Context

The headset has no public documentation. Its commands are known from Synapse captures and from OpenRazer's driver, which verified them on hardware. The firmware has traps: the link drops the first frame after being idle; a preset change across families doesn't reach the preset asked for; a curve is stored in the active preset's slot, whichever it is. Also, the panel and the background watcher open the dongle at the same time.

## Decision

- rzr only sends sequences seen from Synapse or verified by OpenRazer.
- Every sequence:
  - starts with a remote-mode frame that wakes the link;
  - runs inside a cross-process lock (`Local\rzr_hid_bus`);
  - is confirmed by reading the state back when the firmware allows (active preset before writing a curve, retries on a family change).
- Game, Movie and Music are read-only: the headset has factory curves for them.
- When a sequence isn't heard on a headset, variants were tried with the **guided test**, which asked the user and kept the one that worked. The default sequence is never changed blindly.

## Alternatives

- **Write without reading** (like the first release): simpler and faster, but it can write a curve into the wrong slot and damage another preset.
- **Explore new commands from the panel:** risks leaving the headset in an odd state with no way to know why.
- **No cross-process lock:** one process's query cuts the other's write (seen as ignored writes).

## Consequences

- **Safer but slower:** a preset change can take 1–2 s with the reads and retries.
- **Tests:** every new command needs a source (capture or OpenRazer) and a test with its bytes.
- **When something doesn't work**, the path is: debug log, then comparison with OpenRazer ([PROTOCOL.md](../PROTOCOL.md#open-questions)).
