/// Desktop control panel, laid out like Razer Synapse 4's audio pages.

mod theme;
mod widgets;

use eframe::egui::{self, vec2, Align, Align2, Color32, CornerRadius, Layout, Margin, RichText, Sense};
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use crate::config::{Config, EqMode, Profile};
use crate::protocol::{self, EqPreset};
use crate::worker::{self, AudioCmd, AudioState, Change, DevCmd, DevEvent, DeviceInfo, Target};
use crate::{registry, synapse, winaudio};
use theme::*;
use widgets::*;

const CONTENT_MAX_WIDTH: f32 = 1080.0;
const TOAST_TIME: Duration = Duration::from_secs(4);
const SAVE_DELAY: Duration = Duration::from_millis(500);

pub fn run(demo: bool) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("rzr — Razer BlackShark V2 Pro")
            .with_inner_size([1140.0, 860.0])
            .with_min_inner_size([940.0, 640.0])
            .with_icon(app_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "rzr",
        options,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(RzrApp::new(cc.egui_ctx.clone(), demo)))
        }),
    )
    .map_err(|e| e.to_string())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Sound,
    Enhancement,
    Mic,
    Power,
    Settings,
}

impl Tab {
    const ALL: [Tab; 5] = [Tab::Sound, Tab::Enhancement, Tab::Mic, Tab::Power, Tab::Settings];

    fn label(self) -> &'static str {
        match self {
            Tab::Sound => "SONIDO",
            Tab::Enhancement => "MEJORAS",
            Tab::Mic => "MICRÓFONO",
            Tab::Power => "ENERGÍA",
            Tab::Settings => "AJUSTES",
        }
    }
}

struct Toast {
    text: String,
    error: bool,
    at: Instant,
}

enum Dialog {
    Rename(String),
    ConfirmDelete,
}

struct RzrApp {
    cfg: Config,
    tab: Tab,
    demo: bool,
    dev_tx: Sender<DevCmd>,
    dev_rx: Receiver<DevEvent>,
    audio_tx: Sender<AudioCmd>,
    audio_rx: Receiver<AudioState>,
    info: DeviceInfo,
    audio: AudioState,
    toasts: Vec<Toast>,
    dirty_since: Option<Instant>,
    /// Custom EQ edited by a drag that hasn't been sent yet.
    eq_pending: bool,
    autostart: bool,
    dialog: Option<Dialog>,
}

impl RzrApp {
    fn new(ctx: egui::Context, demo: bool) -> Self {
        let cfg = Config::load();
        let (dev_tx, dev_rx) = worker::spawn_device_worker(Target::from_config(&cfg), demo, ctx.clone());
        let (audio_tx, audio_rx) = worker::spawn_audio_worker(demo, ctx);
        Self {
            cfg,
            tab: Tab::Sound,
            demo,
            dev_tx,
            dev_rx,
            audio_tx,
            audio_rx,
            info: DeviceInfo::default(),
            audio: AudioState::default(),
            toasts: Vec::new(),
            dirty_since: None,
            eq_pending: false,
            autostart: registry::autostart_enabled(),
            dialog: None,
        }
    }

    fn profile(&self) -> &Profile {
        self.cfg.profile()
    }

    fn profile_mut(&mut self) -> &mut Profile {
        self.cfg.profile_mut()
    }

    fn mark_dirty(&mut self) {
        self.dirty_since.get_or_insert_with(Instant::now);
    }

    /// Save and push a change to the headset.
    fn push(&mut self, change: Change) {
        self.mark_dirty();
        let _ = self.dev_tx.send(DevCmd::Update(Target::from_config(&self.cfg), change));
    }

    /// Save a change the headset doesn't need right now.
    fn store(&mut self) {
        self.mark_dirty();
        let _ = self.dev_tx.send(DevCmd::SetTarget(Target::from_config(&self.cfg)));
    }

