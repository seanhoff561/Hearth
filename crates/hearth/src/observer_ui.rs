//! Watching the world (the Observer, V2.1 §15.4; H9) on the client: the speed time is watched
//! at, whom the eye follows and their life, the chronicle and the globe's overlays, and the
//! line that tells what is being watched.

use hearth_protocol::{ChronicleEntry, OverlayKind};
use hearth_ui::{Rgba, Ui};

use crate::observer::SPEEDS;

/// What the eye follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Followed {
    Person(u64),
    Animal(u64),
}

/// The Observer's state.
#[derive(Debug, Clone, PartialEq)]
pub struct Watching {
    /// The player is alive, put aside while it watches (stepping in brings it back).
    pub alive: bool,
    /// The speed (an index of [`SPEEDS`]).
    pub speed: usize,
    pub follow: Option<Followed>,
    /// The followed person's life, as the server tells it.
    pub life: Option<(u64, Vec<String>)>,
    /// The chronicle, as last told.
    pub chronicle: Vec<ChronicleEntry>,
    /// The overlay laid on the globe, and the technique the knowledge overlay shows.
    pub overlay: Option<OverlayKind>,
    pub technique: usize,
    /// Seconds since the eye was last told to the server.
    pub told_s: f64,
}

impl Watching {
    pub fn new(alive: bool) -> Self {
        Self {
            alive,
            speed: 1,
            follow: None,
            life: None,
            chronicle: Vec::new(),
            overlay: None,
            technique: 0,
            told_s: f64::INFINITY,
        }
    }

    /// Game seconds a real second at the speed watched.
    pub fn game_s_per_s(&self) -> f64 {
        SPEEDS[self.speed.min(SPEEDS.len() - 1)].0
    }

    /// Time as the world lives it, unhurried.
    pub fn as_lived(&self) -> bool {
        self.speed == crate::observer::AS_LIVED
    }

    /// A step faster or slower; the new speed's index.
    pub fn faster(&mut self, by: i32) -> usize {
        self.speed = (self.speed as i32 + by).clamp(0, SPEEDS.len() as i32 - 1) as usize;
        self.speed
    }

    /// The line at the top of the screen: watching, how fast, whom.
    pub fn headline(&self, followed: Option<String>) -> String {
        let speed = SPEEDS[self.speed.min(SPEEDS.len() - 1)].1;
        let mut line = format!("Watching the world — time {speed}");
        if let Some(f) = followed {
            line.push_str(&format!(" — following {f}"));
        }
        line
    }

    /// Draws the headline, the keys, the followed life and the overlay's key.
    pub fn draw(&self, ui: &mut Ui<'_>, followed: Option<String>, legend: Option<&str>) {
        let (w, _h) = ui.size;
        let line = self.headline(followed);
        let lw = ui.font.width(&line) as f32;
        ui.draw.rect(
            ((w - lw) / 2.0 - 6.0).round(),
            6.0,
            lw + 12.0,
            14.0,
            Rgba([10, 12, 16, 190]),
        );
        ui.label(
            ((w - lw) / 2.0).round(),
            9.0,
            &line,
            Rgba([235, 232, 220, 245]),
        );
        let keys = if self.alive {
            "[ ] time   E follow   J chronicle   M globe (1-4 overlays)   Esc step back in"
        } else {
            "[ ] time   E follow   J chronicle   M globe (1-4 overlays)   Esc the choices"
        };
        let kw = ui.font.width(keys) as f32;
        ui.label(
            ((w - kw) / 2.0).round(),
            24.0,
            keys,
            Rgba([200, 200, 190, 200]),
        );
        if let Some(l) = legend {
            let lw = ui.font.width(l) as f32;
            ui.label(
                ((w - lw) / 2.0).round(),
                36.0,
                l,
                Rgba([255, 200, 120, 230]),
            );
        }
        if let Some((_, lines)) = &self.life {
            let x = 8.0;
            let mut y = 54.0;
            for l in lines {
                ui.label(x, y, l, Rgba([230, 228, 215, 230]));
                y += 10.0;
            }
        }
    }
}
