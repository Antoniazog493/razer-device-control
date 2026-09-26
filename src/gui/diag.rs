/// Guided EQ test: sends contrasting EQ settings with each way of talking to
/// the headset, asks the user whether they hear the difference, and keeps
/// the first way that works. Every frame goes to debug.log meanwhile.
///
/// Round 1 (on the user's headset) found that a curve written into Custom
/// is stored and read back exactly, yet not heard, while switching between
/// the built-in presets is heard. Round 2 tests why.

use eframe::egui::{self, Color32, RichText};

use super::theme::*;
use super::widgets::*;
use crate::config::EqMethod;
use crate::protocol::{EqPreset, EQ_BANDS};
use crate::worker::DiagAction;

/// Strong low end vs strong top end: impossible to miss if the EQ works.
const BASS: [i8; EQ_BANDS] = [5, 5, 5, 4, 0, -5, -5, -5, -5, -5];
const TREBLE: [i8; EQ_BANDS] = [-5, -5, -5, -5, 0, 4, 5, 5, 5, 5];
/// Every band down 9 dB vs flat: a plain volume drop, the easiest change
/// to hear (OpenRazer's own audibility check used the same curve).
const QUIET: [i8; EQ_BANDS] = [-9; EQ_BANDS];
const FLAT: [i8; EQ_BANDS] = [0; EQ_BANDS];

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// Custom curve, then leave Custom and come back.
    Relatch,
    /// Select two esports presets whose stored curves differ a lot.
    EsportsSelect,
    /// Write the curves into an esports preset's slot, like Synapse does.
    EsportsCurve,
    /// Flat vs everything at -9 dB in Custom, with the switch-and-back.
    Loudness,
}

struct Test {
    title: &'static str,
    explain: &'static str,
    kind: Kind,
}

const TESTS: [Test; 4] = [
    Test {
        title: "1 · Personalizado, cambiando de preset y volviendo",
        explain: "Escribe la curva de graves (A) o de agudos (B) en PERSONALIZADO, pasa un momento a JUEGO y vuelve a PERSONALIZADO. Cada pulsación tarda unos 2 segundos.",
        kind: Kind::Relatch,
    },
    Test {
        title: "2 · Presets Esports guardados",
        explain: "Solo cambia de preset, sin escribir curvas: A = APEX LEGENDS (curva suave), B = CS2 (curva muy marcada).",
        kind: Kind::EsportsSelect,
    },
    Test {
        title: "3 · Curva escrita en un preset Esports",
        explain: "Escribe la curva de graves (A) o de agudos (B) en el preset APEX LEGENDS, como hace Synapse con los presets Esports.",
        kind: Kind::EsportsCurve,
    },
    Test {
        title: "4 · Volumen con el ecualizador",
        explain: "A = PERSONALIZADO plano, B = PERSONALIZADO con todas las bandas en −9 dB (debería sonar mucho más bajo). También cambia de preset y vuelve.",
        kind: Kind::Loudness,
    },
];

#[derive(Clone, Copy, PartialEq)]
enum Answer {
    Yes,
    No,
}

/// What the test concluded; applied to the config at the end.
pub struct Outcome {
    /// Method and remote-release that made the EQ audible, if any did.
    pub method: Option<(EqMethod, bool)>,
    /// 0x9E value that keeps the EQ on, if it made a difference.
    pub eq_status: Option<u8>,
    pub summary: Vec<String>,
}

impl Kind {
    fn actions(self) -> (&'static str, &'static str, DiagAction, DiagAction) {
        match self {
            Kind::Relatch => ("A · GRAVES", "B · AGUDOS", DiagAction::CurveRelatch(BASS), DiagAction::CurveRelatch(TREBLE)),
            Kind::EsportsSelect => (
                "A · APEX LEGENDS",
                "B · CS2",
                DiagAction::Select(EqPreset::ApexLegends),
                DiagAction::Select(EqPreset::Csgo),
            ),
            Kind::EsportsCurve => (
                "A · GRAVES",
                "B · AGUDOS",
                DiagAction::SlotCurve(EqPreset::ApexLegends, BASS),
                DiagAction::SlotCurve(EqPreset::ApexLegends, TREBLE),
            ),
            Kind::Loudness => ("A · PLANO", "B · −9 dB", DiagAction::CurveRelatch(FLAT), DiagAction::CurveRelatch(QUIET)),
        }
    }
}