    fn toast(&mut self, text: impl Into<String>, error: bool) {
        self.toasts.push(Toast { text: text.into(), error, at: Instant::now() });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    fn save_now(&mut self) {
        if let Err(e) = self.cfg.save() {
            self.toast(e, true);
        }
        self.dirty_since = None;
    }

    /// Drain worker events.
    fn pump(&mut self) {
        while let Ok(ev) = self.dev_rx.try_recv() {
            match ev {
                DevEvent::Info(info) => self.info = info,
                DevEvent::Message { text, error } => self.toast(text, error),
                DevEvent::PresetChanged(p) => {
                    self.profile_mut().select_preset(p);
                    self.mark_dirty();
                    self.toast(format!("Preset cambiado desde el headset: {}", p.label()), false);
                }
            }
        }
        while let Ok(state) = self.audio_rx.try_recv() {
            self.audio = state;
        }
    }

    fn select_profile(&mut self, index: usize) {
        if index != self.cfg.active && index < self.cfg.profiles.len() {
            self.cfg.active = index;
            self.push(Change::All);
        }
    }

    fn add_profile(&mut self, mut profile: Profile) {
        profile.name = self.cfg.unique_name(&profile.name);
        self.cfg.profiles.push(profile);
        self.cfg.active = self.cfg.profiles.len() - 1;
        self.push(Change::All);
    }

    fn import_path(&mut self, path: &Path) {
        match synapse::import_file(path) {
            Ok(profiles) => {
                let n = profiles.len();
                let first = self.cfg.profiles.len();
                for p in profiles {
                    let mut p = p;
                    p.name = self.cfg.unique_name(&p.name);
                    self.cfg.profiles.push(p);
                }
                self.cfg.active = first;
                self.push(Change::All);
                self.toast(
                    if n == 1 { "Perfil de Synapse importado".to_string() } else { format!("{n} perfiles de Synapse importados") },
                    false,
                );
            }
            Err(e) => self.toast(e, true),
        }
    }

    fn import_dialog(&mut self) {
        #[cfg(windows)]
        {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Perfil de Synapse", &["synapse4"])
                .set_title("Importar perfil de Synapse")
                .pick_file()
            {
                self.import_path(&path);
            }
        }
        #[cfg(not(windows))]
        self.toast("Arrastra el archivo .synapse4 a la ventana para importarlo", false);
    }

    // ----------------------------------------------------------------- chrome

    fn title_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("titlebar")
            .exact_size(38.0)
            .frame(egui::Frame::new().fill(TITLEBAR).inner_margin(Margin::symmetric(14, 0)))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                    paint_logo(ui.painter(), rect);
                    ui.label(RichText::new("rzr").strong().size(16.0).color(GREEN));
                    ui.label(RichText::new("AUDIO").size(13.0).color(TEXT));
                    if self.demo {
                        ui.label(RichText::new("DEMO").size(11.0).color(WARN));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let (text, color) = self.connection_text();
                        ui.label(RichText::new(text).size(12.5).color(color));
                        let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                        ui.painter().circle_filled(dot.center(), 4.0, color);
                        if self.info.busy {
                            ui.add(egui::Spinner::new().size(14.0).color(GREEN));
                            ui.label(RichText::new("Aplicando…").size(12.5).color(TEXT_DIM));
                        }
                    });
                });
            });
    }

    fn connection_text(&self) -> (&'static str, Color32) {
        if !self.info.dongle {
            ("Dongle no encontrado", ERROR)
        } else if !self.info.status.headset_connected {
            ("Headset apagado o fuera de alcance", WARN)
        } else {
            ("Conectado", GREEN)
        }
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("tabs")
            .exact_size(48.0)
            .frame(egui::Frame::new().fill(GREEN).inner_margin(Margin::symmetric(24, 0)))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 36.0;
                    for tab in Tab::ALL {
                        let selected = self.tab == tab;
                        let color = if selected {
                            Color32::from_gray(8)
                        } else {
                            Color32::from_rgba_unmultiplied(0, 0, 0, 120)
                        };
                        let resp = ui
                            .add(egui::Label::new(RichText::new(tab.label()).size(19.0).color(color)).sense(Sense::click()))
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if selected {
                            let r = resp.rect;
                            ui.painter().line_segment(
                                [egui::pos2(r.left(), r.bottom() + 4.0), egui::pos2(r.right(), r.bottom() + 4.0)],
                                egui::Stroke::new(2.0, Color32::from_gray(8)),
                            );
                        }
                        if resp.clicked() {
                            self.tab = tab;
                        }
                    }
                });
            });
    }

    fn footer(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("footer")
            .exact_size(28.0)
            .frame(egui::Frame::new().fill(FOOTER))
            .show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    let name = if self.info.product.is_empty() {
                        "RAZER BLACKSHARK V2 PRO".to_string()
                    } else {
                        self.info.product.to_uppercase()
                    };
                    ui.label(RichText::new(name).size(12.0).color(TEXT_DIM));
                });
            });
    }

    fn profile_bar(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        ui.allocate_ui_with_layout(vec2(width, 52.0), Layout::left_to_right(Align::Center), |ui| {
            ui.add_space(((width - 330.0) / 2.0).max(0.0));
            ui.label(RichText::new("PERFIL").size(13.0).color(TEXT));
            ui.add_space(6.0);

            let mut pick = None;
            egui::ComboBox::from_id_salt("profile")
                .width(220.0)
                .selected_text(self.profile().name.clone())
                .show_ui(ui, |ui| {
                    for (i, p) in self.cfg.profiles.iter().enumerate() {
                        if ui.selectable_label(i == self.cfg.active, &p.name).clicked() {
                            pick = Some(i);
                        }
                    }
                });
            if let Some(i) = pick {
                self.select_profile(i);
            }

            ui.menu_button(RichText::new("•••").size(13.0), |ui| {
                ui.set_min_width(210.0);
                if ui.button("Nuevo perfil").clicked() {
                    self.add_profile(Profile { name: "Perfil nuevo".into(), ..Profile::default() });
                }
                if ui.button("Duplicar perfil").clicked() {
                    let mut p = self.profile().clone();
                    p.name = format!("{} (copia)", p.name);
                    self.add_profile(p);
                }
                if ui.button("Renombrar…").clicked() {
                    self.dialog = Some(Dialog::Rename(self.profile().name.clone()));
                }
                if ui.add_enabled(self.cfg.profiles.len() > 1, egui::Button::new("Eliminar…")).clicked() {
                    self.dialog = Some(Dialog::ConfirmDelete);
                }
                ui.separator();
                if ui.button("Importar de Synapse (.synapse4)…").clicked() {
                    ui.close();
                    self.import_dialog();
                }
                if ui.button("Aplicar al headset ahora").clicked() {
                    self.push(Change::All);
                }
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                let st = &self.info.status;
                battery_icon(ui, st.battery, st.charging == Some(true));
                let text = match st.battery {
                    Some(b) if st.charging == Some(true) => format!("⚡ {b}%"),
                    Some(b) => format!("{b}%"),
                    None => "—".to_string(),
                };
                ui.label(RichText::new(text).size(12.5).color(TEXT));
            });
        });
    }

    // ------------------------------------------------------------------ tabs

    fn tab_sound(&mut self, ui: &mut egui::Ui) {
        self.hero(ui);
        ui.add_space(12.0);
        self.eq_card(ui);
        ui.add_space(12.0);
        ui.columns(2, |cols| {
            self.volume_card(&mut cols[0]);
            self.output_card(&mut cols[1]);
        });
    }

    fn hero(&mut self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 230.0), Sense::hover());
        let p = ui.painter_at(rect);
        // Dotted backdrop, as in Synapse
        let mut y = rect.top() + 8.0;
        while y < rect.bottom() {
            let mut x = rect.left() + 8.0;
            while x < rect.right() {
                p.circle_filled(egui::pos2(x, y), 0.9, Color32::from_gray(52));
                x += 16.0;
            }
            y += 16.0;
        }
        let connected = self.info.status.headset_connected;
        paint_headset(&p, egui::pos2(rect.center().x, rect.top() + 110.0), 1.0, connected);
        let (text, color) = self.connection_text();
        p.text(
            egui::pos2(rect.center().x, rect.bottom() - 10.0),
            Align2::CENTER_BOTTOM,
            text.to_uppercase(),
            egui::FontId::proportional(12.0),
            color,
        );
    }

    fn eq_card(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            card_title(
                ui,
                "ECUALIZADOR DE AUDIO",
                None,
                Some("Los preajustes están guardados en el headset. PERSONALIZADO usa tu propia curva de 10 bandas (−5 a +5 dB)."),
            );
            dim_text(ui, "Selecciona un preajuste de audio o personaliza uno a tu gusto.");
            ui.add_space(8.0);

            let mut mode = match self.profile().eq_mode {
                EqMode::Standard => 0,
                EqMode::Esports => 1,
            };
            if segmented(ui, &["ESTÁNDAR", "ESPORTS"], &mut mode) {
                self.profile_mut().eq_mode = if mode == 0 { EqMode::Standard } else { EqMode::Esports };
                self.push(Change::Eq);
            }
            ui.add_space(8.0);

            let active = self.profile().active_preset();
            let presets: &[EqPreset] = match self.profile().eq_mode {
                EqMode::Standard => &EqPreset::STANDARD,
                EqMode::Esports => &EqPreset::ESPORTS,
            };
            ui.horizontal(|ui| {
                let gap = ui.spacing().item_spacing.x;
                let reset_w = 150.0;
                let w = ((ui.available_width() - reset_w - gap * presets.len() as f32) / presets.len() as f32).max(80.0);
                for &preset in presets {
                    if preset_button(ui, preset.label(), preset == active, w).clicked() && preset != active {
                        self.profile_mut().select_preset(preset);
                        self.push(Change::Eq);
                    }
                }
                ui.add_space(6.0);
                if active == EqPreset::Custom {
                    if link(ui, "⟲ Restablecer").on_hover_text("Deja la curva personalizada plana").clicked() {
                        self.profile_mut().custom_eq = [0; protocol::EQ_BANDS];
                        self.push(Change::Eq);
                    }
                } else if let Some(curve) = active.curve() {
                    if link(ui, "Copiar a personalizado")
                        .on_hover_text("Reemplaza tu curva PERSONALIZADA por la de este preajuste para editarla")
                        .clicked()
                    {
                        let p = self.profile_mut();
                        p.custom_eq = curve;
                        p.select_preset(EqPreset::Custom);
                        self.push(Change::Eq);
                    }
                }
            });
            ui.add_space(10.0);

            // Stored presets live in the headset and can't be edited.
            let mut bands = self.profile().active_curve();
            let r = eq_graph(ui, &mut bands, active == EqPreset::Custom);
            if r.changed {
                self.profile_mut().custom_eq = bands;
                self.eq_pending = true;
                self.mark_dirty();
            }
            if r.committed && self.eq_pending {
                self.eq_pending = false;
                self.push(Change::Eq);
            }

            if active == EqPreset::Custom {
                faint_text(ui, "Arrastra los puntos para ajustar cada banda. Se envía al headset al soltar.");
            } else {
                faint_text(ui, "Curva de referencia del preajuste guardado en el headset (no editable). Usa «Copiar a personalizado» para partir de ella.");
            }
        });
    }

    fn volume_card(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| match self.audio.headset_out.clone() {
            Some(ep) => {
                let mut on = !ep.muted;
                if card_title(ui, "VOLUMEN", Some(&mut on), Some("Volumen de Windows para los auriculares. El interruptor silencia la salida.")) {
                    let _ = self.audio_tx.send(AudioCmd::SetMute(ep.id.clone(), !on));
                    if let Some(e) = &mut self.audio.headset_out {
                        e.muted = !on;
                    }
                }
                let mut v = (ep.volume * 100.0).round() as i32;
                let r = slider(ui, &mut v, 0..=100, 1, ("0", None, "100"), on, |v| v.to_string());
                if r.changed {
                    let _ = self.audio_tx.send(AudioCmd::SetVolume(ep.id.clone(), v as f32 / 100.0));
                    if let Some(e) = &mut self.audio.headset_out {
                        e.volume = v as f32 / 100.0;
                    }
                }
                faint_text(ui, ep.name);
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if external_link(ui, "Mezclador de volumen").clicked() {
                        winaudio::open_volume_mixer();
                    }
                    ui.add_space(16.0);
                    if external_link(ui, "Propiedades de sonido").clicked() {
                        winaudio::open_sound_settings();
                    }
                });
            }
            None => {
                card_title(ui, "VOLUMEN", None, None);
                dim_text(ui, "No se encontró la salida de audio del headset en Windows.");
            }
        });
    }

    /// Combo to pick the Windows default device set on connect. Returns the
    /// new endpoint id ("" = don't change) when the choice changes.
    fn default_device_combo(ui: &mut egui::Ui, id_salt: &str, current: &str, devices: &[winaudio::AudioDevice]) -> Option<String> {
        let selected = if current.is_empty() {
            "No cambiar".to_string()
        } else {
            devices
                .iter()
                .find(|d| d.id == current)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "(dispositivo no conectado)".to_string())
        };
        let mut pick = None;
        egui::ComboBox::from_id_salt(id_salt)
            .width(ui.available_width())
            .selected_text(selected)
            .show_ui(ui, |ui| {
                if ui.selectable_label(current.is_empty(), "No cambiar").clicked() {
                    pick = Some(String::new());
                }
                for d in devices {
                    let label = if d.is_default { format!("{}  (actual)", d.name) } else { d.name.clone() };
                    if ui.selectable_label(d.id == current, label).clicked() {
                        pick = Some(d.id.clone());
                    }
                }
            });
        pick.filter(|p| p != current)
    }

    fn output_card(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            card_title(ui, "SALIDA PREDETERMINADA", None, Some("Windows a veces cambia de dispositivo al reconectar. rzr lo corrige al conectar el headset."));
            dim_text(ui, "Dispositivo de salida que se establecerá como predeterminado al conectar el headset.");
            ui.add_space(6.0);
            let devices = self.audio.outputs.clone();
            let current = self.cfg.default_speaker.clone();
            if let Some(id) = Self::default_device_combo(ui, "default_out", &current, &devices) {
                self.cfg.default_speaker = id.clone();
                self.store();
                if !id.is_empty() {
                    let _ = self.audio_tx.send(AudioCmd::SetDefault(id));
                }
            }
        });
    }

    fn tab_enhancement(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |cols| {
            let ui = &mut cols[0];
            card(ui, |ui| {
                let mut dnd = self.profile().dnd;
                if card_title(ui, "NO MOLESTAR", Some(&mut dnd), Some("Se guarda en el headset.")) {
                    self.profile_mut().dnd = dnd;
                    self.push(Change::Dnd);
                }
                dim_text(ui, "Bloquea las llamadas entrantes de tu móvil (vía Bluetooth) mientras juegas en 2.4 GHz.");
                ui.add_space(4.0);
                faint_text(ui, "Nota: solo aplica cuando el headset está conectado al dongle inalámbrico y emparejado con tu móvil a la vez.");
            });

            let ui = &mut cols[1];
            software_features_card(
                ui,
                "MEJORAS DE SONIDO (SOFTWARE)",
                &["BASS BOOST", "NORMALIZACIÓN DE SONIDO", "CLARIDAD DE VOZ", "THX SPATIAL AUDIO"],
                "Synapse las aplica con el motor de audio de THX que instala en tu PC; el headset no las implementa (Synapse no le envía ningún comando para ellas).",
                true,
            );
        });
    }

    fn tab_mic(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |cols| {
            self.mic_card(&mut cols[0]);
            cols[0].add_space(12.0);
            card(&mut cols[0], |ui| {
                card_title(ui, "ENTRADA PREDETERMINADA", None, None);
                dim_text(ui, "Micrófono que se establecerá como predeterminado al conectar el headset.");
                ui.add_space(6.0);
                let devices = self.audio.inputs.clone();
                let current = self.cfg.default_microphone.clone();
                if let Some(id) = Self::default_device_combo(ui, "default_in", &current, &devices) {
                    self.cfg.default_microphone = id.clone();
                    self.store();
                    if !id.is_empty() {
                        let _ = self.audio_tx.send(AudioCmd::SetDefault(id));
                    }
                }
            });

            self.sidetone_card(&mut cols[1]);
            cols[1].add_space(12.0);
            software_features_card(
                &mut cols[1],
                "MEJORAS DE MICRÓFONO (SOFTWARE)",
                &["ECUALIZADOR DE MICRÓFONO", "NORMALIZACIÓN DE VOLUMEN", "CLARIDAD VOCAL", "REDUCCIÓN DE RUIDO AMBIENTAL", "SENSIBILIDAD (PUERTA DE VOZ)"],
                "Synapse procesa estas funciones en el PC; no envía ningún comando al headset para ellas, por eso no están disponibles sin Synapse.",
                false,
            );
        });
    }

    fn mic_card(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            match self.audio.headset_in.clone() {
                Some(ep) => {
                    let mut on = !ep.muted;
                    if card_title(ui, "MICRÓFONO", Some(&mut on), Some("Silencia o activa el micrófono en Windows.")) {
                        let _ = self.audio_tx.send(AudioCmd::SetMute(ep.id.clone(), !on));
                        if let Some(e) = &mut self.audio.headset_in {
                            e.muted = !on;
                        }
                    }
                    ui.label(RichText::new("VOLUMEN DEL MICRÓFONO").size(13.0));
                    let mut v = (ep.volume * 100.0).round() as i32;
                    let r = slider(ui, &mut v, 0..=100, 1, ("0", None, "100"), on, |v| v.to_string());
                    if r.changed {
                        let _ = self.audio_tx.send(AudioCmd::SetVolume(ep.id.clone(), v as f32 / 100.0));
                        if let Some(e) = &mut self.audio.headset_in {
                            e.volume = v as f32 / 100.0;
                        }
                    }
                    faint_text(ui, ep.name);
                }
                None => {
                    card_title(ui, "MICRÓFONO", None, None);
                    dim_text(ui, "No se encontró el micrófono del headset en Windows.");
                }
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let (text, color) = match self.info.status.mic_muted {
                    Some(true) => ("SILENCIADO con el botón del headset", ERROR),
                    Some(false) => ("Activo (botón del headset)", GREEN),
                    None => ("Estado del botón de silencio desconocido", TEXT_FAINT),
                };
                let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                ui.painter().circle_filled(dot.center(), 4.0, color);
                ui.label(RichText::new(text).size(12.5).color(color));
            });
        });
    }

    fn sidetone_card(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            let mut on = self.profile().sidetone_enabled;
            if card_title(ui, "MONITOREO DE MICRÓFONO (SIDETONE)", Some(&mut on), Some("Se guarda en el headset.")) {
                self.profile_mut().sidetone_enabled = on;
                self.push(Change::Sidetone);
            }
            dim_text(ui, "Escucha tu propia voz a través de los auriculares mientras hablas.");
            ui.add_space(4.0);
            let mut v = self.profile().sidetone_volume as i32;
            let r = slider(ui, &mut v, 1..=100, 1, ("0", None, "100"), on, |v| v.to_string());
            if r.changed {
                self.profile_mut().sidetone_volume = v as u8;
                self.mark_dirty();
            }
            if r.committed {
                if on {
                    self.push(Change::Sidetone);
                } else {
                    self.store();
                }
            }
        });
    }

    fn tab_power(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |cols| {
            let st = self.info.status.clone();
            card(&mut cols[0], |ui| {
                card_title(ui, "BATERÍA", None, None);
                ui.horizontal(|ui| {
                    let text = st.battery.map(|b| format!("{b}%")).unwrap_or_else(|| "—".into());
                    ui.label(RichText::new(text).size(40.0).color(TEXT));
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.add_space(12.0);
                        let (state, color) = match (st.headset_connected, st.charging) {
                            (false, _) => ("Headset no conectado", TEXT_FAINT),
                            (true, Some(true)) => ("⚡ Cargando", GREEN),
                            (true, _) => ("Con batería", TEXT_DIM),
                        };
                        ui.label(RichText::new(state).color(color));
                    });
                });
                if let Some(level) = st.battery {
                    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 8.0), Sense::hover());
                    let p = ui.painter();
                    p.rect_filled(rect, CornerRadius::same(4), TRACK);
                    let mut fill = rect;
                    fill.set_width(rect.width() * level as f32 / 100.0);
                    p.rect_filled(fill, CornerRadius::same(4), if level <= 15 { ERROR } else { GREEN });
                }
            });

            card(&mut cols[1], |ui| {
                let mut on = self.profile().auto_off_enabled;
                if card_title(ui, "APAGADO AUTOMÁTICO", Some(&mut on), Some("Se guarda en el headset.")) {
                    self.profile_mut().auto_off_enabled = on;
                    self.push(Change::AutoOff);
                }
                dim_text(ui, "Apaga el headset tras este tiempo de inactividad para ahorrar batería.");
                ui.add_space(4.0);
                let mut v = self.profile().auto_off_minutes as i32;
                let min = protocol::AUTO_OFF_MIN_MINUTES as i32;
                let max = protocol::AUTO_OFF_MAX_MINUTES as i32;
                let r = slider(ui, &mut v, min..=max, 5, ("15 MIN", None, "60 MIN"), on, |v| format!("{v} min"));
                if r.changed {
                    self.profile_mut().auto_off_minutes = v as u8;
                    self.mark_dirty();
                }
                if r.committed {
                    if on {
                        self.push(Change::AutoOff);
                    } else {
                        self.store();
                    }
                }
            });
        });
        ui.add_space(12.0);
        card(ui, |ui| {
            card_title(ui, "INFORMACIÓN DEL DISPOSITIVO", None, None);
            let yes_no = |b: bool| if b { "Sí" } else { "No" };
            let rows = [
                ("Dispositivo", if self.info.product.is_empty() { "—".to_string() } else { self.info.product.clone() }),
                ("Dongle conectado", yes_no(self.info.dongle).to_string()),
                ("Headset enlazado", yes_no(self.info.status.headset_connected).to_string()),
                ("Firmware del headset", self.info.firmware.clone().unwrap_or_else(|| "—".into())),
                ("Firmware del dongle", self.info.dongle_firmware.clone().unwrap_or_else(|| "—".into())),
                ("Número de serie", self.info.serial.clone().unwrap_or_else(|| "—".into())),
                ("Preajuste activo en el headset", self.info.status.preset.map(|p| p.label().to_string()).unwrap_or_else(|| "—".into())),
            ];
            egui::Grid::new("devinfo").num_columns(2).spacing(vec2(40.0, 8.0)).show(ui, |ui| {
                for (k, v) in rows {
                    ui.label(RichText::new(k).color(TEXT_DIM));
                    ui.label(v);
                    ui.end_row();
                }
            });
        });
    }

    fn tab_settings(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |cols| {
            card(&mut cols[0], |ui| {
                let mut on = self.autostart;
                if card_title(ui, "INICIAR CON WINDOWS", Some(&mut on), None) {
                    match registry::set_autostart(on) {
                        Ok(()) => self.autostart = on,
                        Err(e) => self.toast(e, true),
                    }
                }
                dim_text(ui, "rzr se ejecutará en segundo plano, sin ventana, y aplicará tu perfil cada vez que el headset se conecte o reconecte.");
                ui.add_space(4.0);
                faint_text(ui, "Equivale a ejecutar «rzr --silent --watch» al iniciar sesión.");
            });
            cols[0].add_space(12.0);
            card(&mut cols[0], |ui| {
                card_title(ui, "AVANZADO", None, None);
                let mut legacy = self.cfg.send_legacy_config;
                ui.horizontal(|ui| {
                    if toggle(ui, &mut legacy, true).changed() {
                        self.cfg.send_legacy_config = legacy;
                        self.store();
                    }
                    ui.label("Enviar la secuencia de inicio de Synapse al aplicar el perfil completo");
                });
                faint_text(ui, "Consulta la versión del dongle y desactiva «Speaker Preset EQ Status» (0x9E), igual que Synapse al iniciar. Desactívalo solo si notas algún problema.");
            });

            card(&mut cols[1], |ui| {
                card_title(ui, "PERFILES", None, None);
                dim_text(ui, "Tus perfiles se guardan en:");
                ui.label(RichText::new(Config::path().display().to_string()).monospace().size(11.5).color(TEXT_FAINT));
                ui.add_space(6.0);
                if ui.button("Importar perfil de Synapse (.synapse4)…").clicked() {
                    self.import_dialog();
                }
                faint_text(ui, "También puedes arrastrar un archivo .synapse4 a esta ventana.");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("Aplicar perfil al headset ahora").clicked() {
                        self.push(Change::All);
                    }
                    if ui.button("Abrir carpeta").clicked() {
                        if let Some(dir) = Config::path().parent() {
                            let _ = std::fs::create_dir_all(dir);
                            winaudio::open_folder(dir);
                        }
                    }
                });
            });
            cols[1].add_space(12.0);
            card(&mut cols[1], |ui| {
                card_title(ui, "ACERCA DE", None, None);
                dim_text(ui, format!("rzr {} — control del Razer BlackShark V2 Pro sin Razer Synapse.", env!("CARGO_PKG_VERSION")));
                faint_text(ui, "Protocolo USB por ingeniería inversa de Synapse 4, verificado con el driver de OpenRazer para este headset.");
            });
        });
    }

    // --------------------------------------------------------------- overlays

    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.dialog.take() else { return };
        let mut keep = true;
        match dialog {
            Dialog::Rename(mut name) => {
                egui::Modal::new(egui::Id::new("rename")).show(ctx, |ui| {
                    ui.set_width(320.0);
                    ui.label(RichText::new("RENOMBRAR PERFIL").color(GREEN));
                    let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY));
                    r.request_focus();
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.horizontal(|ui| {
                        if ui.button("Guardar").clicked() || enter {
                            let name = name.trim().to_string();
                            if !name.is_empty() {
                                self.profile_mut().name = name;
                                self.store();
                            }
                            keep = false;
                        }
                        if ui.button("Cancelar").clicked() {
                            keep = false;
                        }
                    });
                });
                if keep {
                    self.dialog = Some(Dialog::Rename(name));
                }
            }
            Dialog::ConfirmDelete => {
                egui::Modal::new(egui::Id::new("delete")).show(ctx, |ui| {
                    ui.set_width(320.0);
                    ui.label(RichText::new("ELIMINAR PERFIL").color(GREEN));
                    ui.label(format!("¿Eliminar «{}»?", self.profile().name));
                    ui.horizontal(|ui| {
                        if ui.button("Eliminar").clicked() {
                            self.cfg.profiles.remove(self.cfg.active);
                            self.cfg.active = self.cfg.active.saturating_sub(1);
                            self.push(Change::All);
                            keep = false;
                        }
                        if ui.button("Cancelar").clicked() {
                            keep = false;
                        }
                    });
                });
                if keep {
                    self.dialog = Some(Dialog::ConfirmDelete);
                }
            }
        }
    }

    fn toasts_ui(&mut self, ctx: &egui::Context) {
        self.toasts.retain(|t| t.at.elapsed() < TOAST_TIME);
        if self.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(Align2::RIGHT_BOTTOM, vec2(-18.0, -42.0))
            .show(ctx, |ui| {
                for t in &self.toasts {
                    egui::Frame::new()
                        .fill(Color32::from_gray(24))
                        .stroke(egui::Stroke::new(1.0, if t.error { ERROR } else { GREEN }))
                        .corner_radius(CornerRadius::same(3))
                        .inner_margin(Margin::symmetric(14, 9))
                        .show(ui, |ui| {
                            ui.set_max_width(380.0);
                            ui.label(RichText::new(&t.text).size(13.0));
                        });
                    ui.add_space(6.0);
                }
            });
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

impl eframe::App for RzrApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.pump();

        let dropped: Vec<_> = ui.input(|i| {
            i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect()
        });
        for path in dropped {
            self.import_path(&path);
        }

        self.title_bar(ui);
        self.tab_bar(ui);
        self.footer(ui);
        egui::CentralPanel::no_frame()
            .frame(egui::Frame::new().fill(BG))
            .show(ui, |ui| {
                self.profile_bar(ui);
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    let avail = ui.available_width();
                    let w = avail.min(CONTENT_MAX_WIDTH) - 32.0;
                    ui.horizontal(|ui| {
                        ui.add_space((avail - w) / 2.0);
                        ui.vertical(|ui| {
                            ui.set_width(w);
                            match self.tab {
                                Tab::Sound => self.tab_sound(ui),
                                Tab::Enhancement => self.tab_enhancement(ui),
                                Tab::Mic => self.tab_mic(ui),
                                Tab::Power => self.tab_power(ui),
                                Tab::Settings => self.tab_settings(ui),
                            }
                            ui.add_space(24.0);
                        });
                    });
                });
            });

        let ctx = ui.ctx().clone();
        self.dialogs(&ctx);
        self.toasts_ui(&ctx);

        if let Some(since) = self.dirty_since {
            if since.elapsed() >= SAVE_DELAY {
                self.save_now();
            } else {
                ctx.request_repaint_after(SAVE_DELAY);
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.dirty_since.is_some() {
            let _ = self.cfg.save();
        }
    }
}

