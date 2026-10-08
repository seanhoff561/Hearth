//! Where to begin (Amendment E §6.3): the places suggested for a new world and their cards
//! beside its globe, or any place chosen on it.

use hearth_ui::widgets::theme;
use hearth_ui::{Column, Rect, Ui};

use crate::menus::{MenuAction, MenuContext, NewWorldChoice, ROW, Screen};

/// The Birthplace screen's state.
pub struct Birth<'a> {
    pub chosen: &'a mut Option<(f32, f32)>,
    pub shown: &'a mut usize,
    pub anywhere: &'a mut bool,
    pub card: &'a mut Option<Box<crate::places::Place>>,
}

/// Where to begin (Amendment E §6.3): the places suggested down the left, the one shown's card
/// on the right — what to look for and what to watch out for there — and the globe between;
/// or, choosing anywhere, the globe to click, and the card of the place clicked.
pub fn birthplace_screen(
    ui: &mut Ui<'_>,
    cx: &mut MenuContext<'_>,
    choice: &NewWorldChoice,
    b: &mut Birth<'_>,
    out: &mut Vec<MenuAction>,
    push: &mut Option<Screen>,
    pop: &mut bool,
) {
    use crate::places;
    let size = ui.size;
    ui.title(8.0, &ui.t("menu.birthplace.title"));
    let line = hearth_ui::font::LINE as f32;
    let side = (size.0 * 0.3).clamp(120.0, 200.0);
    let card_w = (size.0 * 0.34).clamp(140.0, 240.0);
    let foot_y = size.1 - ROW - 8.0;
    let top = 24.0;
    let scale = ui.draw.scale;
    let pointer = ui.input.pointer;
    let suggested: Vec<places::Place> = cx
        .globe
        .as_ref()
        .map(|g| g.places.to_vec())
        .unwrap_or_default();
    // The places suggested, and choosing anywhere.
    let mut picked: Option<usize> = None;
    let row_h = 2.0 * line + 8.0;
    let rows = suggested.len() as f32 * row_h;
    let list = Rect::new(8.0, top, side, rows.min(foot_y - top - 2.0 * (ROW + 4.0)));
    if !suggested.is_empty() {
        let cards: Vec<(String, String)> = suggested
            .iter()
            .map(|p| (ui.t(p.difficulty.key()), places::words(ui.lang, &p.name)))
            .collect();
        let selected = (!*b.anywhere).then_some(*b.shown);
        if let Some(i) = ui.list(
            list,
            "places",
            cards.len(),
            row_h,
            selected,
            |ui, r, i, _| {
                ui.label(r.x + 3.0, r.y + 4.0, &cards[i].0, theme::FOCUS);
                let name = ui.font.wrap(&cards[i].1, r.w as u32 - 6);
                ui.label(r.x + 3.0, r.y + 4.0 + line, &name[0], theme::TEXT);
            },
        ) {
            picked = Some(i);
        }
    } else {
        ui.label(8.0, top, &ui.t("menu.birthplace.none"), theme::DIM);
    }
    let mut c = Column::new(8.0, list.y + list.h + 6.0, side);
    let label = if *b.anywhere {
        ui.t("menu.birthplace.suggested")
    } else {
        ui.t("menu.birthplace.anywhere")
    };
    if !suggested.is_empty() && ui.button(c.row(ROW), &label) {
        *b.anywhere = !*b.anywhere;
        if !*b.anywhere {
            picked = Some(*b.shown);
        }
    }
    if let Some(i) = picked {
        *b.anywhere = false;
        *b.shown = i;
        if let (Some(g), Some(p)) = (cx.globe.as_mut(), suggested.get(i)) {
            let at = glam::DVec3::new(p.x as f64, 0.0, p.z as f64);
            let (lat, lon) = crate::globe::lat_lon(g.terrain.planet(), at);
            g.picker.view.lat = lat;
            g.picker.view.lon = lon;
            *b.chosen = Some((lat, lon));
        }
    }
    // The globe between: clicked anywhere, the place under the click.
    let card_x = size.0 - card_w - 8.0;
    let mut said: Option<String> = None;
    let when = match choice.shape.start {
        Some(hearth_save::Start::Now) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0.0, |d| d.as_secs_f64());
            places::When::YearFrac(hearth_env::Calendar::from_unix(now).at(0).year_frac)
        }
        _ => places::When::Spring,
    };
    if let Some(g) = cx.globe.as_mut() {
        let over_globe =
            pointer.is_some_and(|p| p.1 > top && p.1 < foot_y && p.0 > 8.0 + side && p.0 < card_x);
        if let Some(p) = pointer {
            g.picker.cursor_moved(glam::Vec2::new(p.0, p.1) * scale);
        }
        if over_globe {
            if ui.input.pressed {
                g.picker.button(true);
            }
            if ui.input.released
                && let Some(ll) = g.picker.button(false)
            {
                // A click on the globe chooses there.
                *b.anywhere = true;
                *b.chosen = Some(ll);
                // What is there, read from the world (a place without fresh water near has
                // no card).
                let (x, z) = crate::globe::world_xz(g.terrain.planet(), ll.0, ll.1);
                let (x, z) = g.terrain.spawn_near(x, z);
                *b.card = g
                    .finder
                    .as_ref()
                    .and_then(|f| f.verify(x, z, when))
                    .map(Box::new);
            }
            if ui.input.scroll != 0.0 {
                g.picker.view.zoom_by(ui.input.scroll.round() as i32);
            }
        }
        if *b.anywhere {
            let shown = g.picker.hovered().or(*b.chosen);
            said = shown.map(|(lat, lon)| crate::globe::describe(&g.terrain, lat, lon));
        }
    }
    if let Some(s) = said {
        let mid = Rect::new(8.0 + side + 4.0, top, card_x - side - 16.0, line);
        for (k, l) in ui.font.wrap(&s, mid.w as u32).iter().take(3).enumerate() {
            ui.text_centred(
                &Rect::new(mid.x, mid.y + k as f32 * line, mid.w, line),
                l,
                theme::TEXT,
            );
        }
    }
    // The card of the place shown.
    let place: Option<&places::Place> = if *b.anywhere {
        b.card.as_deref()
    } else {
        suggested.get(*b.shown)
    };
    let area = Rect::new(card_x, top, card_w, foot_y - top - 6.0);
    ui.draw.rect(
        area.x - 4.0,
        area.y - 2.0,
        area.w + 8.0,
        area.h + 4.0,
        theme::PANEL,
    );
    let lang = ui.lang;
    ui.scroll(area, "place_card", 1.0, |ui, c| {
        let para = |ui: &mut Ui<'_>, c: &mut Column, text: &str, colour| {
            for l in ui.font.wrap(text, c.w as u32) {
                ui.label(c.x, c.y, &l, colour);
                c.space(line);
            }
        };
        match place {
            Some(p) => {
                para(ui, c, &places::words(lang, &p.name), theme::TEXT);
                para(ui, c, lang.get(p.difficulty.key()), theme::FOCUS);
                para(ui, c, &places::words(lang, &p.climate), theme::DIM);
                para(ui, c, &places::words(lang, &p.terrain), theme::DIM);
                c.space(4.0);
                para(ui, c, lang.get("menu.birthplace.look_for"), theme::FOCUS);
                for claim in &p.look_for {
                    para(ui, c, &places::claim_words(lang, claim), theme::TEXT);
                }
                if !p.watch_out.is_empty() {
                    c.space(4.0);
                    para(ui, c, lang.get("menu.birthplace.watch_out"), theme::WARN);
                    for d in &p.watch_out {
                        para(ui, c, &places::danger_words(lang, d), theme::TEXT);
                    }
                }
            }
            None if *b.anywhere && b.chosen.is_some() => {
                para(ui, c, lang.get("menu.birthplace.unverified"), theme::DIM);
            }
            None => {
                para(ui, c, lang.get("menu.birthplace.click"), theme::DIM);
            }
        }
    });
    // Begin here, or back.
    let w = ((size.0 - 24.0) / 2.0).min(200.0);
    let x0 = ((size.0 - 2.0 * w - 8.0) / 2.0).round();
    let here = Rect::new(x0, foot_y, w, ROW);
    let back = Rect::new(x0 + w + 8.0, foot_y, w, ROW);
    // A suggested place begins at its own spot; one chosen anywhere about the place clicked.
    let at: Option<glam::DVec2> = match place {
        Some(p) => Some(glam::DVec2::new(p.x as f64 + 0.5, p.z as f64 + 0.5)),
        None => match (*b.chosen, cx.globe.as_ref()) {
            (Some((lat, lon)), Some(g)) if *b.anywhere => {
                let (x, z) = crate::globe::world_xz(g.terrain.planet(), lat, lon);
                Some(glam::DVec2::new(x as f64 + 0.5, z as f64 + 0.5))
            }
            _ => None,
        },
    };
    if ui.button_enabled(here, &ui.t("menu.birthplace.born_here"), at.is_some())
        && let Some(at) = at
    {
        // Then who begins there (Amendment E §9.1), and the world.
        *push = Some(Screen::Character(crate::character_ui::CharacterScreen {
            then: Some(Box::new(MenuAction::Play {
                folder: choice.folder.clone(),
                seed: choice.seed,
                knowledge: Default::default(),
                era: choice.era.clone(),
                size: choice.size,
                shape: choice.shape,
                birthplace: Some(at),
                mode: Some(choice.mode.clone()),
            })),
            ..Default::default()
        }));
    }
    if ui.button(back, &ui.t("menu.back")) {
        out.push(MenuAction::CancelCreate);
        *pop = true;
    }
}
