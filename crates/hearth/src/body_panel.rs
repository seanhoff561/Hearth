//! The Body panel (v2 §9.9, key B): the body drawn region by region with its injuries, and in
//! words beside it how it is — hungry, thirsty, warm or cold, tired, out of breath, wet,
//! bleeding, in pain, ill — and each injury with how far it has healed and what is being done
//! for it.

use hearth_body::{BodyConfig, Side};
use hearth_content::schema::body::BodyRegion;
use hearth_protocol::BodyView;
use hearth_ui::widgets::theme;
use hearth_ui::{Rgba, Ui};

/// Where each region sits in the drawing (interface pixels from the drawing's top middle;
/// x of the figure's left side, which faces the viewer on the right), and its size.
fn place(r: BodyRegion) -> (f32, f32, f32, f32) {
    match r {
        BodyRegion::Head => (-9.0, 0.0, 18.0, 22.0),
        BodyRegion::Neck => (-5.0, 22.0, 10.0, 6.0),
        BodyRegion::Chest => (-20.0, 28.0, 40.0, 32.0),
        BodyRegion::Abdomen => (-17.0, 60.0, 34.0, 24.0),
        BodyRegion::Pelvis => (-19.0, 84.0, 38.0, 16.0),
        BodyRegion::UpperArm => (21.0, 30.0, 9.0, 34.0),
        BodyRegion::LowerArm => (22.0, 64.0, 8.0, 30.0),
        BodyRegion::Hand => (22.0, 94.0, 8.0, 12.0),
        BodyRegion::UpperLeg => (2.0, 100.0, 15.0, 46.0),
        BodyRegion::LowerLeg => (3.0, 146.0, 13.0, 44.0),
        BodyRegion::Foot => (3.0, 190.0, 15.0, 8.0),
    }
}

const REGIONS: [BodyRegion; 11] = [
    BodyRegion::Head,
    BodyRegion::Neck,
    BodyRegion::Chest,
    BodyRegion::Abdomen,
    BodyRegion::Pelvis,
    BodyRegion::UpperArm,
    BodyRegion::LowerArm,
    BodyRegion::Hand,
    BodyRegion::UpperLeg,
    BodyRegion::LowerLeg,
    BodyRegion::Foot,
];

/// Whether a region comes in two (arms and legs): drawn on both sides.
fn paired(r: BodyRegion) -> bool {
    matches!(
        r,
        BodyRegion::UpperArm
            | BodyRegion::LowerArm
            | BodyRegion::Hand
            | BodyRegion::UpperLeg
            | BodyRegion::LowerLeg
            | BodyRegion::Foot
    )
}

fn region_key(r: BodyRegion) -> &'static str {
    match r {
        BodyRegion::Head => "body.region.head",
        BodyRegion::Neck => "body.region.neck",
        BodyRegion::Chest => "body.region.chest",
        BodyRegion::Abdomen => "body.region.abdomen",
        BodyRegion::Pelvis => "body.region.pelvis",
        BodyRegion::UpperArm => "body.region.upper_arm",
        BodyRegion::LowerArm => "body.region.lower_arm",
        BodyRegion::Hand => "body.region.hand",
        BodyRegion::UpperLeg => "body.region.upper_leg",
        BodyRegion::LowerLeg => "body.region.lower_leg",
        BodyRegion::Foot => "body.region.foot",
    }
}

/// The rectangles a region (on a side) is drawn as.
fn rects(r: BodyRegion, side: Side, cx: f32, top: f32) -> Vec<(f32, f32, f32, f32)> {
    let (x, y, w, h) = place(r);
    if !paired(r) {
        return vec![(cx + x, top + y, w, h)];
    }
    // The figure faces the viewer: its left side is on the viewer's right.
    let left = (cx + x, top + y, w, h);
    let right = (cx - x - w, top + y, w, h);
    match side {
        Side::Left => vec![left],
        Side::Right => vec![right],
        Side::Middle => vec![left, right],
    }
}

