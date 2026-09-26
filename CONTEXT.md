# Glossary

The words rzr uses, in the interface, the code, commits and docs. If a term is missing or used with another meaning, clarify it and add it here first.

## Hardware

**Headset**: the Razer BlackShark V2 Pro. It stores its own settings (presets, curves, mic monitoring…) and keeps them without rzr.
_Avoid:_ "device" when you mean just the headset.

**Model** (`HeadsetModel`): which headset the user has. Only **supported** models are ever sent a command; the others get read-only **diagnostics**. Today: BlackShark V2 Pro (2023, dongle `1532:0555`, supported), BlackShark V2 Pro (2020, dongle `1532:0528`) and "other".

**Dongle**: the 2.4 GHz USB receiver. rzr talks to the dongle, which forwards to the headset.

**Link**: the wireless connection between dongle and headset. It can be down while the dongle is plugged in.

**Drop**: a link outage, with its time and duration (connection log).

## Talking to the headset

**Command**: a message from rzr to the headset: a **query** (reads a value) or a **write** (changes one).

**Event**: a message the headset sends on its own: link change, battery, EQ button, mute button.

**Sequence**: the ordered commands a change needs (for example selecting a preset). The order comes from Synapse and OpenRazer.

**Remote mode**: the state in which the headset takes commands from the PC. Every sequence turns it on first.

**Apply**: send the headset everything the active profile says.

**Diagnostics**: a read-only report for a model (USB descriptors, the events it sends, Windows audio), saved as a text file the user can send. It never writes a setting.

## Equalizer

**Curve**: the 10 EQ values, −5 to +5 dB, 31 Hz to 16 kHz.

**Preset**: an EQ setting stored in the headset. Each one has a **slot**.

**Slot**: where the headset stores a preset's curve. Game, Movie and Music have factory slots that can't change; Custom and the esports presets can.

**Family**: the group a preset belongs to. **Standard**: Game, Movie, Music and Custom. **Esports**: Apex Legends, Call of Duty, CS2, Fortnite and Valorant. The headset remembers one preset per family.

**Selector**: the number the headset identifies a preset by.

**Custom**: the preset whose curve the user draws.

## Headset settings

**Mic monitoring** (sidetone): hearing your own voice in the headset.

**Mic enhancements**: microphone EQ, volume normalization, vocal clarity, noise reduction and voice gate. THX's microphone effect does them, not the headset; they're stored in the profile.

**Voice gate**: cuts out what the microphone picks up below a threshold in dB (−40 to −20).

**Do Not Disturb** (`dnd`): blocks phone calls over Bluetooth while the dongle is in use.

**Auto power-off** (`auto_off`): minutes without use before the headset turns off (15 to 60).

## rzr

**Profile**: a named set of all headset settings. One is the **active profile**.

**Panel**: rzr's window.

**Background watcher**: rzr running without a window (`--silent --watch`), applying the profile whenever the headset connects.

**Capture**: a guided session of `tools/capture-synapse.ps1` that records what Synapse sends at each step.

## Windows and THX

**Audio enhancements**: the effects Windows applies to an audio device. If they're off, no effect (THX included) runs.

**Audio effect** (APO): a component that processes sound inside Windows, not in the headset. THX Spatial Audio is one.

**THX**: the audio effect Synapse installs for this headset. It does Bass Boost, Sound Normalization, Voice Clarity, spatial sound and a software EQ. Not part of the headset.
_Avoid:_ "THX driver" for its settings; the driver is just the package that installs it.

**Windows Sonic**: Windows' own spatial sound. It clashes with THX.

**THX preset**: a preset of THX's software EQ: `Game Mode`, `Cinema Mode`, `Music Mode` or `Custom`. Not the same as the headset's preset, although Synapse (and rzr) pick both together: Game, Movie and Music have their own, while Custom and the esports presets use `Custom` with their curve.
_Avoid:_ plain "preset" when talking about THX.

**THX state**: the JSON with all of THX's output settings (spatial, Bass Boost, normalization, voice clarity, THX preset, curve), stored in the registry of the headset's output.

**THX service**: `VSSrv.exe`, installed with the THX driver package. It takes changes (Synapse over ZeroMQ, rzr over COM and ZeroMQ) and stores the THX state. Not a Razer service.
