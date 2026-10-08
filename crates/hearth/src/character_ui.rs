//! The character creator (Amendment E §6.2): the player's people on the left, the one chosen
//! turning in the middle under the light chosen, what they look like on the right — body,
//! skin, face, eyes, hair, brows and beard, name — with Randomize drawing someone from the
//! natural variation. Appearance changes nothing a person can do.

use hearth_character::{
    Appearance, BodyType, EyeColor, Eyebrows, FACE_PRESETS, Face, FacialHair, HAIR_COLORS,
    HairStyle, Loincloth,
    appearance::{MAX_HEIGHT_M, MIN_HEIGHT_M, from_hsl, skin_presets, to_hsl},
};
use hearth_render::figure::PreviewLight;
use hearth_ui::widgets::theme;
use hearth_ui::{Column, Rect, Ui};

use crate::menus::ROW;
use crate::profiles::Profiles;

/// The creator's own state: which way the person faces (radians), the light, a drag in
/// progress (the pointer's last x), and what follows it when it is the last step of making a
/// world (Amendment E §9.1: name and mode, the place, then who).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CharacterScreen {
    pub yaw: f32,
    pub light: usize,
    pub drag: Option<f32>,
    pub then: Option<Box<crate::menus::MenuAction>>,
}

/// The person on the creator, for the app to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    /// Where (interface pixels).
    pub rect: Rect,
    pub appearance: Appearance,
    pub yaw: f32,
    pub light: PreviewLight,
}

/// What came of a frame of the creator.
pub struct Outcome {
    pub preview: Preview,
    /// The profiles changed (to be saved).
    pub changed: bool,
    pub done: bool,
}

/// Draws the creator.
pub fn character_screen(
    ui: &mut Ui<'_>,
    profiles: &mut Profiles,
    st: &mut CharacterScreen,
) -> Outcome {
    let size = ui.size;
    let mut changed = false;
    let mut done = false;
    ui.title(8.0, &ui.t("menu.character.title"));
    // The people.
    let list_w = 120.0;
    let foot = 3.0 * (ROW + 4.0) + 8.0;
    let list_r = Rect::new(8.0, 24.0, list_w, (size.1 - 24.0 - foot).max(40.0));
    let unnamed = ui.t("menu.character.unnamed");
    let names: Vec<String> = (0..profiles.list.len())
        .map(|i| profiles.name(i, &unnamed))
        .collect();
    if let Some(i) = ui.list(
        list_r,
        "people",
        names.len(),
        14.0,
        Some(profiles.selected),
        |ui, r, i, _| {
            ui.label(r.x + 3.0, r.y + 3.0, &names[i], theme::TEXT);
        },
    ) && i != profiles.selected
    {
        profiles.selected = i;
        changed = true;
    }
    let mut c = Column::new(8.0, size.1 - foot + 4.0, list_w);
    let (a, b) = c.row(ROW).split_left((list_w - 4.0) / 2.0, 4.0);
    if ui.button(a, &ui.t("menu.character.new")) {
        profiles.add();
        changed = true;
    }
    if ui.button_enabled(b, &ui.t("menu.character.delete"), profiles.list.len() > 1) {
        profiles.remove();
        changed = true;
    }
    if ui.button(c.row(ROW), &ui.t("menu.character.randomize")) {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64);
        let a = profiles.current_mut();
        *a = a.randomized(seed);
        changed = true;
    }
    let done_words = if st.then.is_some() {
        ui.t("menu.character.begin")
    } else {
        ui.t("menu.done")
    };
    if ui.button(c.row(ROW), &done_words) {
        done = true;
    }
    // What they look like, in a column that scrolls.
    let cw = (size.0 * 0.4).min(210.0);
    let area = Rect::new(size.0 - cw - 8.0, 24.0, cw, (size.1 - 32.0).max(40.0));
    let a = profiles.current_mut();
    let before = a.clone();
    ui.scroll(area, "looks", 2.0, |ui, c| looks(ui, c, a));
    if *a != before {
        changed = true;
    }
    // The person between, turned by dragging across them.
    let px = list_w + 16.0;
    let pw = (size.0 - cw - 16.0 - px).max(40.0);
    let rect = Rect::new(px, 24.0, pw, (size.1 - 24.0 - 30.0).max(40.0));
    match ui.input.pointer {
        Some(p) if ui.input.down && (st.drag.is_some() || rect.contains(p)) => {
            if let Some(last) = st.drag {
                st.yaw += (p.0 - last) * 0.03;
            }
            st.drag = Some(p.0);
        }
        _ => st.drag = None,
    }
    let lights: Vec<String> = PreviewLight::ALL.iter().map(|l| ui.t(l.key())).collect();
    let lw = pw.min(180.0);
    let lr = Rect::new((px + (pw - lw) / 2.0).round(), size.1 - 26.0, lw, ROW);
    ui.cycle(lr, &ui.t("menu.character.light"), &lights, &mut st.light);
    Outcome {
        preview: Preview {
            rect,
            appearance: profiles.current().clone(),
            yaw: st.yaw,
            light: PreviewLight::ALL[st.light.min(PreviewLight::ALL.len() - 1)],
        },
        changed,
        done,
    }
}