/// Draws the panel over the middle of the screen.
pub fn draw(ui: &mut Ui<'_>, cfg: Option<&BodyConfig>, b: &BodyView, veil: u8) {
    let (w, h) = ui.size;
    let pw = 420.0f32.min(w - 16.0);
    let ph = 280.0f32.min(h - 16.0);
    let (x0, y0) = (((w - pw) / 2.0).round(), ((h - ph) / 2.0).round());
    ui.draw
        .rect(x0, y0, pw, ph, Rgba([10, 12, 16, veil.max(200)]));
    ui.title(y0 + 6.0, &ui.t("body.panel.title"));
    // The body, region by region.
    let cx = x0 + 70.0;
    let top = y0 + 30.0;
    for r in REGIONS {
        for (x, y, rw, rh) in rects(r, Side::Middle, cx, top) {
            ui.draw.rect(x, y, rw, rh, Rgba([86, 88, 96, 255]));
        }
    }
    for inj in &b.injuries {
        let sev = inj.severity.clamp(0.0, 1.0);
        let fresh = (1.0 - inj.healed as f32).clamp(0.0, 1.0);
        let color = if inj.infected {
            Rgba([170, 190, 60, 255])
        } else if inj.id.ends_with("frostbite") {
            Rgba([90, 150, 230, 255])
        } else if inj.id.ends_with("bruise") {
            Rgba([120, 80, 150, 255])
        } else {
            Rgba([200, 40, 40, 255])
        };
        let a = (80.0 + 175.0 * sev * fresh.max(0.25)) as u8;
        for (x, y, rw, rh) in rects(inj.region, inj.side, cx, top) {
            ui.draw.rect(x, y, rw, rh, color.with_alpha(a));
            // Dressed: white bands; splinted: a bar along it.
            if inj.treatments.iter().any(|t| t == "bandage") {
                for k in 0..3 {
                    let by = y + rh * (0.25 + 0.25 * k as f32);
                    ui.draw.rect(x, by, rw, 1.0, Rgba([235, 235, 230, 230]));
                }
            }
            if inj.treatments.iter().any(|t| t == "splint") {
                ui.draw
                    .rect(x + rw / 2.0 - 1.0, y, 2.0, rh, Rgba([150, 110, 60, 255]));
            }
        }
    }
    // How it is, in words.
    let s = &b.status;
    let mut lines: Vec<(String, Rgba)> = Vec::new();
    let state = |ui: &Ui<'_>, label: &str, value: &str| format!("{}: {}", ui.t(label), ui.t(value));
    lines.push((state(ui, "body.panel.hunger", s.hunger.key()), theme::TEXT));
    lines.push((state(ui, "body.panel.thirst", s.thirst.key()), theme::TEXT));
    let mut warmth = state(ui, "body.panel.warmth", s.warmth.key());
    if s.effects.shivering > 0.3 {
        warmth = format!("{warmth}, {}", ui.t("body.panel.shivering"));
    }
    if s.effects.sweating > 0.3 {
        warmth = format!("{warmth}, {}", ui.t("body.panel.sweating"));
    }
    lines.push((warmth, theme::TEXT));
    lines.push((
        state(ui, "body.panel.tiredness", s.tiredness.key()),
        theme::TEXT,
    ));
    let breath = if s.stamina < 0.25 {
        "body.breath.spent"
    } else if s.stamina < 0.6 {
        "body.breath.winded"
    } else {
        "body.breath.steady"
    };
    lines.push((state(ui, "body.panel.breath", breath), theme::TEXT));
    let wet = if s.wet < 0.1 {
        "body.wet.dry"
    } else if s.wet < 0.4 {
        "body.wet.damp"
    } else if s.wet < 0.75 {
        "body.wet.wet"
    } else {
        "body.wet.soaked"
    };
    lines.push((state(ui, "body.panel.wet", wet), theme::TEXT));
    if s.bleeding {
        lines.push((ui.t("body.panel.bleeding"), Rgba::rgb(230, 90, 80)));
    }
    if s.blood_lost > 0.1 {
        lines.push((ui.t("body.panel.blood_lost"), Rgba::rgb(230, 90, 80)));
    }
    if s.effects.pain > 0.15 {
        let pain = if s.effects.pain < 0.4 {
            "body.pain.mild"
        } else if s.effects.pain < 0.7 {
            "body.pain.strong"
        } else {
            "body.pain.severe"
        };
        lines.push((state(ui, "body.panel.pain", pain), theme::WARN));
    }
    for id in &b.illnesses {
        let name = cfg
            .and_then(|c| c.illness(id))
            .map_or_else(|| id.clone(), |i| i.name.clone());
        lines.push((format!("{}: {}", ui.t("body.panel.ill"), name), theme::WARN));
    }
    lines.push((String::new(), theme::TEXT));
    lines.push((ui.t("body.panel.injuries"), theme::DIM));
    if b.injuries.is_empty() {
        lines.push((ui.t("body.panel.none"), theme::DIM));
    }
    for inj in &b.injuries {
        let name = cfg
            .and_then(|c| c.injury(&inj.id))
            .map_or_else(|| inj.id.clone(), |i| i.name.clone());
        let side = match inj.side {
            Side::Left => format!(" ({})", ui.t("body.side.left")),
            Side::Right => format!(" ({})", ui.t("body.side.right")),
            Side::Middle => String::new(),
        };
        let severity = if inj.severity < 0.3 {
            "body.severity.light"
        } else if inj.severity < 0.6 {
            "body.severity.moderate"
        } else {
            "body.severity.severe"
        };
        let healing = if inj.healed < 0.25 {
            "body.healing.fresh"
        } else if inj.healed < 0.6 {
            "body.healing.healing"
        } else if inj.healed < 0.95 {
            "body.healing.mending"
        } else {
            "body.healing.nearly"
        };
        let mut line = format!(
            "{name}, {}{side}: {}, {}",
            ui.t(region_key(inj.region)),
            ui.t(severity),
            ui.t(healing)
        );
        if inj.infected {
            line = format!("{line}, {}", ui.t("body.panel.infected"));
        }
        lines.push((line, theme::TEXT));
        if !inj.treatments.is_empty() {
            let list = inj.treatments.join(", ");
            lines.push((
                format!(
                    "  {}",
                    ui.lang.format("body.panel.treated", &[("list", &list)])
                ),
                theme::DIM,
            ));
        }
    }
    let tx = x0 + 150.0;
    let lh = hearth_ui::font::LINE as f32;
    let max_w = (x0 + pw - 6.0 - tx) as u32;
    let mut y = y0 + 26.0;
    for (line, color) in &lines {
        if y > y0 + ph - 2.0 * lh {
            break;
        }
        // Long lines wrap to the panel's width.
        for part in ui.font.wrap(line, max_w) {
            ui.label(tx, y, &part, *color);
            y += lh;
        }
        if line.is_empty() {
            y += 2.0;
        }
    }
    let hint = ui.t("body.panel.hint");
    let hw = ui.font.width(&hint) as f32;
    ui.label(x0 + pw - hw - 6.0, y0 + ph - lh - 2.0, &hint, theme::DIM);
}
