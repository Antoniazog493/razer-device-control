# 0008. Models that aren't supported only get read-only diagnostics

- Status: Accepted
- Date: 2026-09-26
- Retires the guided EQ test mentioned in [0002](0002-verified-sequences-only.md).

## Context

rzr is written for the BlackShark V2 Pro (2023, dongle `1532:0555`). Owners of other Razer headsets want it too; the first candidate is the original BlackShark V2 Pro (2020, dongle `1532:0528`, micro-USB charging). OpenRazer has no support for it, and its USB interface may not speak the same protocol: the 2023 model uses the "Audio MXIC" frames, while most Razer devices use a 90-byte feature-report protocol.

Separately, the panel carried a **guided EQ test** and an **Advanced** card (three ways of sending the EQ, releasing remote mode, Synapse's start-up frames). They were built to find out why one unit's Custom curve wasn't heard. That's settled for users (the EQ that's heard goes through THX, [ADR 0006](0006-eq-like-synapse.md)), the test wrote experimental curves to the headset, and the options meant nothing to anyone else.

## Decision

- **Settings › Headset model** lists the known models. Only supported ones (`HeadsetModel::supported`) are ever sent a command: with any other model picked, the panel and the background watcher let go of the dongle and write nothing.
- **Settings › Diagnostics** (and `rzr diagnose`) produces a text report for the picked model:
  - every Razer HID collection: IDs, strings and its report descriptor;
  - the input reports the model's collections send on their own for 20 s while the user presses buttons and turns the headset off and on: the device is opened and read, never written;
  - for a supported model only, the panel's own read-only queries (link, battery, firmware, EQ);
  - Windows' Razer audio devices, THX on them and whether the THX service answers.
  Serial numbers and device paths are left out, so the report can be posted in a public issue.
- **The guided EQ test and the Advanced card are removed.** The sequence is always the verified one (read-back, remote mode released after each sequence, Synapse's start-up frames), as before.

## Alternatives

- **Try the 2023 sequences on other models:** a query is still a write on the wire; on a device with another protocol it could do anything. Breaks [0002](0002-verified-sequences-only.md).
- **Probe with Razer's standard 90-byte queries** (firmware, serial), which OpenRazer sends to most devices: likely harmless, but still unverified for these models. Can come later, once a report shows which protocol the device speaks.
- **Ask owners to use USBPcap or Wireshark:** accurate, but too hard for most people; the built-in report is one click.
- **Keep the guided test for other models:** it writes curves, which is exactly what an unsupported model mustn't get.

## Consequences

- Owners of other models can help with one click, without risk, and the report is enough to recognise the protocol.
- Adding a model still needs a Synapse capture (`tools/capture-synapse.ps1`) and a per-model table in the device layer ([NEW-HEADSETS.md](../NEW-HEADSETS.md)).
- Old `config.json` files keep their `eq_method`, `release_remote`, `send_legacy_config` and `eq_status` fields; they're ignored.
- If a headset's own EQ turns out to need another sequence, it will come back as a verified sequence for that model, not as a user option.
