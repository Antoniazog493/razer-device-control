/// Guided EQ test: sends contrasting EQ settings with each way of talking to
/// the headset, asks the user whether they hear the difference, and keeps
/// the first way that works. Every frame goes to debug.log meanwhile.
///
/// Round 1 (on the user's headset) found that a curve written into Custom
/// is stored and read back exactly, yet not heard, while switching between
/// the built-in presets is heard. Round 2 tests why.
///
/// This is the test's logic only; the page draws it from `view()`.

use serde_json::{json, Value};

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

const INTRO: &str = "Ronda 2: la curva PERSONALIZADA se guarda en el headset pero no se oye, y los presets de fábrica sí. Estas pruebas buscan por qué.";
const STEPS: [&str; 5] = [
    "1. Pon música que conozcas bien (con graves y voces) a un volumen cómodo.",
    "2. En cada prueba pulsa A y B varias veces y escucha si el sonido cambia.",
    "3. Responde lo que oyes. Tarda unos 3 minutos.",
    "Al terminar, rzr usará el método que funcionó y volverá a aplicar tu perfil.",
    "Mientras tanto se guarda todo en debug.log, por si hay que enviarlo.",
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
    fn actions(self) -> [(&'static str, DiagAction); 2] {
        match self {
            Kind::Relatch => [("A · GRAVES", DiagAction::CurveRelatch(BASS)), ("B · AGUDOS", DiagAction::CurveRelatch(TREBLE))],
            Kind::EsportsSelect => [
                ("A · APEX LEGENDS", DiagAction::Select(EqPreset::ApexLegends)),
                ("B · CS2", DiagAction::Select(EqPreset::Csgo)),
            ],
            Kind::EsportsCurve => [
                ("A · GRAVES", DiagAction::SlotCurve(EqPreset::ApexLegends, BASS)),
                ("B · AGUDOS", DiagAction::SlotCurve(EqPreset::ApexLegends, TREBLE)),
            ],
            Kind::Loudness => [("A · PLANO", DiagAction::CurveRelatch(FLAT)), ("B · −9 dB", DiagAction::CurveRelatch(QUIET))],
        }
    }
}

/// A test step for the device worker.
pub struct Run {
    pub action: DiagAction,
    pub method: EqMethod,
    pub release: bool,
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

    fn answer_of(&self, kind: Kind) -> Option<Answer> {
        TESTS.iter().zip(&self.answers).find(|(t, _)| t.kind == kind).and_then(|(_, a)| *a)
    }

    /// Switching away and back is the only change that can become the
    /// default method; the esports findings need code changes first.
    fn working_method(&self) -> Option<(EqMethod, bool)> {
        let heard = |k| self.answer_of(k) == Some(Answer::Yes);
        (heard(Kind::Relatch) || heard(Kind::Loudness)).then_some((EqMethod::Relatch, true))
    }

    fn enter_test(&mut self, i: usize) {
        self.stage = Stage::Test(i);
        self.tried = [false; 2];
        self.playing = None;
        self.readback = None;
    }

    /// Leave the intro.
    pub fn start(&mut self) {
        if matches!(self.stage, Stage::Intro) {
            self.enter_test(0);
        }
    }

    /// The user pressed A (0) or B (1). Returns the step to send, unless a
    /// previous one is still running or the headset can't take it.
    pub fn press(&mut self, k: usize, can_send: bool) -> Option<Run> {
        let Stage::Test(i) = self.stage else { return None };
        if k > 1 || !can_send || self.waiting {
            return None;
        }
        self.tried[k] = true;
        self.playing = Some(k);
        self.waiting = true;
        let action = TESTS[i].kind.actions()[k].1;
        Some(Run { action, method: EqMethod::Verified, release: true })
    }

    /// Both A and B were tried and the last one finished.
    fn ready(&self) -> bool {
        self.tried[0] && self.tried[1] && !self.waiting
    }

    /// The user's answer to the current test.
    pub fn answer(&mut self, heard: bool) {
        let Stage::Test(i) = self.stage else { return };
        if !self.ready() {
            return;
        }
        self.answers[i] = Some(if heard { Answer::Yes } else { Answer::No });
        if i + 1 < TESTS.len() {
            self.enter_test(i + 1);
        } else {
            self.stage = Stage::Summary;
        }
    }

    pub fn outcome(&self) -> Outcome {
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

    /// What the page shows. `connected` = headset linked, `busy` = the
    /// device worker is sending something.
    pub fn view(&self, connected: bool, busy: bool) -> Value {
        match self.stage {
            Stage::Intro => json!({ "stage": "intro", "text": INTRO, "steps": STEPS }),
            Stage::Test(i) => {
                let t = &TESTS[i];
                let [a, b] = t.kind.actions();
                let button = |k: usize, label: &str| json!({ "label": label, "playing": self.playing == Some(k), "tried": self.tried[k] });
                json!({
                    "stage": "test",
                    "title": t.title,
                    "explain": t.explain,
                    "buttons": [button(0, a.0), button(1, b.0)],
                    "waiting": self.waiting || busy,
                    "connected": connected,
                    "readback": self.readback,
                    "ready": self.ready(),
                })
            }
            Stage::Summary => {
                let outcome = self.outcome();
                let esports = self.answer_of(Kind::EsportsCurve) == Some(Answer::Yes);
                let (text, ok) = match (outcome.method, esports) {
                    (Some(_), _) => ("Funciona escribir la curva y cambiar de preset. rzr lo hará así a partir de ahora.", true),
                    (None, true) => (
                        "La curva solo se oye en un preset Esports. Envíame debug.log: con eso puedo hacer que PERSONALIZADO use ese camino.",
                        false,
                    ),
                    (None, false) => (
                        "Ninguna prueba cambió el sonido. Envíame el archivo debug.log para seguir investigando.",
                        false,
                    ),
                };
                json!({
                    "stage": "summary",
                    "lines": outcome.summary,
                    "verdict": text,
                    "ok": ok,
                    "eq_status": outcome.eq_status,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_need_both_sides_and_pick_relatch() {
        let mut w = Wizard::new();
        assert!(w.press(0, true).is_none(), "no step before starting");
        w.start();
        assert!(w.press(0, false).is_none(), "nothing sent while disconnected");
        assert!(w.press(0, true).is_some());
        assert!(w.press(1, true).is_none(), "one step at a time");
        w.on_result("ok".into());
        w.answer(true);
        assert_eq!(w.view(true, false)["title"], TESTS[0].title, "can't answer before trying B");
        assert!(w.press(1, true).is_some());
        w.on_result("ok".into());
        w.answer(true);
        assert_eq!(w.view(true, false)["title"], TESTS[1].title);
        for _ in 1..TESTS.len() {
            for k in 0..2 {
                w.press(k, true).unwrap();
                w.on_result("ok".into());
            }
            w.answer(false);
        }
        assert_eq!(w.view(true, false)["stage"], "summary");
        assert_eq!(w.outcome().method, Some((EqMethod::Relatch, true)));
    }
}
