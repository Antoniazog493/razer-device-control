// rzr panel. rzr calls rzr.state(state) whenever something changes and
// rzr.toast(text, error) for notifications; the page answers with
// send(cmd, args), which reaches rzr as {"cmd": cmd, ...args}. The state's
// shape is App::view() in src/gui/mod.rs; the commands are its Msg enum.
"use strict";

let S = null; // latest state from rzr
let tab = "sound";
let dialog = null; // "rename" | "delete" | null (the guided test comes from S.wizard)

const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];

function send(cmd, args = {}) {
  window.ipc.postMessage(JSON.stringify({ cmd, ...args }));
}

/** Value at a dotted path in the state ("audio.out.name"). */
function get(path, obj = S) {
  return path.split(".").reduce((o, k) => (o == null ? undefined : o[k]), obj);
}

/** "a.b" is truthy, "!a.b" is falsy. Empty lists count as false. */
function test(expr) {
  const neg = expr.startsWith("!");
  const v = get(neg ? expr.slice(1) : expr);
  const t = Array.isArray(v) ? v.length > 0 : !!v;
  return neg ? !t : t;
}

function display(v) {
  if (v === true) return "Sí";
  if (v === false) return "No";
  return v == null || v === "" ? "—" : String(v);
}

function esc(text) {
  return String(text).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
}

function setChecked(el, on) {
  el.setAttribute("role", "switch");
  el.setAttribute("aria-checked", on ? "true" : "false");
}

window.rzr = {
  state(state) {
    S = state;
    render();
  },
  toast,
};

// ------------------------------------------------------------------ render

function render() {
  if (!S) return;
  for (const el of $$("[data-text]")) el.textContent = display(get(el.dataset.text));
  for (const el of $$("[data-show]")) el.hidden = !test(el.dataset.show);
  for (const el of $$("[data-level-from]")) el.setAttribute("data-level", get(el.dataset.levelFrom) ?? "");
  for (const el of $$(".toggle[data-toggle]")) setChecked(el, !!get(el.dataset.toggle));
  for (const el of $$(".slider[data-slider]")) {
    setSlider(el, get(el.dataset.slider), !el.dataset.enabled || test(el.dataset.enabled));
  }

  renderTabs();
  renderProfiles();
  renderBattery();
  renderEq();
  renderAudio();
  renderPower();
  renderSettings();
  renderDialog();
}

function renderTabs() {
  for (const b of $$("[data-tab]")) b.setAttribute("aria-selected", b.dataset.tab === tab ? "true" : "false");
  for (const p of $$("[data-page]")) p.hidden = p.dataset.page !== tab;
  // Sizes are only known once the page is visible.
  for (const el of $$(`[data-page="${tab}"] .slider`)) paintSlider(el);
  if (tab === "sound") drawEq();
}

function setTab(name) {
  tab = name;
  try { localStorage.setItem("rzr.tab", name); } catch (_) {}
  renderTabs();
}

/** Rebuild a <select>'s options only when they changed (keeps it open otherwise). */
function fillSelect(select, options, value) {
  const key = JSON.stringify(options);
  if (select.dataset.key !== key) {
    select.dataset.key = key;
    select.innerHTML = options.map(([v, label]) => `<option value="${esc(v)}">${esc(label)}</option>`).join("");
  }
  select.value = value;
}

function renderProfiles() {
  fillSelect($("#profile"), S.profiles.map((name, i) => [i, name]), S.active);
  $("#delete-profile").disabled = S.profiles.length < 2;
}

function renderBattery() {
  const d = S.device;
  const mini = $(".battery-mini");
  $("#battery-text").textContent = d.battery == null ? "—" : `${d.charging ? "⚡ " : ""}${d.battery}%`;
  $(".battery-fill", mini).style.width = `${d.battery ?? 0}%`;
  mini.classList.toggle("charging", d.charging === true);
  mini.classList.toggle("low", d.battery != null && d.battery <= 15);
  $(".headset").classList.toggle("lit", d.headset);
}