/// A choice among `all`, by their language keys.
fn choose<T: Copy + PartialEq>(
    ui: &mut Ui<'_>,
    c: &mut Column,
    label: &str,
    all: &[T],
    key: impl Fn(T) -> &'static str,
    value: &mut T,
) {
    let names: Vec<String> = all.iter().map(|v| ui.t(key(*v))).collect();
    let mut i = all.iter().position(|v| v == value).unwrap_or(0);
    if ui.cycle(c.row(ROW), &ui.t(label), &names, &mut i) {
        *value = all[i];
    }
}

/// A slider of a share (0–1, or −1–1 with `signed`), its words given.
fn share(ui: &mut Ui<'_>, c: &mut Column, label: &str, value: &mut f32, signed: bool, text: &str) {
    let lo = if signed { -1.0 } else { 0.0 };
    ui.slider(c.row(ROW), &ui.t(label), value, lo, 1.0, text);
}

/// A heading over a group of rows.
fn heading(ui: &mut Ui<'_>, c: &mut Column, key: &str) {
    c.space(3.0);
    ui.label(c.x, c.y, &ui.t(key), theme::DIM);
    c.space(hearth_ui::font::LINE as f32);
}

fn looks(ui: &mut Ui<'_>, c: &mut Column, a: &mut Appearance) {
    ui.text_field(c.row(ROW), &ui.t("menu.character.name"), &mut a.name, 32);
    // Body.
    heading(ui, c, "menu.character.group.body");
    choose(
        ui,
        c,
        "menu.character.body",
        &[BodyType::Female, BodyType::Male],
        |b| match b {
            BodyType::Female => "character.body.female",
            BodyType::Male => "character.body.male",
        },
        &mut a.body,
    );
    let text = format!("{:.2} m", a.height_m);
    ui.slider(
        c.row(ROW),
        &ui.t("menu.character.height"),
        &mut a.height_m,
        MIN_HEIGHT_M,
        MAX_HEIGHT_M,
        &text,
    );
    let build = ["slight", "lean", "average", "sturdy", "heavy"][((a.build * 5.0) as usize).min(4)];
    let text = ui.t(&format!("character.build.{build}"));
    share(ui, c, "menu.character.build", &mut a.build, false, &text);
    // Skin.
    heading(ui, c, "menu.character.group.skin");
    let presets = skin_presets();
    let nearest = presets
        .iter()
        .enumerate()
        .min_by(|x, y| {
            (x.1 - a.skin_tone)
                .abs()
                .total_cmp(&(y.1 - a.skin_tone).abs())
        })
        .map_or(0, |(i, _)| i);
    let text = ui
        .lang
        .format("character.skin.tone", &[("n", &(nearest + 1).to_string())]);
    share(ui, c, "menu.character.skin", &mut a.skin_tone, false, &text);
    let under = if a.undertone < -0.33 {
        "cool"
    } else if a.undertone > 0.33 {
        "warm"
    } else {
        "neutral"
    };
    let text = ui.t(&format!("character.undertone.{under}"));
    share(
        ui,
        c,
        "menu.character.undertone",
        &mut a.undertone,
        true,
        &text,
    );
    let text = format!("{:.0}%", a.freckles * 100.0);
    share(
        ui,
        c,
        "menu.character.freckles",
        &mut a.freckles,
        false,
        &text,
    );
    // Face: a preset to begin from, then each feature.
    heading(ui, c, "menu.character.group.face");
    let mut names: Vec<String> = FACE_PRESETS.iter().map(|(k, _)| ui.t(k)).collect();
    names.push(ui.t("character.face.preset.own"));
    let mut i = FACE_PRESETS
        .iter()
        .position(|(_, f)| *f == a.face)
        .unwrap_or(FACE_PRESETS.len());
    if ui.cycle(c.row(ROW), &ui.t("menu.character.face"), &names, &mut i) && i < FACE_PRESETS.len()
    {
        a.face = FACE_PRESETS[i].1;
    }
    let keys = Face::KEYS;
    for (k, v) in keys.iter().zip(a.face.features_mut()) {
        let text = format!("{:+.0}", *v * 10.0);
        share(ui, c, k, v, true, &text);
    }
    choose(
        ui,
        c,
        "menu.character.eyes",
        &EyeColor::ALL,
        EyeColor::key,
        &mut a.eyes,
    );
    // Hair: style, length, natural colours, then the colour finely.
    heading(ui, c, "menu.character.group.hair");
    choose(
        ui,
        c,
        "menu.character.hair",
        &HairStyle::ALL,
        HairStyle::key,
        &mut a.hair,
    );
    let text = format!("{:.0}%", a.hair_length * 100.0);
    share(
        ui,
        c,
        "menu.character.hair_length",
        &mut a.hair_length,
        false,
        &text,
    );
    let mut colours: Vec<String> = HAIR_COLORS.iter().map(|(k, _)| ui.t(k)).collect();
    colours.push(ui.t("character.hair_color.custom"));
    let mut i = HAIR_COLORS
        .iter()
        .position(|(_, c)| *c == a.hair_color)
        .unwrap_or(HAIR_COLORS.len());
    if ui.cycle(
        c.row(ROW),
        &ui.t("menu.character.hair_color"),
        &colours,
        &mut i,
    ) && i < HAIR_COLORS.len()
    {
        a.hair_color = HAIR_COLORS[i].1;
    }
    let [mut h, mut s, mut l] = to_hsl(a.hair_color);
    let text = format!("{h:.0}°");
    let mut fine = ui.slider(
        c.row(ROW),
        &ui.t("menu.character.hue"),
        &mut h,
        0.0,
        360.0,
        &text,
    );
    let text = format!("{:.0}%", s * 100.0);
    fine |= ui.slider(
        c.row(ROW),
        &ui.t("menu.character.saturation"),
        &mut s,
        0.0,
        1.0,
        &text,
    );
    let text = format!("{:.0}%", l * 100.0);
    fine |= ui.slider(
        c.row(ROW),
        &ui.t("menu.character.lightness"),
        &mut l,
        0.02,
        0.95,
        &text,
    );
    if fine {
        a.hair_color = from_hsl([h, s, l]);
    }
    choose(
        ui,
        c,
        "menu.character.eyebrows",
        &Eyebrows::ALL,
        Eyebrows::key,
        &mut a.eyebrows,
    );
    choose(
        ui,
        c,
        "menu.character.facial_hair",
        &FacialHair::ALL,
        FacialHair::key,
        &mut a.facial_hair,
    );
    // What they first wear.
    choose(
        ui,
        c,
        "menu.character.loincloth",
        &[Loincloth::Hide, Loincloth::PlantFibre],
        |l| match l {
            Loincloth::Hide => "character.loincloth.hide",
            Loincloth::PlantFibre => "character.loincloth.fibre",
        },
        &mut a.loincloth,
    );
}