pub enum Request {
    Run { action: DiagAction, method: EqMethod, release: bool },
    Finish(Outcome),
    Cancel,
}

enum Stage {
    Intro,
    Test(usize),
    Summary,
}

pub struct Wizard {
    stage: Stage,
    answers: Vec<Option<Answer>>,
    /// Which of A/B the user tried in this step.
    tried: [bool; 2],
    /// A/B currently playing.
    playing: Option<usize>,
    readback: Option<String>,
    waiting: bool,
}

impl Wizard {
    pub fn new() -> Self {
        Self {
            stage: Stage::Intro,
            answers: vec![None; TESTS.len()],
            tried: [false; 2],
            playing: None,
            readback: None,
            waiting: false,
        }
    }

    /// Reply from the device worker to the last Run.
    pub fn on_result(&mut self, text: String) {
        self.readback = Some(text);
        self.waiting = false;
    }

    fn answer(&self, kind: Kind) -> Option<Answer> {
        TESTS.iter().zip(&self.answers).find(|(t, _)| t.kind == kind).and_then(|(_, a)| *a)
    }

    /// Switching away and back is the only change that can become the
    /// default method; the esports findings need code changes first.
    fn working_method(&self) -> Option<(EqMethod, bool)> {
        let heard = |k| self.answer(k) == Some(Answer::Yes);
        (heard(Kind::Relatch) || heard(Kind::Loudness)).then_some((EqMethod::Relatch, true))
    }

    fn run(&mut self, out: &mut Vec<Request>, action: DiagAction) {
        self.waiting = true;
        out.push(Request::Run { action, method: EqMethod::Verified, release: true });
    }

    fn enter_test(&mut self, i: usize) {
        self.stage = Stage::Test(i);
        self.tried = [false; 2];
        self.playing = None;
        self.readback = None;
    }

    fn next(&mut self) {
        match self.stage {
            Stage::Intro => self.enter_test(0),
            Stage::Test(i) if i + 1 < TESTS.len() => self.enter_test(i + 1),
            _ => self.stage = Stage::Summary,
        }
    }

    fn outcome(&self) -> Outcome {
        let mut summary = vec!["Ronda 2 de la prueba guiada".to_string()];
        for (t, a) in TESTS.iter().zip(&self.answers) {
            let a = match a {
                Some(Answer::Yes) => "se oye el cambio",
                Some(Answer::No) => "no se oye",
                None => "sin responder",
            };
            summary.push(format!("{}: {a}", t.title));
        }
        Outcome { method: self.working_method(), eq_status: None, summary }
    }

    /// Draw the dialog. `connected` = headset linked.
    pub fn show(&mut self, ctx: &egui::Context, connected: bool, busy: bool) -> Vec<Request> {
        let mut out = Vec::new();
        egui::Modal::new(egui::Id::new("diag")).show(ctx, |ui| {
            ui.set_width(560.0);
            ui.label(RichText::new("PRUEBA GUIADA DEL ECUALIZADOR").size(16.0).color(GREEN));
            ui.add_space(6.0);
            match self.stage {
                Stage::Intro => self.intro(ui, &mut out),
                Stage::Test(i) => self.test(ui, i, connected, busy, &mut out),
                Stage::Summary => self.summary(ui, &mut out),
            }
        });
        out
    }