function renderAudio() {
  for (const [flow, list] of [["out", "outputs"], ["in", "inputs"]]) {
    const ep = S.audio[flow];
    if (ep) {
      setChecked($(`#${flow}-mute`), !ep.muted);
      setSlider($(`#${flow}-volume`), ep.volume, !ep.muted);
    }
    const current = S.audio[`default_${flow}`];
    const options = [["", "No cambiar"]];
    if (current && !S.audio[list].some((d) => d.id === current)) options.push([current, "(dispositivo no conectado)"]);
    for (const d of S.audio[list]) options.push([d.id, d.default ? `${d.name}  (actual)` : d.name]);
    fillSelect($(`[data-devices="${flow}"]`), options, current);
  }
  const m = S.device.mic_muted;
  const state = $("#mic-state");
  state.setAttribute("data-level", m === true ? "error" : m === false ? "ok" : "");
  state.lastElementChild.textContent =
    m === true ? "SILENCIADO con el botón del headset" : m === false ? "Activo (botón del headset)" : "Estado del botón de silencio desconocido";
}

function renderPower() {
  const d = S.device;
  $("#battery-percent").textContent = d.battery == null ? "—" : `${d.battery}%`;
  const state = $("#battery-state");
  const [text, level] = !d.headset ? ["Headset no conectado", ""] : d.charging ? ["⚡ Cargando", "ok"] : ["Con batería", ""];
  state.textContent = text;
  state.setAttribute("data-level", level);
  const meter = $("#battery-meter");
  meter.hidden = d.battery == null;
  meter.firstElementChild.style.width = `${d.battery ?? 0}%`;
  meter.classList.toggle("low", d.battery != null && d.battery <= 15);

  const n = S.connlog.drops_today;
  const drops = $("#drops");
  drops.textContent = n === 0 ? "Sin caídas hoy" : n === 1 ? "1 caída hoy" : `${n} caídas hoy`;
  drops.setAttribute("data-level", n === 0 ? "ok" : "warn");
  const lines = S.connlog.lines;
  $("#connlog").innerHTML = lines.length
    ? lines
        .map((l) => `<span class="${l.includes("DESCONECTADO") ? "down" : l.includes("RECONECTADO") ? "up" : ""}">${esc(l)}</span>`)
        .join("\n")
    : "Todavía no hay registros.";
}

function renderSettings() {
  $("#eq-method").value = S.settings.eq_method;
  setChecked($("#release-remote"), S.settings.release_remote);
  setChecked($("#legacy-config"), S.settings.send_legacy_config);
}

function sendAdvanced(change) {
  send("advanced", {
    eq_method: S.settings.eq_method,
    release_remote: S.settings.release_remote,
    send_legacy_config: S.settings.send_legacy_config,
    ...change,
  });
}

// ------------------------------------------------------------------ sliders

/**
 * Build a slider inside <div class="slider">. Attributes: data-min,
 * data-max, data-step, data-unit (appended to the value), data-labels
 * ("left,right"). With data-slider/data-send it's bound to the state and
 * sends {cmd, value, commit: true} when released.
 */
function makeSlider(el) {
  const [left, right] = (el.dataset.labels || ",").split(",");
  el.innerHTML = `<span class="bubble"></span><input type="range"><div class="labels"><span>${esc(left)}</span><span>${esc(right)}</span></div>`;
  const input = $("input", el);
  input.min = el.dataset.min ?? 0;
  input.max = el.dataset.max ?? 100;
  input.step = el.dataset.step ?? 1;
  input.addEventListener("pointerdown", () => (el.dataset.active = "1"));
  for (const end of ["pointerup", "pointercancel", "blur"]) input.addEventListener(end, () => delete el.dataset.active);
  input.addEventListener("input", () => {
    el.dataset.active = "1";
    paintSlider(el);
  });
  input.addEventListener("change", () => {
    delete el.dataset.active;
    if (el.dataset.send) send(el.dataset.send, { value: Number(input.value), commit: true });
  });
}

