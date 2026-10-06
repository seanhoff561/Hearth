//! The journal (J, v2 §12.3): what the person has learned, in their own words with the real
//! history behind it; hunches not yet understood; skills and how practised they are; and
//! everything noted, day by day.

use hearth_craft::{Graph, KnowledgeState, Mode, NoteKind, Skill};
use hearth_ui::widgets::{Column, Rect, theme};
use hearth_ui::{Rgba, Ui};

/// What the journal shows.
pub struct JournalView<'a> {
    pub knowledge: &'a KnowledgeState,
    pub graph: &'a Graph,
    pub mode: Mode,
    pub ticks_per_day: f64,
}

const TABS: [&str; 4] = [
    "journal.tab.known",
    "journal.tab.hunches",
    "journal.tab.skills",
    "journal.tab.notes",
];

/// The journal screen; `tab` and `scroll` are its state. True when it should close.
pub fn journal_screen(ui: &mut Ui<'_>, view: &JournalView, tab: &mut u8, scroll: &mut i32) -> bool {
    let (w, h) = ui.size;
    let pw = (w - 40.0).min(560.0);
    let x = ((w - pw) / 2.0).round();
    ui.title(6.0, &ui.t("journal.title"));
    let mode = ui.t(match view.mode {
        Mode::Discovery => "journal.mode.discovery",
        Mode::Guided => "journal.mode.guided",
        Mode::Open => "journal.mode.open",
    });
    let mw = ui.font.width(&mode) as f32;
    ui.label(((w - mw) / 2.0).round(), 20.0, &mode, theme::DIM);
    // Tabs.
    let tw = ((pw - 3.0 * 4.0) / 4.0).floor();
    for (k, key) in TABS.iter().enumerate() {
        let r = Rect {
            x: x + k as f32 * (tw + 4.0),
            y: 32.0,
            w: tw,
            h: 16.0,
        };
        let label = if *tab as usize == k {
            format!("[{}]", ui.t(key))
        } else {
            ui.t(key)
        };
        if ui.button(r, &label) {
            *tab = k as u8;
            *scroll = 0;
        }
    }
    let lines = lines_of(ui, view, *tab, pw - 8.0);
    let lh = hearth_ui::font::LINE as f32;
    let top = 54.0;
    let rows = ((h - top - 34.0) / lh).floor().max(1.0) as i32;
    let max_scroll = (lines.len() as i32 - rows).max(0);
    let wheel = ui.input.scroll.round() as i32;
    *scroll = (*scroll - wheel * 3).clamp(0, max_scroll);
    ui.draw.rect(
        x,
        top - 2.0,
        pw,
        rows as f32 * lh + 4.0,
        theme::PANEL.with_alpha(200),
    );
    if lines.is_empty() {
        ui.label(x + 4.0, top, &ui.t("journal.empty"), theme::DIM);
    }
    for (k, (text, color)) in lines
        .iter()
        .skip(*scroll as usize)
        .take(rows as usize)
        .enumerate()
    {
        ui.label(x + 4.0, top + k as f32 * lh, text, *color);
    }
    let mut c = Column::new(x + pw / 4.0, h - 28.0, pw / 2.0);
    ui.button(c.row(20.0), &ui.t("menu.done"))
}

/// The lines of a tab, wrapped to `width`.
fn lines_of(ui: &Ui<'_>, view: &JournalView, tab: u8, width: f32) -> Vec<(String, Rgba)> {
    let k = view.knowledge;
    let mut out: Vec<(String, Rgba)> = Vec::new();
    // Wrapped lines keep the indent of their first.
    let push = |text: &str, color: Rgba, out: &mut Vec<(String, Rgba)>| {
        let body = text.trim_start();
        let indent = &text[..text.len() - body.len()];
        let room = (width - ui.font.width(indent) as f32).max(40.0) as u32;
        for l in ui.font.wrap(body, room) {
            out.push((format!("{indent}{l}"), color));
        }
    };
    let day = |tick: u64| (tick as f64 / view.ticks_per_day.max(1.0)).floor() as i64 + 1;
    match tab {
        0 => {
            // Known techniques, by era, each with what is known of its history.
            let mut nodes: Vec<_> = view.graph.nodes.iter().filter(|n| k.knows(&n.id)).collect();
            // By era, then in the order they were learned.
            let when = |id: &str| k.known.get(id).map_or(0, |l| l.tick);
            nodes.sort_by(|a, b| a.era.cmp(&b.era).then(when(&a.id).cmp(&when(&b.id))));
            let mut era = None;
            for n in nodes {
                if era != Some(n.era) {
                    era = Some(n.era);
                    let key = format!("journal.era.{}", n.era);
                    push(&ui.t(&key), theme::TEXT, &mut out);
                }
                let when = k
                    .known
                    .get(&n.id)
                    .map(|l| {
                        ui.lang
                            .format("journal.day", &[("day", &day(l.tick).to_string())])
                    })
                    .unwrap_or_default();
                push(
                    &format!("  {} — {when}", n.name),
                    Rgba([240, 220, 150, 255]),
                    &mut out,
                );
                push(
                    &format!("    {} ({})", n.summary, n.date),
                    theme::DIM,
                    &mut out,
                );
            }
            if !k.legends.is_empty() {
                push(&ui.t("journal.legend"), theme::TEXT, &mut out);
                for id in &k.legends {
                    let name = view.graph.node(id).map_or(id.as_str(), |n| n.name.as_str());
                    push(&format!("  {name}"), theme::DIM, &mut out);
                }
            }
        }
        1 => {
            for n in k.journal.iter().rev().filter(|n| n.kind == NoteKind::Hunch) {
                let pending = n.node.as_ref().is_some_and(|id| !k.knows(id));
                if pending {
                    push(
                        &format!("— {}", n.text),
                        Rgba([200, 210, 240, 255]),
                        &mut out,
                    );
                }
            }
        }
        2 => {
            for (name, s) in &k.skills {
                let line = ui.lang.format(
                    "journal.skill",
                    &[
                        ("skill", name),
                        ("level", Skill::words(s.level)),
                        ("hours", &format!("{:.1}", s.hours)),
                    ],
                );
                push(&line, theme::TEXT, &mut out);
            }
        }
        _ => {
            let mut last_day = None;
            for n in k.journal.iter().rev() {
                let d = day(n.tick);
                if last_day != Some(d) {
                    last_day = Some(d);
                    push(
                        &ui.lang.format("journal.day", &[("day", &d.to_string())]),
                        theme::TEXT,
                        &mut out,
                    );
                }
                let color = match n.kind {
                    NoteKind::Discovery => Rgba([240, 220, 150, 255]),
                    NoteKind::Hunch => Rgba([200, 210, 240, 255]),
                    NoteKind::Legend | NoteKind::PastLife => theme::DIM,
                    NoteKind::Made => Rgba([220, 225, 215, 255]),
                };
                push(&format!("  {}", n.text), color, &mut out);
            }
        }
    }
    out
}