    fn intro(&mut self, ui: &mut egui::Ui, out: &mut Vec<Request>) {
        dim_text(ui, "Ronda 2: la curva PERSONALIZADA se guarda en el headset pero no se oye, y los presets de fábrica sí. Estas pruebas buscan por qué.");
        ui.add_space(4.0);
        for line in [
            "1. Pon música que conozcas bien (con graves y voces) a un volumen cómodo.",
            "2. En cada prueba pulsa A y B varias veces y escucha si el sonido cambia.",
            "3. Responde lo que oyes. Tarda unos 3 minutos.",
            "Al terminar, rzr usará el método que funcionó y volverá a aplicar tu perfil.",
            "Mientras tanto se guarda todo en debug.log, por si hay que enviarlo.",
        ] {
            ui.label(line);
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button(RichText::new("Empezar").size(15.0)).clicked() {
                self.next();
            }
            if ui.button("Cancelar").clicked() {
                out.push(Request::Cancel);
            }
        });
    }

    fn test(&mut self, ui: &mut egui::Ui, i: usize, connected: bool, busy: bool, out: &mut Vec<Request>) {
        let t = &TESTS[i];
        ui.label(RichText::new(t.title).size(15.0).strong());
        dim_text(ui, t.explain);
        ui.add_space(10.0);

        let (a_label, b_label, a, b) = t.kind.actions();
        let can_send = connected && !self.waiting && !busy;
        ui.horizontal(|ui| {
            for (k, (label, action)) in [(a_label, a), (b_label, b)].into_iter().enumerate() {
                if preset_button(ui, label, self.playing == Some(k), 200.0).clicked() && can_send {
                    self.tried[k] = true;
                    self.playing = Some(k);
                    self.run(out, action);
                }
            }
            if self.waiting || busy {
                ui.add(egui::Spinner::new().size(16.0).color(GREEN));
            }
        });
        if !connected {
            ui.label(RichText::new("El headset no está conectado: enciéndelo para seguir.").color(WARN));
        }
        if let Some(text) = &self.readback {
            ui.add_space(4.0);
            ui.label(RichText::new(text).monospace().size(11.0).color(TEXT_FAINT));
        }
        ui.add_space(10.0);

        let ready = self.tried[0] && self.tried[1] && !self.waiting;
        if !ready {
            faint_text(ui, "Prueba A y B al menos una vez cada una para poder responder.");
        }
        let choices: &[(&str, Answer)] = &[("Sí, el sonido cambia", Answer::Yes), ("No, suena igual", Answer::No)];
        ui.horizontal(|ui| {
            for (label, answer) in choices {
                if ui.add_enabled(ready, egui::Button::new(*label)).clicked() {
                    self.answers[i] = Some(*answer);
                    self.next();
                }
            }
            ui.add_space(12.0);
            if ui.button("Cancelar prueba").clicked() {
                out.push(Request::Cancel);
            }
        });
    }

    fn summary(&mut self, ui: &mut egui::Ui, out: &mut Vec<Request>) {
        let outcome = self.outcome();
        for line in &outcome.summary {
            ui.label(RichText::new(line).size(12.5));
        }
        ui.add_space(8.0);
        let esports = self.answer(Kind::EsportsCurve) == Some(Answer::Yes);
        let (text, color): (String, Color32) = match (outcome.method, esports) {
            (Some(_), _) => (
                "Funciona escribir la curva y cambiar de preset. rzr lo hará así a partir de ahora.".into(),
                GREEN,
            ),
            (None, true) => (
                "La curva solo se oye en un preset Esports. Envíame debug.log: con eso puedo hacer que PERSONALIZADO use ese camino.".into(),
                WARN,
            ),
            (None, false) => (
                "Ninguna prueba cambió el sonido. Envíame el archivo debug.log (botón de abajo) para seguir investigando."
                    .into(),
                WARN,
            ),
        };
        ui.label(RichText::new(text).color(color));
        if let Some(v) = outcome.eq_status {
            ui.label(format!("El ajuste 0x9E importa: rzr enviará {v} para mantener el ecualizador activo."));
        }
        ui.add_space(4.0);
        faint_text(ui, "Al cerrar se vuelve a aplicar tu perfil. El detalle quedó en debug.log.");
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button(RichText::new("Terminar").size(15.0)).clicked() {
                out.push(Request::Finish(self.outcome()));
            }
            if external_link(ui, "Abrir carpeta de registros").clicked() {
                if let Some(dir) = crate::debuglog::path().parent() {
                    crate::winaudio::open_path(dir);
                }
            }
        });
    }
}