function paintSlider(el) {
  const input = $("input", el);
  const t = (input.value - input.min) / (input.max - input.min || 1);
  input.style.setProperty("--fill", `${t * 100}%`);
  const bubble = $(".bubble", el);
  bubble.textContent = `${input.value}${el.dataset.unit ?? ""}`;
  // Follow the knob (7px = half its width), without leaving the track.
  const x = 7 + t * (input.clientWidth - 14);
  bubble.style.left = `${Math.min(Math.max(x, bubble.offsetWidth / 2), el.clientWidth - bubble.offsetWidth / 2)}px`;
}

function setSlider(el, value, enabled) {
  el.classList.toggle("disabled", !enabled);
  const input = $("input", el);
  // Leave it alone while the user is dragging it.
  if (!el.dataset.active && value != null) input.value = value;
  paintSlider(el);
}

// ----------------------------------------------------------------- equalizer

const REGIONS = [
  [0, 0, "SUBGRAVES"],
  [1, 2, "GRAVES"],
  [3, 3, "MED. BAJOS"],
  [4, 5, "MEDIOS"],
  [6, 7, "MED. ALTOS"],
  [8, 9, "AGUDOS"],
];

const eq = {
  bands: [],
  editable: false,
  drag: null, // band being dragged
  hover: null,
  // After a drag, states for the same preset may still carry the old curve
  // until rzr has handled our change: ignore those for a moment.
  hold: 0,
  holdPreset: null,
  plot: null,
};

function renderEq() {
  const p = S.profile;
  for (const b of $$("#eq-mode button")) b.setAttribute("aria-pressed", b.dataset.mode === p.eq_mode ? "true" : "false");
  const list = S.presets[p.eq_mode];
  const box = $("#presets");
  const key = JSON.stringify(list);
  if (box.dataset.key !== key) {
    box.dataset.key = key;
    box.innerHTML = list.map((x) => `<button data-preset="${x.id}">${esc(x.label)}</button>`).join("");
  }
  for (const b of $$("button", box)) b.setAttribute("aria-pressed", b.dataset.preset === p.preset ? "true" : "false");

  eq.editable = p.editable;
  $("#eq").classList.toggle("editable", p.editable);
  const same = JSON.stringify(p.curve) === JSON.stringify(eq.bands);
  const held = p.preset === eq.holdPreset && Date.now() < eq.hold && !same;
  if (eq.drag === null && !held) eq.bands = [...p.curve];
  drawEq();
}

function drawEq() {
  const box = $("#eq");
  const w = box.clientWidth;
  const h = 320;
  if (!S || !w || !eq.bands.length) return;
  const { min, max, freqs } = S.eq;
  const n = eq.bands.length;
  const plot = { l: 4, r: w - 64, t: 34, b: h - 70 };
  const dx = (plot.r - plot.l) / n;
  eq.plot = { ...plot, dx };
  const x = (i) => plot.l + dx * (i + 0.5);
  const y = (db) => plot.t + ((max - Math.max(min, Math.min(max, db))) / (max - min)) * (plot.b - plot.t);
  const lit = eq.drag ?? eq.hover;

  let svg = "";
  for (const db of [max, 0, min]) {
    svg += `<text class="scale" x="${plot.r + 22}" y="${y(db)}">${db > 0 ? "+" : ""}${db}dB</text>`;
  }
  for (let i = 0; i < n; i++) {
    const on = lit === i ? " lit" : "";
    svg += `<line class="band-line${on}" x1="${x(i)}" x2="${x(i)}" y1="${plot.t}" y2="${plot.b}"/>`;
    svg += `<circle class="zero" cx="${x(i)}" cy="${y(0)}" r="2"/>`;
    svg += `<text class="freq${on}" x="${x(i)}" y="${plot.b + 18}">${freqs[i]}</text>`;
  }
  const points = eq.bands.map((db, i) => `${x(i)},${y(db)}`).join(" ");
  svg += `<polyline class="curve" points="${points}"/>`;
  eq.bands.forEach((db, i) => {
    svg += `<circle class="node" cx="${x(i)}" cy="${y(db)}" r="${eq.drag === i ? 9 : eq.editable ? 7 : 6}"/>`;
  });
  if (lit !== null && lit !== undefined) {
    const v = eq.bands[lit];
    const label = `${v > 0 ? "+" : ""}${v} dB`;
    const bw = label.length * 6.5 + 10;
    const cy = y(v) - 22;
    svg += `<g class="bubble"><rect x="${x(lit) - bw / 2}" y="${cy - 9}" width="${bw}" height="18" rx="2"/><text x="${x(lit)}" y="${cy}">${label}</text></g>`;
  }
  const barTop = plot.b + 36;
  for (const [a, b, label] of REGIONS) {
    const on = lit != null && lit >= a && lit <= b ? " lit" : "";
    const left = x(a) - dx / 2 + 1;
    const width = x(b) + dx / 2 - 1 - left;
    svg += `<g class="region${on}"><rect x="${left}" y="${barTop}" width="${width}" height="24"/><text x="${left + width / 2}" y="${barTop + 12}">${label}</text></g>`;
  }
  box.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}">${svg}</svg>`;
}

