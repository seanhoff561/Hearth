//! Knapping by hand (v2 §11.3): the stone seen from above, the shape wanted drawn over it.
//! Press on the stone near its edge and drag the way the blow should drive the flake (further
//! for a harder blow); let go to strike. Flint takes the line it is given; coarse stone stops
//! short or runs on. Finish when the piece is shaped, or let the hands do it as they know how.

use hearth_craft::knap::Knap;
use hearth_protocol::AimAt;
use hearth_ui::widgets::{Rect, theme};
use hearth_ui::{Rgba, Ui};

/// A stone being knapped by hand, and the work it is for.
#[derive(Debug, Clone)]
pub struct KnapScreen {
    pub knap: Knap,
    /// The process (content id) and what it is done to.
    pub process: String,
    pub aim: AimAt,
    /// What is being made, in words.
    pub title: String,
    /// The stone's colour (sRGB).
    pub color: [u8; 3],
    /// Where a blow began (cells), while it is being aimed.
    pub press: Option<(f32, f32)>,
}

/// How the knapping ended.
#[derive(Debug, Clone, PartialEq)]
pub enum KnapDone {
    /// Finished by hand, at this quality (0: it snapped).
    Finished(f32),
    /// Left to the hands' habit: the work is done as usual.
    Habit,
    /// Put aside: nothing done.
    Leave,
}

/// Cells of drag for a full-force blow.
const FULL_DRAG: f32 = 8.0;

/// The knapping screen; some when it is over.
pub fn knapping_screen(ui: &mut Ui<'_>, s: &mut KnapScreen) -> Option<KnapDone> {
    let (w, h) = ui.size;
    ui.title(6.0, &s.title);
    let k = &s.knap;
    let cell = ((w - 40.0) / k.w as f32)
        .min((h - 110.0) / k.h as f32)
        .floor()
        .max(4.0);
    let (gw, gh) = (cell * k.w as f32, cell * k.h as f32);
    let (gx, gy) = (((w - gw) / 2.0).round(), 24.0);
    ui.draw
        .rect(gx - 4.0, gy - 4.0, gw + 8.0, gh + 8.0, theme::PANEL);
    let stone = Rgba([s.color[0], s.color[1], s.color[2], 255]);
    let shade = |c: Rgba, f: f32| {
        Rgba([
            (c.0[0] as f32 * f) as u8,
            (c.0[1] as f32 * f) as u8,
            (c.0[2] as f32 * f) as u8,
            c.0[3],
        ])
    };
    for y in 0..k.h {
        for x in 0..k.w {
            let i = y * k.w + x;
            let (px, py) = (gx + x as f32 * cell, gy + y as f32 * cell);
            if k.stone[i] {
                let f = 0.85 + 0.15 * (((x * 7 + y * 13) % 5) as f32 / 4.0);
                let c = if k.target[i] {
                    stone
                } else {
                    shade(stone, 0.7)
                };
                ui.draw.rect(px, py, cell, cell, shade(c, f));
            }
            // The shape wanted: its outline.
            if k.target[i] {
                let edge = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|(dx, dy)| {
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        nx < 0
                            || ny < 0
                            || nx >= k.w as i32
                            || ny >= k.h as i32
                            || !k.target[ny as usize * k.w + nx as usize]
                    });
                if edge {
                    ui.draw.rect(
                        px + cell * 0.35,
                        py + cell * 0.35,
                        cell * 0.3,
                        cell * 0.3,
                        Rgba([240, 220, 150, 200]),
                    );
                }
            }
        }
    }
    let to_cells = |p: (f32, f32)| ((p.0 - gx) / cell, (p.1 - gy) / cell);
    let pointer = ui.input.pointer.map(to_cells);
    let inside = |c: (f32, f32)| {
        c.0 >= -1.0 && c.1 >= -1.0 && c.0 <= k.w as f32 + 1.0 && c.1 <= k.h as f32 + 1.0
    };
    // Aiming and striking.
    if !k.finished() {
        if ui.input.pressed
            && let Some(p) = pointer
            && inside(p)
        {
            s.press = Some(p);
        }
        if let (Some(start), Some(now)) = (s.press, pointer) {
            if let Some((at, inward)) = s.knap.edge_near(start) {
                let drag = (now.0 - start.0, now.1 - start.1);
                let len = (drag.0 * drag.0 + drag.1 * drag.1).sqrt();
                let (dir, force) = if len < 0.6 {
                    (inward, 0.25)
                } else {
                    ((drag.0 / len, drag.1 / len), (len / FULL_DRAG).min(1.0))
                };
                // The blow's line, as long as the flake it would drive.
                let run = 2.0 + force * 7.0;
                let (x0, y0) = (gx + at.0 * cell, gy + at.1 * cell);
                for t in 0..(run * 2.0) as i32 {
                    let f = t as f32 / 2.0;
                    ui.draw.rect(
                        x0 + dir.0 * f * cell - 1.0,
                        y0 + dir.1 * f * cell - 1.0,
                        2.0,
                        2.0,
                        Rgba([250, 120, 80, 230]),
                    );
                }
                if ui.input.released {
                    s.knap.strike(at, dir, force);
                    s.press = None;
                }
            } else if ui.input.released {
                s.press = None;
            }
        }
    }
    let k = &s.knap;
    let words = if k.snapped {
        ui.t("knap.snapped")
    } else {
        ui.lang.format(
            "knap.status",
            &[
                ("strikes", &k.strikes.to_string()),
                ("max", &k.max_strikes.to_string()),
                ("shape", &format!("{:.0}", k.quality() * 100.0)),
            ],
        )
    };
    let ww = ui.font.width(&words) as f32;
    ui.label(((w - ww) / 2.0).round(), gy + gh + 8.0, &words, theme::TEXT);
    let hint = ui.t("knap.hint");
    let hw = ui.font.width(&hint) as f32;
    ui.label(((w - hw) / 2.0).round(), gy + gh + 20.0, &hint, theme::DIM);
    let bw = 120.0;
    let by = h - 30.0;
    let left = (w / 2.0 - bw * 1.5 - 6.0).round();
    let done = Rect::new(left, by, bw, 20.0);
    let habit = Rect::new(left + bw + 6.0, by, bw, 20.0);
    let leave = Rect::new(left + 2.0 * (bw + 6.0), by, bw, 20.0);
    let finished = ui.t(if k.snapped {
        "knap.take_rest"
    } else {
        "knap.finished"
    });
    if ui.button(done, &finished) {
        return Some(KnapDone::Finished(k.quality()));
    }
    if ui.button_enabled(habit, &ui.t("knap.habit"), k.strikes == 0) {
        return Some(KnapDone::Habit);
    }
    if ui.button(leave, &ui.t("knap.leave")) {
        return Some(KnapDone::Leave);
    }
    None
}
