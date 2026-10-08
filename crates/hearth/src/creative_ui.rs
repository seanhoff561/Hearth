//! Creative's inventory screen (Amendment P §3.1): the categories as tabs, a search by name,
//! the things found in a list, and what to do with the one chosen — take an item into the hands
//! (as many as asked), place a block or a plant, summon animals (of a sex and an age, one or a
//! group), build a piece, learn a piece of knowledge.

use hearth_ui::Ui;
use hearth_ui::widgets::{Rect, theme};

use crate::creative::{Category, Entry, search};

/// What the screen keeps between frames.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CreativeScreen {
    pub tab: usize,
    pub query: String,
    pub selected: Option<usize>,
    /// How many to take or summon (an index into [`COUNTS`]).
    pub count: usize,
    /// For animals: a female; for animals and plants: young.
    pub female: bool,
    pub young: bool,
    /// For knowledge: forgetting rather than learning.
    pub forget: bool,
}

/// How many may be taken or summoned at once.
pub const COUNTS: [u32; 4] = [1, 4, 8, 16];

pub use hearth_protocol::CreativeAct;

/// Where the screen goes next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    Stay,
    Close,
    /// To the things carried (the normal inventory).
    Carried,
}

const ROW: f32 = 18.0;

/// Draws the screen; returns what was asked and where the screen goes next. `instant` is
/// Creative's instant actions, toggled here.
pub fn creative_screen(
    ui: &mut Ui<'_>,
    all: &[Entry],
    st: &mut CreativeScreen,
    instant: &mut bool,
) -> (Option<CreativeAct>, Next) {
    let (w, h) = ui.size;
    let wide = (w - 16.0).min(420.0);
    let x = ((w - wide) / 2.0).round();
    ui.title(6.0, &ui.t("creative.title"));
    let names: Vec<String> = Category::ALL.iter().map(|c| ui.t(c.key())).collect();
    let before = st.tab;
    ui.tabs(Rect::new(x, 20.0, wide, ROW), &names, &mut st.tab);
    let field = Rect::new(x, 42.0, wide, ROW);
    if ui.text_field(field, &ui.t("creative.search"), &mut st.query, 40) || st.tab != before {
        st.selected = None;
    }
    let category = Category::ALL[st.tab.min(Category::ALL.len() - 1)];
    let found = search(all, category, &st.query);
    // The list, and below it the options and the buttons (three rows).
    let foot = 3.0 * (ROW + 4.0) + 6.0;
    let list = Rect::new(x, 64.0, wide, (h - 64.0 - foot).max(30.0));
    let shown: Vec<String> = found.iter().map(|e| e.name.clone()).collect();
    if let Some(i) = ui.list(
        list,
        "creative",
        shown.len(),
        12.0,
        st.selected,
        |ui, r, i, _| {
            let mut t = shown[i].clone();
            while ui.font.width(&t) as f32 > r.w - 12.0 && t.pop().is_some() {}
            ui.label(r.x + 4.0, r.y + 2.0, &t, theme::TEXT);
        },
    ) {
        st.selected = Some(i);
    }
    if shown.is_empty() {
        let none = ui.t("creative.none");
        ui.text_centred(&list, &none, theme::DIM);
    }
    let chosen = st.selected.and_then(|i| found.get(i)).copied();
    let mut y = list.y + list.h + 6.0;
    let half = ((wide - 4.0) / 2.0).floor();
    // The options: how many, and for animals which.
    let counts: Vec<String> = COUNTS.iter().map(|n| n.to_string()).collect();
    let row = Rect::new(x, y, wide, ROW);
    let (a, b) = row.split_left(half, 4.0);
    if matches!(category, Category::Items | Category::Animals) {
        ui.cycle(a, &ui.t("creative.count"), &counts, &mut st.count);
    }
    if category == Category::Animals {
        let sexes = [ui.t("creative.male"), ui.t("creative.female")];
        let mut s = usize::from(st.female);
        if ui.cycle(b, &ui.t("creative.sex"), &sexes, &mut s) {
            st.female = s == 1;
        }
    }
    y += ROW + 4.0;
    let row = Rect::new(x, y, wide, ROW);
    let (a, b) = row.split_left(half, 4.0);
    ui.toggle(b, &ui.t("creative.instant"), instant);
    if category == Category::Knowledge {
        let ways = [ui.t("creative.learn"), ui.t("creative.forget")];
        let mut s = usize::from(st.forget);
        if ui.cycle(a, &ui.t("creative.knowing"), &ways, &mut s) {
            st.forget = s == 1;
        }
    }
    if matches!(category, Category::Animals | Category::Plants) {
        let ages = [ui.t("creative.grown"), ui.t("creative.young")];
        let mut s = usize::from(st.young);
        if ui.cycle(a, &ui.t("creative.age"), &ages, &mut s) {
            st.young = s == 1;
        }
    }
    y += ROW + 4.0;
    let row = Rect::new(x, y, wide, ROW);
    let third = ((wide - 8.0) / 3.0).floor();
    let (a, rest) = row.split_left(third, 4.0);
    let (c, b) = rest.split_left(third, 4.0);
    let verb = match category {
        Category::Terrain | Category::Plants | Category::Building => "creative.place",
        Category::Animals => "creative.summon",
        Category::Items => "creative.take",
        Category::Knowledge if st.forget => "creative.forget",
        Category::Knowledge => "creative.learn",
    };
    let mut act = None;
    if ui.button_enabled(a, &ui.t(verb), chosen.is_some())
        && let Some(e) = chosen
    {
        let count = COUNTS[st.count.min(COUNTS.len() - 1)];
        act = Some(match category {
            Category::Terrain => CreativeAct::Place {
                block: e.id.clone(),
            },
            Category::Plants => CreativeAct::Plant {
                species: e.id.clone(),
                young: st.young,
            },
            Category::Animals => CreativeAct::Summon {
                species: e.id.clone(),
                female: st.female,
                young: st.young,
                count,
            },
            Category::Items => CreativeAct::Take {
                item: e.id.clone(),
                count,
            },
            Category::Building => CreativeAct::Build {
                piece: e.id.clone(),
            },
            Category::Knowledge => CreativeAct::Learn {
                node: e.id.clone(),
                known: !st.forget,
            },
        });
    }
    let next = if ui.button(c, &ui.t("creative.carried")) {
        Next::Carried
    } else if ui.button(b, &ui.t("menu.done")) {
        Next::Close
    } else {
        Next::Stay
    };
    (act, next)
}