/// Card for Synapse features that run on the PC rather than the headset.
/// `windows_alternative` adds a pointer to Windows' own sound enhancements.
fn software_features_card(ui: &mut egui::Ui, title: &str, features: &[&str], note: &str, windows_alternative: bool) {
    card(ui, |ui| {
        card_title_disabled(ui, title);
        for f in features {
            ui.horizontal(|ui| {
                let mut off = false;
                toggle(ui, &mut off, false);
                ui.label(RichText::new(*f).size(13.0).color(TEXT_FAINT));
            });
        }
        ui.add_space(6.0);
        dim_text(ui, note);
        if windows_alternative {
            ui.add_space(4.0);
            faint_text(
                ui,
                "Sin Synapse, Windows ofrece sus propias mejoras para estos audífonos (Bass Boost, Loudness Equalization, sonido envolvente virtual): Panel de sonido › Reproducción › BlackShark › Propiedades › Mejoras.",
            );
            if external_link(ui, "Abrir Panel de sonido").clicked() {
                winaudio::open_sound_settings();
            }
        }
    });
}

fn paint_logo(p: &egui::Painter, rect: egui::Rect) {
    let c = rect.center();
    let r = rect.width() / 2.0;
    p.circle_filled(c, r, GREEN);
    p.circle_filled(c, r * 0.62, TITLEBAR);
    p.circle_filled(c, r * 0.28, GREEN);
}

/// Window icon: the same green ring logo, rendered procedurally.
fn app_icon() -> egui::IconData {
    const N: u32 = 64;
    let mut rgba = Vec::with_capacity((N * N * 4) as usize);
    let c = N as f32 / 2.0 - 0.5;
    for y in 0..N {
        for x in 0..N {
            let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt() / (N as f32 / 2.0);
            let (r, g, b, a) = if d > 0.97 {
                (0, 0, 0, 0)
            } else if d > 0.6 || d < 0.28 {
                (0x44, 0xD6, 0x2C, 255)
            } else {
                (0x0B, 0x0B, 0x0B, 255)
            };
            rgba.extend_from_slice(&[r, g, b, a]);
        }
    }
    egui::IconData { rgba, width: N, height: N }
}
