// Demo backend, loaded only when index.html is opened in a plain browser
// (rzr.exe doesn't serve this file). It stands in for rzr so the page can be
// designed without the app or the headset: it answers a few commands and
// shows the rest as a toast.
"use strict";

(() => {
  const curves = {
    game: [-3, -3, -4, 0, 5, 5, 4, 1, 0, -1],
    movie: [4, 4, 3, 0, -3, -1, 3, 5, 2, 1],
    music: [2, 2, 1, 1, 2, 3, 3, 3, 1, 0],
    apex_legends: [-1, 0, 0, 0, -1, 0, 2, 3, 2, 3],
    call_of_duty: [-2, 0, 3, 3, 3, 3, 0, 0, 0, 0],
    csgo: [-5, -4, 3, 5, 3, -2, 1, 5, 4, -4],
    fortnite: [-4, 5, 5, 3, -2, 3, 4, 4, -1, 4],
    valorant: [0, 0, 0, 0, 0, 1, 4, 4, 4, -3],
  };
  const labels = {
    game: "Game", movie: "Movie", music: "Music", custom: "Custom",
    apex_legends: "Apex Legends", call_of_duty: "Call of Duty", csgo: "CS2", fortnite: "Fortnite", valorant: "Valorant",
  };
  const list = (ids) => ids.map((id) => ({ id, label: labels[id] }));
  const standard = ["game", "movie", "music", "custom"];
  let custom = [1, -2, 1, -3, 1, -3, -5, 2, 2, 3];
  const micCurves = {
    default: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    mic_boost: [0, 2, 3, 4, 5, 5, 5, 4, 3, 1],
    broadcast: [4, 4, 4, 3, -2, -7, -4, -2, -3, -5],
    conference: [-8, -7, -5, -3, -1, 1, 3, 2, 1, 0],
  };
  let micCustom = [6, 0, 0, 0, 0, 12, 0, 0, 0, -12];
  const selected = { standard: "custom", esports: "apex_legends" };
  const models = [
    { id: "blackshark_v2_pro_2023", name: "Razer BlackShark V2 Pro (2023)", hint: "2.4 GHz + Bluetooth, charges over USB-C · dongle 1532:0555", supported: true },
    { id: "blackshark_v2_pro_2020", name: "Razer BlackShark V2 Pro (2020)", hint: "2.4 GHz only, charges over micro-USB · dongle 1532:0528", supported: false },
    { id: "other", name: "Other Razer headset", hint: "Not listed here · diagnostics look at every Razer device", supported: false },
  ];

  const S = {
    demo: true,
    version: "demo",
    device: {
      product: "Razer BlackShark V2 Pro 2.4", dongle: true, headset: true, busy: false,
      text: "Connected", level: "ok", battery: 76, charging: false, mic_muted: false,
      firmware: "v1.3", dongle_firmware: "2.4.1.0", serial: "DEMO000000000", preset: "Custom",
    },
    model: { id: models[0].id, name: models[0].name, supported: true, options: models },
    diag: { running: false, step: "", summary: null, file: null, error: null },
    profiles: ["Default", "Music"],
    active: 0,
    profile: {
      name: "Default", eq_mode: "standard", preset: "custom", editable: true, has_curve: false,
      curve: custom, sidetone_enabled: false, sidetone_volume: 50, dnd: false,
      auto_off_enabled: true, auto_off_minutes: 30,
    },
    presets: { standard: list(standard), esports: list(["apex_legends", "call_of_duty", "csgo", "fortnite", "valorant"]) },
    eq: { freqs: ["31Hz", "63Hz", "125Hz", "250Hz", "500Hz", "1kHz", "2kHz", "4kHz", "8kHz", "16kHz"], min: -5, max: 5 },
    auto_off_range: [15, 60],
    audio: {
      out: { id: "out", name: "Headphones (Razer BlackShark V2 Pro 2.4)", volume: 80, muted: false },
      in: { id: "in", name: "Microphone (Razer BlackShark V2 Pro 2.4)", volume: 100, muted: false },
      outputs: [
        { id: "out", name: "Headphones (Razer BlackShark V2 Pro 2.4)", default: true },
        { id: "spk", name: "Speakers (Realtek(R) Audio)", default: false },
      ],
      inputs: [{ id: "in", name: "Microphone (Razer BlackShark V2 Pro 2.4)", default: true }],
    },
    settings: {
      autostart: true, debug_log: false, config_path: "C:\\Users\\you\\AppData\\Roaming\\rzr\\config.json",
    },
    thx: {
      service: true, writable: true, busy: false, preset: "Music",
      spatial: false, bass_boost: true, bass_boost_level: 50,
      normalization: false, normalization_level: 100, voice_clarity: false, voice_clarity_level: 100,
    },
    mic: {
      eq_preset: "default",
      presets: [
        { id: "default", label: "Default" }, { id: "mic_boost", label: "Mic Boost" },
        { id: "broadcast", label: "Broadcast" }, { id: "conference", label: "Conference" },
        { id: "custom", label: "Custom" },
      ],
      editable: false, curve: micCurves.default, min: -12, max: 12,
      effects: {
        normalization: { on: false, level: 55, min: 0, max: 100 },
        voice_clarity: { on: true, level: 60, min: 0, max: 100 },
        noise_reduction: { on: false, level: 55, min: 0, max: 100 },
        voice_gate: { on: false, level: -30, min: -40, max: -20 },
      },
      available: true,
    },
    connlog: {
      drops_today: 1,
      lines: [
        "2026-09-26 09:14:07.412  Headset RECONNECTED after 4.1 s  [headset event, panel]",
        "2026-09-26 09:14:03.305  Headset DISCONNECTED  [headset event, panel]",
      ],
    },
  };

  function refreshEq() {
    const p = S.profile;
    p.preset = selected[p.eq_mode];
    p.editable = p.preset === "custom";
    p.has_curve = !p.editable;
    p.curve = p.editable ? custom : curves[p.preset];
    S.device.preset = labels[p.preset];
    // Like rzr with THX installed: the headset preset also picks THX's.
    S.thx.preset = { game: "Game", movie: "Movie", music: "Music" }[p.preset] || "Custom";
  }

  function refreshMic() {
    const m = S.mic;
    m.editable = m.eq_preset === "custom";
    m.curve = m.editable ? micCustom : micCurves[m.eq_preset];
  }

  const handlers = {
    ready() {},
    eq_mode({ mode }) { S.profile.eq_mode = mode; refreshEq(); },
    preset({ preset }) {
      S.profile.eq_mode = standard.includes(preset) ? "standard" : "esports";
      selected[S.profile.eq_mode] = preset;
      refreshEq();
    },
    eq_bands({ bands }) { custom = bands; refreshEq(); },
    eq_reset() { custom = Array(10).fill(0); refreshEq(); },
    eq_copy_to_custom() { custom = [...S.profile.curve]; selected.standard = "custom"; S.profile.eq_mode = "standard"; refreshEq(); },
    select_profile({ index }) { S.active = index; S.profile.name = S.profiles[index]; },
    rename_profile({ name }) { S.profiles[S.active] = S.profile.name = name.trim() || S.profile.name; },
    new_profile() { S.profiles.push("New profile"); S.active = S.profiles.length - 1; S.profile.name = "New profile"; },
    delete_profile() { S.profiles.splice(S.active, 1); S.active = 0; S.profile.name = S.profiles[0]; },
    dnd({ on }) { S.profile.dnd = on; },
    sidetone({ on }) { S.profile.sidetone_enabled = on; },
    sidetone_volume({ value }) { S.profile.sidetone_volume = value; },
    auto_off({ on }) { S.profile.auto_off_enabled = on; },
    auto_off_minutes({ value }) { S.profile.auto_off_minutes = value; },
    thx_spatial({ on }) { S.thx.spatial = on; },
    thx_bass_boost({ on }) { S.thx.bass_boost = on; },
    thx_normalization({ on }) { S.thx.normalization = on; },
    thx_voice_clarity({ on }) { S.thx.voice_clarity = on; },
    thx_bass_boost_level({ value }) { S.thx.bass_boost_level = value; },
    thx_normalization_level({ value }) { S.thx.normalization_level = value; },
    thx_voice_clarity_level({ value }) { S.thx.voice_clarity_level = value; },
    mic_eq_preset({ preset }) { S.mic.eq_preset = preset; refreshMic(); },
    mic_eq_bands({ bands }) { micCustom = bands; refreshMic(); },
    mic_eq_reset() { micCustom = Array(10).fill(0); refreshMic(); },
    mic_effect({ effect, on }) { S.mic.effects[effect].on = on; },
    mic_effect_level({ effect, value }) { S.mic.effects[effect].level = value; },
    volume({ id, value }) { S.audio[id].volume = value; },
    mute({ id, muted }) { S.audio[id].muted = muted; },
    default_device({ id }) {
      for (const list of [S.audio.outputs, S.audio.inputs]) {
        if (list.some((d) => d.id === id)) for (const d of list) d.default = d.id === id;
      }
    },
    autostart({ on }) { S.settings.autostart = on; },
    debug_log({ on }) { S.settings.debug_log = on; },
    headset_model({ model }) {
      const m = models.find((x) => x.id === model);
      S.model = { ...S.model, id: m.id, name: m.name, supported: m.supported };
      S.device.text = m.supported ? "Connected" : "Model not supported yet";
      S.device.level = m.supported ? "ok" : "warn";
    },
    run_diagnostics() {
      S.diag = { running: true, step: "Looking for Razer devices…", summary: null, file: null, error: null };
      let left = 3;
      const tick = setInterval(() => {
        if (left > 0) {
          S.diag.step = `Listening for ${left} s: turn the headset off and on, press its buttons, turn its dials… (${3 - left} reports so far)`;
          left--;
        } else {
          clearInterval(tick);
          S.diag = { running: false, step: "", summary: ["Demo: nothing was read."], file: "rzr-diagnostics-demo.txt", error: null };
        }
        rzr.state(JSON.parse(JSON.stringify(S)));
      }, 1000);
    },
  };

  window.ipc = {
    postMessage(text) {
      const msg = JSON.parse(text);
      const handler = handlers[msg.cmd];
      if (handler) handler(msg);
      else rzr.toast(`(demo) ${msg.cmd}`, false);
      // Like rzr: the answer arrives after the message, not during it.
      setTimeout(() => rzr.state(JSON.parse(JSON.stringify(S))), 0);
    },
  };
})();