function eqBandAt(clientX) {
  const r = $("#eq").getBoundingClientRect();
  const i = Math.floor((clientX - r.left - eq.plot.l) / eq.plot.dx);
  return Math.max(0, Math.min(eq.bands.length - 1, i));
}

function eqDbAt(clientY) {
  const r = $("#eq").getBoundingClientRect();
  const { min, max } = S.eq;
  const t = (clientY - r.top - eq.plot.t) / (eq.plot.b - eq.plot.t);
  return Math.round(Math.max(min, Math.min(max, max - t * (max - min))));
}

function setupEq() {
  const box = $("#eq");
  box.addEventListener("pointerdown", (e) => {
    if (!eq.editable || !eq.plot || e.button !== 0) return;
    box.setPointerCapture(e.pointerId);
    eq.drag = eqBandAt(e.clientX);
    eq.bands[eq.drag] = eqDbAt(e.clientY);
    drawEq();
  });
  box.addEventListener("pointermove", (e) => {
    if (!eq.plot) return;
    if (eq.drag !== null) {
      const db = eqDbAt(e.clientY);
      if (db !== eq.bands[eq.drag]) {
        eq.bands[eq.drag] = db;
        drawEq();
      }
      return;
    }
    const r = box.getBoundingClientRect();
    const inside = e.clientY - r.top <= eq.plot.b + 60;
    const hover = inside ? eqBandAt(e.clientX) : null;
    if (hover !== eq.hover) {
      eq.hover = hover;
      drawEq();
    }
  });
  const release = () => {
    if (eq.drag === null) return;
    eq.drag = null;
    eq.hold = Date.now() + 1000;
    eq.holdPreset = S.profile.preset;
    send("eq_bands", { bands: eq.bands });
    drawEq();
  };
  box.addEventListener("pointerup", release);
  box.addEventListener("pointercancel", release);
  box.addEventListener("pointerleave", () => {
    if (eq.drag === null && eq.hover !== null) {
      eq.hover = null;
      drawEq();
    }
  });
  new ResizeObserver(() => {
    drawEq();
    for (const el of $$(".slider")) paintSlider(el);
  }).observe($("main"));
}

// ------------------------------------------------------------------ dialogs

function renderDialog() {
  const modal = $("#dialog");
  const box = $(".dialog", modal);
  const kind = S.wizard ? "wizard" : dialog;
  modal.hidden = !kind;
  if (!kind) {
    box.dataset.kind = "";
    return;
  }
  if (kind === "wizard") {
    box.className = "dialog";
    box.innerHTML = wizardHtml(S.wizard);
  } else if (box.dataset.kind !== kind) {
    // Local dialogs are built once, so typing isn't interrupted.
    box.className = "dialog small";
    if (kind === "rename") {
      box.innerHTML = `<h2>RENOMBRAR PERFIL</h2>
        <input type="text" id="rename-input" maxlength="60" value="${esc(S.profile.name)}">
        <div class="actions"><button class="button primary" data-action="rename">Guardar</button><button class="button" data-action="close">Cancelar</button></div>`;
      const input = $("#rename-input");
      input.focus();
      input.select();
    } else {
      box.innerHTML = `<h2>ELIMINAR PERFIL</h2><p>¿Eliminar «${esc(S.profile.name)}»?</p>
        <div class="actions"><button class="button primary" data-action="delete">Eliminar</button><button class="button" data-action="close">Cancelar</button></div>`;
    }
  }
  box.dataset.kind = kind;
}

