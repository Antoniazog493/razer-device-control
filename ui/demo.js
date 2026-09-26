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
    game: "JUEGO", movie: "PELÍCULA", music: "MÚSICA", custom: "PERSONALIZADO",
    apex_legends: "APEX LEGENDS", call_of_duty: "CALL OF DUTY", csgo: "CS2", fortnite: "FORTNITE", valorant: "VALORANT",
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

  const S = {
    demo: true,
    version: "demo",
    device: {
      product: "Razer BlackShark V2 Pro 2.4", dongle: true, headset: true, busy: false,
      text: "Conectado", level: "ok", battery: 76, charging: false, mic_muted: false,
      firmware: "1.3.0.8", dongle_firmware: "2.4.1.0", serial: "PM2239H12345678", preset: "PERSONALIZADO",
    },
    profiles: ["Predeterminado", "Música"],
    active: 0,
    profile: {
      name: "Predeterminado", eq_mode: "standard", preset: "custom", editable: true, has_curve: false,
      curve: custom, sidetone_enabled: false, sidetone_volume: 50, dnd: false,
      auto_off_enabled: true, auto_off_minutes: 30,
    },
    presets: { standard: list(standard), esports: list(["apex_legends", "call_of_duty", "csgo", "fortnite", "valorant"]) },
    eq: { freqs: ["31Hz", "63Hz", "125Hz", "250Hz", "500Hz", "1kHz", "2kHz", "4kHz", "8kHz", "16kHz"], min: -5, max: 5 },
    auto_off_range: [15, 60],
    audio: {
      out: { id: "out", name: "Auriculares (Razer BlackShark V2 Pro 2.4)", volume: 80, muted: false },
      in: { id: "in", name: "Micrófono (Razer BlackShark V2 Pro 2.4)", volume: 100, muted: false },
      outputs: [
        { id: "out", name: "Auriculares (Razer BlackShark V2 Pro 2.4)", default: true },
        { id: "spk", name: "Altavoces (Realtek(R) Audio)", default: false },
      ],
      inputs: [{ id: "in", name: "Micrófono (Razer BlackShark V2 Pro 2.4)", default: true }],
      default_out: "out",
      default_in: "",
    },
    settings: {
      autostart: true, debug_log: false, eq_method: "verified", release_remote: true,
      send_legacy_config: true, eq_status: 0, config_path: "C:\\Users\\tu\\AppData\\Roaming\\rzr\\config.json",
    },
    thx: {
      service: true, writable: true, busy: false, preset: "Música",
      spatial: false, bass_boost: true, bass_boost_level: 50,
      normalization: false, normalization_level: 100, voice_clarity: false, voice_clarity_level: 100,
    },
    mic: {
      eq_preset: "default",
      presets: [
        { id: "default", label: "PREDETERMINADO" }, { id: "mic_boost", label: "REFUERZO DE MICRÓFONO" },
        { id: "broadcast", label: "TRANSMISIÓN" }, { id: "conference", label: "CONFERENCIA" },
        { id: "custom", label: "PERSONALIZADO" },
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
        "2026-09-26 09:14:03  DESCONECTADO  (aviso del headset)",
        "2026-09-26 09:14:07  RECONECTADO tras 4 s  (aviso del headset)",
      ],
    },
    wizard: null,
  };

  function refreshEq() {
    const p = S.profile;
    p.preset = selected[p.eq_mode];
    p.editable = p.preset === "custom";
    p.has_curve = !p.editable;
    p.curve = p.editable ? custom : curves[p.preset];
    S.device.preset = labels[p.preset];
    // Like rzr with THX installed: the headset preset also picks THX's.
    S.thx.preset = { game: "Juego", movie: "Película", music: "Música" }[p.preset] || "Personalizado";
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
    new_profile() { S.profiles.push("Perfil nuevo"); S.active = S.profiles.length - 1; S.profile.name = "Perfil nuevo"; },
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
    default_device({ flow, id }) { S.audio[`default_${flow}`] = id; },
    autostart({ on }) { S.settings.autostart = on; },
    debug_log({ on }) { S.settings.debug_log = on; },
    advanced(a) { Object.assign(S.settings, a); delete S.settings.cmd; },
    wizard_open() { S.wizard = { stage: "intro", text: "Demo de la prueba guiada.", steps: ["Pon música.", "Pulsa A y B.", "Responde."] }; },
    wizard_start() {
      S.wizard = {
        stage: "test", title: "1 · Personalizado, cambiando de preset y volviendo",
        explain: "Escribe la curva de graves (A) o de agudos (B) en PERSONALIZADO.",
        buttons: [{ label: "A · GRAVES", playing: false, tried: false }, { label: "B · AGUDOS", playing: false, tried: false }],
        waiting: false, connected: true, readback: null, ready: false,
      };
    },
    wizard_press({ k }) {
      const w = S.wizard;
      w.buttons.forEach((b, i) => (b.playing = i === k));
      w.buttons[k].tried = true;
      w.ready = w.buttons.every((b) => b.tried);
      w.readback = "preset: FF · curva: [5, 5, 5, 4, 0, -5, -5, -5, -5, -5]";
    },
    wizard_answer() {
      S.wizard = { stage: "summary", lines: ["Demo terminada"], verdict: "Así se ve el resultado.", ok: true, eq_status: null };
    },
    wizard_finish() { S.wizard = null; },
    wizard_cancel() { S.wizard = null; },
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
