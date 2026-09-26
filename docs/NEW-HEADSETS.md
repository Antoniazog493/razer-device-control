# Adding a headset

rzr only sends commands it has seen Synapse send, or that OpenRazer verified on that exact model ([ADR 0002](adr/0002-verified-sequences-only.md)). So adding a model starts with **reading**, never with trying commands. This page is for owners who want to help, and for developers who turn their data into support.

## If you own the headset

### 1. Send a diagnostics report (5 minutes, nothing is changed)

1. Download `rzr.exe` from the [releases](https://github.com/Antoniazog493/razer-device-control/releases) and run it. Close Razer Synapse first if it's open.
2. Go to **Settings › Headset model** and pick your model, or **Other Razer headset** if it isn't listed.
3. In **Settings › Diagnostics**, click **Run diagnostics**. For 20 seconds, use the headset: turn it off and on, press every button, turn every dial, plug and unplug the charging cable.
4. Click **Send it on GitHub**, choose **Headset report** and attach the file (Open folder shows it).

Without the panel: `rzr diagnose blackshark_v2_pro_2020 --seconds 30` (or `other`).

The report lists every Razer USB device with its HID report descriptors, every message the headset sent on its own while you used it, and your Windows audio devices and THX. It has no serial numbers or personal paths. For a model rzr doesn't support, **rzr sends it nothing**: it only opens the device and listens.

### 2. Record what Synapse does (optional, 15 minutes)

The report shows *how* the headset talks; a Synapse capture shows *what to say*. Synapse writes every command it sends into its logs.

1. Install Razer Synapse and let it set up the headset.
2. Download [`tools/capture-synapse.ps1`](../tools/capture-synapse.ps1) and run it in PowerShell:
   ```
   powershell -ExecutionPolicy Bypass -File .\capture-synapse.ps1
   ```
3. It asks you to change one setting at a time in Synapse (EQ presets, one band of the custom EQ, mic monitoring, auto power-off…) and to press Enter after each. Type `s` to skip an option your headset doesn't have.
4. At the end you get `rzr-capture.zip` on your Desktop.

**Check it before sharing it publicly:** Synapse's logs can include your Windows user name in paths and your headset's serial number. If you'd rather not post it, open the issue and ask for a private way to send it.

## If you're a developer

1. **Identify the protocol** from the report's report descriptors and input reports:
   - A vendor collection (usage page `FF00`) with 64-byte reports and `50 41` ("PA") / `50 49` ("PI") frames: the Audio MXIC protocol of the 2023 model ([PROTOCOL.md](PROTOCOL.md)).
   - 90-byte feature reports: Razer's older protocol, the one most OpenRazer devices use. Check OpenRazer's driver for that model first.
2. **Decode the capture:** `python3 tools/decode-synapse-log.py rzr-capture/` lists, for each step, the frames Synapse sent and what it logged about them.
3. **Add the model** in `src/models.rs` (name, hint, product ID). Keep `supported()` false until every sequence has a source.
4. **Write the sequences** in the protocol and device layers, each with a test comparing its bytes with the capture ([ARCHITECTURE.md › Recipe](ARCHITECTURE.md#recipe-adding-a-headset-feature)). Today `device.rs` speaks to one model; a second one needs a per-model table, like OpenRazer's, rather than copies of the sequences.
5. **Test on the real headset** with the owner, then mark it supported and record what was verified in [STATUS.md](STATUS.md).
