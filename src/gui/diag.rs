/// Guided EQ test: sends contrasting EQ settings with each way of talking to
/// the headset, asks the user whether they hear the difference, and keeps
/// the first way that works. Every frame goes to debug.log meanwhile.

use eframe::egui::{self, Color32, RichText};

use super::theme::*;
use super::widgets::*;
use crate::config::EqMethod;
use crate::protocol::{EqPreset, EQ_BANDS};
use crate::worker::DiagAction;

/// Strong low end vs strong top end: impossible to miss if the EQ works.
const BASS: [i8; EQ_BANDS] = [5, 5, 5, 4, 0, -5, -5, -5, -5, -5];
const TREBLE: [i8; EQ_BANDS] = [-5, -5, -5, -5, 0, 4, 5, 5, 5, 5];

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Curve { method: EqMethod, release: bool },
    Presets,
    EqStatus,
}

struct Test {
    title: &'static str,
    explain: &'static str,
    kind: Kind,
}

const TESTS: [Test; 5] = [
    Test {
        title: "1 · Ecualizador, método actual",
        explain: "Envía una curva con muchos graves (A) y otra con muchos agudos (B), como lo hace rzr ahora.",
        kind: Kind::Curve { method: EqMethod::Verified, release: true },
    },
    Test {
        title: "2 · Ecualizador, sin devolver el control al headset",
        explain: "Lo mismo, pero el headset se queda en «modo remoto» (controlado por el PC) al terminar cada comando.",
        kind: Kind::Curve { method: EqMethod::Verified, release: false },
    },
    Test {
        title: "3 · Ecualizador, método de la primera versión de rzr",
        explain: "Lo mismo, con la secuencia exacta que usaba la primera versión de rzr.",
        kind: Kind::Curve { method: EqMethod::Original, release: false },
    },
    Test {
        title: "4 · Interruptor del ecualizador (0x9E)",
        explain: "Se deja puesta la curva de graves y se cambia un ajuste interno del headset: A = 1, B = 0.",
        kind: Kind::EqStatus,
    },
    Test {
        title: "5 · Presets guardados en el headset",
        explain: "A = preset JUEGO, B = preset PELÍCULA. La diferencia es menor que en las pruebas anteriores.",
        kind: Kind::Presets,
    },
];

#[derive(Clone, Copy, PartialEq)]
enum Answer {
    Yes,
    No,
    /// Test 4: which one has more bass.
    MoreBassA,
    MoreBassB,
    Same,
}

/// What the test concluded; applied to the config at the end.
pub struct Outcome {
    /// Method and remote-release that made the EQ audible, if any did.
    pub method: Option<(EqMethod, bool)>,
    /// 0x9E value that keeps the EQ on, if it made a difference.
    pub eq_status: Option<u8>,
    pub summary: Vec<String>,
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

    /// First method the user could hear, else the default one.
    fn working_method(&self) -> Option<(EqMethod, bool)> {
        TESTS.iter().zip(&self.answers).find_map(|(t, a)| match (t.kind, a) {
            (Kind::Curve { method, release }, Some(Answer::Yes)) => Some((method, release)),
            _ => None,
        })
    }

    fn method_for_rest(&self) -> (EqMethod, bool) {
        self.working_method().unwrap_or((EqMethod::Verified, true))
    }

    fn run(&mut self, out: &mut Vec<Request>, action: DiagAction) {
        let (method, release) = match TESTS[self.current()].kind {
            Kind::Curve { method, release } => (method, release),
            _ => self.method_for_rest(),
        };
        self.waiting = true;
        out.push(Request::Run { action, method, release });
    }

    fn current(&self) -> usize {
        match self.stage {
            Stage::Test(i) => i,
            _ => 0,
        }
    }

    fn enter_test(&mut self, i: usize, out: &mut Vec<Request>) {
        self.stage = Stage::Test(i);
        self.tried = [false; 2];
        self.playing = None;
        self.readback = None;
        if TESTS[i].kind == Kind::EqStatus {
            // Put the bass curve in first, with the best method found so far.
            self.run(out, DiagAction::Curve(BASS));
        }
    }

    fn next(&mut self, out: &mut Vec<Request>) {
        match self.stage {
            Stage::Intro => self.enter_test(0, out),
            Stage::Test(i) if i + 1 < TESTS.len() => self.enter_test(i + 1, out),
            _ => self.stage = Stage::Summary,
        }
    }

    fn outcome(&self) -> Outcome {
        let method = self.working_method();
        let eq_status = match self.answers[3] {
            Some(Answer::MoreBassA) => Some(1),
            Some(Answer::MoreBassB) => Some(0),
            _ => None,
        };
        let mut summary = Vec::new();
        for (t, a) in TESTS.iter().zip(&self.answers) {
            let a = match a {
                Some(Answer::Yes) => "se oye el cambio",
                Some(Answer::No) => "no se oye",
                Some(Answer::MoreBassA) => "más graves con A (1)",
                Some(Answer::MoreBassB) => "más graves con B (0)",
                Some(Answer::Same) => "suena igual",
                None => "sin responder",
            };
            summary.push(format!("{}: {a}", t.title));
        }
        Outcome { method, eq_status, summary }
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
        dim_text(ui, "Sirve para averiguar qué forma de enviar el ecualizador funciona con tus audífonos.");
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
                self.next(out);
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

        let (a_label, b_label, a, b) = match t.kind {
            Kind::Curve { .. } => ("A · GRAVES", "B · AGUDOS", DiagAction::Curve(BASS), DiagAction::Curve(TREBLE)),
            Kind::EqStatus => ("A · 0x9E = 1", "B · 0x9E = 0", DiagAction::EqStatus(1), DiagAction::EqStatus(0)),
            Kind::Presets => (
                "A · JUEGO",
                "B · PELÍCULA",
                DiagAction::Preset(EqPreset::Game),
                DiagAction::Preset(EqPreset::Movie),
            ),
        };
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
        let choices: &[(&str, Answer)] = match t.kind {
            Kind::EqStatus => &[
                ("Más graves con A", Answer::MoreBassA),
                ("Más graves con B", Answer::MoreBassB),
                ("Suenan igual", Answer::Same),
            ],
            _ => &[("Sí, el sonido cambia", Answer::Yes), ("No, suena igual", Answer::No)],
        };
        ui.horizontal(|ui| {
            for (label, answer) in choices {
                if ui.add_enabled(ready, egui::Button::new(*label)).clicked() {
                    self.answers[i] = Some(*answer);
                    self.next(out);
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
        let (text, color): (String, Color32) = match outcome.method {
            Some((EqMethod::Verified, true)) => ("El método actual funciona. No hace falta cambiar nada.".into(), GREEN),
            Some((EqMethod::Verified, false)) => (
                "Funciona si el headset se queda en modo remoto. rzr lo hará así a partir de ahora.".into(),
                GREEN,
            ),
            Some((EqMethod::Original, _)) => (
                "Funciona el método de la primera versión de rzr. rzr lo usará a partir de ahora.".into(),
                GREEN,
            ),
            None => (
                "Ningún método cambió el sonido. Envíame el archivo debug.log (botón de abajo) para seguir investigando."
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
