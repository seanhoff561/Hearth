//! Game controllers (v1 M11, V2-3), read each frame through gilrs. In the world the left stick
//! walks (pushed all the way it jogs; clicked it sprints), the right stick looks, South jumps,
//! climbs and swims up, East crouches and dives, West crawls, Start pauses and Select opens the
//! globe. In the menus the D-pad and the left stick move the focus, South presses, East goes
//! back and Start closes the pause screen.

use gilrs::{Axis, Button, EventType, Gilrs};
use glam::DVec2;

/// Stick travel ignored around the centre.
const DEADZONE: f64 = 0.18;
/// How far a stick must lean to step the menu focus, and the repeat while held (s).
const NAV_LEAN: f64 = 0.6;
const NAV_REPEAT_S: f64 = 0.22;

/// A button pressed this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    South,
    East,
    West,
    North,
    Start,
    Select,
    Up,
    Down,
    Left,
    Right,
    LeftThumb,
}

/// The controller's state for movement and looking.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Pad {
    /// Left stick after the deadzone: x right, y forward (0–1 lengths).
    pub stick: DVec2,
    /// Right stick after the deadzone and a squared response: x right, y down.
    pub look: DVec2,
    pub jump: bool,
    pub crouch: bool,
    /// Toggled by West.
    pub crawl: bool,
    /// Toggled by the left stick's click; off when the stick is let go.
    pub sprint: bool,
}

pub struct Gamepads {
    gilrs: Option<Gilrs>,
    pub pad: Pad,
    raw_left: DVec2,
    raw_right: DVec2,
    /// The menu direction the left stick is leaning, and when it last stepped.
    nav: Option<(Press, f64)>,
}

impl Default for Gamepads {
    fn default() -> Self {
        Self::new()
    }
}

impl Gamepads {
    pub fn new() -> Self {
        let gilrs = match Gilrs::new() {
            Ok(g) => Some(g),
            Err(e) => {
                log::info!("no controller support: {e}");
                None
            }
        };
        Self {
            gilrs,
            pad: Pad::default(),
            raw_left: DVec2::ZERO,
            raw_right: DVec2::ZERO,
            nav: None,
        }
    }

    /// Reads what happened since the last frame; `now` is a clock in seconds. Returns the
    /// presses (stick leans count as D-pad presses, repeating while held).
    pub fn poll(&mut self, now: f64) -> Vec<Press> {
        let mut out = Vec::new();
        let Some(g) = &mut self.gilrs else {
            return out;
        };
        while let Some(ev) = g.next_event() {
            match ev.event {
                EventType::ButtonPressed(b, _) => {
                    let p = match b {
                        Button::South => Some(Press::South),
                        Button::East => Some(Press::East),
                        Button::West => Some(Press::West),
                        Button::North => Some(Press::North),
                        Button::Start => Some(Press::Start),
                        Button::Select => Some(Press::Select),
                        Button::DPadUp => Some(Press::Up),
                        Button::DPadDown => Some(Press::Down),
                        Button::DPadLeft => Some(Press::Left),
                        Button::DPadRight => Some(Press::Right),
                        Button::LeftThumb => Some(Press::LeftThumb),
                        _ => None,
                    };
                    match b {
                        Button::South => self.pad.jump = true,
                        Button::East => self.pad.crouch = true,
                        Button::West => self.pad.crawl = !self.pad.crawl,
                        Button::LeftThumb => self.pad.sprint = !self.pad.sprint,
                        _ => {}
                    }
                    out.extend(p);
                }
                EventType::ButtonReleased(b, _) => match b {
                    Button::South => self.pad.jump = false,
                    Button::East => self.pad.crouch = false,
                    _ => {}
                },
                EventType::AxisChanged(a, v, _) => {
                    let v = v as f64;
                    match a {
                        Axis::LeftStickX => self.raw_left.x = v,
                        Axis::LeftStickY => self.raw_left.y = v,
                        Axis::RightStickX => self.raw_right.x = v,
                        Axis::RightStickY => self.raw_right.y = v,
                        _ => {}
                    }
                }
                EventType::Disconnected => {
                    self.pad = Pad::default();
                    self.raw_left = DVec2::ZERO;
                    self.raw_right = DVec2::ZERO;
                }
                _ => {}
            }
        }
        self.pad.stick = deadzone(self.raw_left);
        if self.pad.stick == DVec2::ZERO {
            self.pad.sprint = false;
        }
        let look = deadzone(self.raw_right);
        // Squared response: fine aim near the centre, quick turns at the edge.
        self.pad.look = DVec2::new(look.x, -look.y) * look.length();
        // The left stick as a D-pad for menus.
        let lean = if self.raw_left.y > NAV_LEAN {
            Some(Press::Up)
        } else if self.raw_left.y < -NAV_LEAN {
            Some(Press::Down)
        } else if self.raw_left.x > NAV_LEAN {
            Some(Press::Right)
        } else if self.raw_left.x < -NAV_LEAN {
            Some(Press::Left)
        } else {
            None
        };
        match (lean, self.nav) {
            (Some(d), Some((held, t))) if d == held => {
                if now - t >= NAV_REPEAT_S {
                    out.push(d);
                    self.nav = Some((d, now));
                }
            }
            (Some(d), _) => {
                out.push(d);
                self.nav = Some((d, now));
            }
            (None, _) => self.nav = None,
        }
        out
    }
}

/// A stick past the deadzone, rescaled to start from zero.
fn deadzone(v: DVec2) -> DVec2 {
    let len = v.length();
    if len <= DEADZONE {
        DVec2::ZERO
    } else {
        v / len * ((len - DEADZONE) / (1.0 - DEADZONE)).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sticks_ignore_the_centre_and_reach_full() {
        assert_eq!(deadzone(DVec2::new(0.1, 0.1)), DVec2::ZERO);
        let full = deadzone(DVec2::new(1.0, 0.0));
        assert!((full.x - 1.0).abs() < 1e-9);
        let half = deadzone(DVec2::new(0.0, 0.59));
        assert!((half.y - 0.5).abs() < 0.01, "{half}");
    }
}