function wizardHtml(w) {
  let html = `<h2>PRUEBA GUIADA DEL ECUALIZADOR</h2>`;
  if (w.stage === "intro") {
    html += `<p class="dim">${esc(w.text)}</p><ul class="steps">${w.steps.map((s) => `<li>${esc(s)}</li>`).join("")}</ul>
      <div class="actions"><button class="button primary" data-send="wizard_start">Empezar</button><button class="button" data-send="wizard_cancel">Cancelar</button></div>`;
  } else if (w.stage === "test") {
    const off = !w.connected || w.waiting ? " disabled" : "";
    html += `<h3>${esc(w.title)}</h3><p class="dim">${esc(w.explain)}</p><div class="ab">`;
    w.buttons.forEach((b, k) => {
      html += `<button data-press="${k}" aria-pressed="${b.playing}"${off}>${esc(b.label)}</button>`;
    });
    html += `${w.waiting ? '<span class="spinner"></span>' : ""}</div>`;
    if (!w.connected) html += `<p style="color: var(--warn)">El headset no está conectado: enciéndelo para seguir.</p>`;
    if (w.readback) html += `<p class="mono faint">${esc(w.readback)}</p>`;
    if (!w.ready) html += `<p class="faint">Prueba A y B al menos una vez cada una para poder responder.</p>`;
    const dis = w.ready ? "" : " disabled";
    html += `<div class="actions">
      <button class="button" data-answer="true"${dis}>Sí, el sonido cambia</button>
      <button class="button" data-answer="false"${dis}>No, suena igual</button>
      <span class="spacer"></span><button class="button" data-send="wizard_cancel">Cancelar prueba</button></div>`;
  } else {
    html += w.lines.map((l) => `<p>${esc(l)}</p>`).join("");
    html += `<p class="verdict" data-level="${w.ok ? "ok" : "warn"}">${esc(w.verdict)}</p>`;
    html += `<p class="faint">Al cerrar se vuelve a aplicar tu perfil. El detalle quedó en debug.log.</p>
      <div class="actions"><button class="button primary" data-send="wizard_finish">Terminar</button>
      <button class="link external" data-open="logdir">Abrir carpeta de registros</button></div>`;
  }
  return html;
}

function openDialog(kind) {
  dialog = kind;
  closeMenu();
  renderDialog();
}

function closeDialog() {
  dialog = null;
  renderDialog();
}

function closeMenu() {
  $("#profile-menu").hidden = true;
}

// ------------------------------------------------------------ toasts, files

function toast(text, error = false) {
  const box = $("#toasts");
  const el = document.createElement("div");
  el.className = error ? "toast error" : "toast";
  el.textContent = text;
  box.append(el);
  while (box.children.length > 4) box.firstElementChild.remove();
  setTimeout(() => el.remove(), error ? 7000 : 4000);
}

async function importFiles(files) {
  for (const file of files) {
    if (!file.name.toLowerCase().endsWith(".synapse4")) {
      toast(`${file.name} no es un perfil de Synapse (.synapse4)`, true);
      continue;
    }
    send("import", { name: file.name, text: await file.text() });
  }
}

// ------------------------------------------------------------------- events

function onClick(e) {
  const t = e.target.closest("button, [data-open]");
  if (!$("#profile-menu").hidden && !e.target.closest(".menu-wrap")) closeMenu();
  if (!t || t.disabled) return;
  const d = t.dataset;

  if (t.matches(".toggle")) {
    if (t.getAttribute("aria-disabled") === "true") return;
    const on = t.getAttribute("aria-checked") !== "true";
    setChecked(t, on);
    if (d.toggle) send(d.send, { on });
    else if (t.id === "out-mute" || t.id === "in-mute") {
      const ep = S.audio[t.id.slice(0, -5)];
      if (ep) send("mute", { id: ep.id, muted: !on });
    } else if (t.id === "release-remote") sendAdvanced({ release_remote: on });
    else if (t.id === "legacy-config") sendAdvanced({ send_legacy_config: on });
    return;
  }
  if (d.tab) return setTab(d.tab);
  if (d.mode) return d.mode !== S.profile.eq_mode && send("eq_mode", { mode: d.mode });
  if (d.preset) return d.preset !== S.profile.preset && send("preset", { preset: d.preset });
  if (d.press) return send("wizard_press", { k: Number(d.press) });
  if (d.answer) return send("wizard_answer", { heard: d.answer === "true" });
  if (t.id === "profile-menu-button") {
    $("#profile-menu").hidden = !$("#profile-menu").hidden;
    return;
  }
  if (d.dialog) return openDialog(d.dialog);
  if (t.hasAttribute("data-import")) {
    closeMenu();
    return $("#import-file").click();
  }
  if (d.action === "rename") {
    send("rename_profile", { name: $("#rename-input").value });
    return closeDialog();
  }
  if (d.action === "delete") {
    send("delete_profile");
    return closeDialog();
  }
  if (d.action === "close") return closeDialog();
  if (d.open) return send("open", { what: d.open });
  if (d.send) {
    closeMenu();
    send(d.send);
  }
}

function setup() {
  try { tab = localStorage.getItem("rzr.tab") || tab; } catch (_) {}
  for (const el of $$(".slider")) makeSlider(el);
  setupEq();

  document.addEventListener("click", onClick);
  document.addEventListener("contextmenu", (e) => e.preventDefault());
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      if (dialog) closeDialog();
      closeMenu();
    } else if (e.key === "Enter" && e.target.id === "rename-input") {
      send("rename_profile", { name: e.target.value });
      closeDialog();
    } else if (e.key === "F5" || (e.ctrlKey && e.key === "r")) {
      location.reload();
    }
  });

  $("#profile").addEventListener("change", (e) => send("select_profile", { index: Number(e.target.value) }));
  $("#eq-method").addEventListener("change", (e) => sendAdvanced({ eq_method: e.target.value }));
  for (const sel of $$("[data-devices]")) {
    sel.addEventListener("change", () => send("default_device", { flow: sel.dataset.devices, id: sel.value }));
  }
  for (const flow of ["out", "in"]) {
    $(`#${flow}-volume input`).addEventListener("input", (e) => {
      const ep = S && S.audio[flow];
      if (ep) send("volume", { id: ep.id, value: Number(e.target.value) });
    });
  }

  const file = $("#import-file");
  file.addEventListener("change", () => {
    importFiles([...file.files]);
    file.value = "";
  });
  const hint = $("#drop-hint");
  const hasFiles = (e) => [...(e.dataTransfer?.types || [])].includes("Files");
  document.addEventListener("dragover", (e) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    hint.hidden = false;
  });
  document.addEventListener("dragleave", (e) => {
    if (!e.relatedTarget) hint.hidden = true;
  });
  document.addEventListener("drop", (e) => {
    e.preventDefault();
    hint.hidden = true;
    importFiles([...(e.dataTransfer?.files || [])]);
  });
  renderTabs();
}

/** Outside rzr (the page opened in a browser): demo data from demo.js. */
function loadDemo() {
  return new Promise((resolve) => {
    const s = document.createElement("script");
    s.src = "demo.js";
    s.onload = resolve;
    s.onerror = () => (document.body.textContent = "Falta demo.js: abre esta página con rzr.");
    document.head.append(s);
  });
}

setup();
(window.ipc ? Promise.resolve() : loadDemo()).then(() => send("ready"));
