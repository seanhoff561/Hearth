//! The inventory screen (v2 §10.4, Tab): the body's places — hands, back, each garment and its
//! attachment points, a drag — and a grid for every container carried (one held in a hand
//! opens). A thing is lifted onto the pointer by a click and placed by another; the right
//! button turns a lifted thing or lifts half a stack; a click outside puts it down on the
//! ground. Below, what it all weighs against the body.

use hearth_items::{Carry, Hand, Items, Path, Root, Target};
use hearth_ui::widgets::theme;
use hearth_ui::{Rect, Rgba, Ui};

use crate::menus::MenuAction;

/// What the inventory screen shows.
pub struct InventoryView<'a> {
    pub carry: &'a Carry,
    pub items: &'a Items,
    pub body_kg: f32,
}

/// A thing on the pointer: where it is carried, and how many of a stack.
#[derive(Debug, Clone, PartialEq)]
pub struct Lifted {
    pub from: Path,
    pub count: Option<u16>,
}

/// Grid cell size (interface pixels).
const CELL: f32 = 13.0;
const ROW: f32 = 15.0;

/// A place on the body that holds one thing.
struct Place {
    label: String,
    root: Root,
    indent: bool,
}

/// Draws the screen; what the player did becomes menu actions. True to close.
pub fn inventory_screen(
    ui: &mut Ui<'_>,
    view: &InventoryView<'_>,
    lifted: &mut Option<Lifted>,
    turned: &mut bool,
    out: &mut Vec<MenuAction>,
) -> bool {
    let (w, h) = ui.size;
    let c = view.carry;
    let items = view.items;
    ui.title(6.0, &ui.t("inv.title"));
    let pointer = ui.input.pointer;
    let clicked = ui.input.pressed;
    let alt = ui.input.alt_pressed;
    let mut used = false;
    let mut hover: Option<Path> = None;

    // The body's places.
    let mut places = Vec::new();
    if c.both {
        places.push(Place {
            label: ui.t("inv.both"),
            root: Root::Hand(Hand::Right),
            indent: false,
        });
    } else {
        places.push(Place {
            label: ui.t("inv.left"),
            root: Root::Hand(Hand::Left),
            indent: false,
        });
        places.push(Place {
            label: ui.t("inv.right"),
            root: Root::Hand(Hand::Right),
            indent: false,
        });
    }
    places.push(Place {
        label: ui.t("inv.back"),
        root: Root::Back,
        indent: false,
    });
    for (i, worn) in c.worn.iter().enumerate() {
        let kind = worn.stack.kind(items);
        places.push(Place {
            label: ui.t("inv.wearing"),
            root: Root::Worn(i),
            indent: false,
        });
        let points = kind
            .and_then(|k| k.wear.as_ref())
            .map(|wear| wear.attachments.clone())
            .unwrap_or_default();
        for (p, point) in points.iter().enumerate() {
            places.push(Place {
                label: ui.t(&format!("inv.point.{point}")),
                root: Root::Hung(i, p),
                indent: true,
            });
        }
    }
    if c.dragging.is_some() {
        places.push(Place {
            label: ui.t("inv.dragging"),
            root: Root::Dragging,
            indent: false,
        });
    }
    let (x0, mut y) = (10.0f32, 26.0f32);
    let col = 190.0f32;
    ui.draw
        .rect(x0 - 4.0, y - 4.0, col, h - y - 26.0, theme::PANEL);
    for place in &places {
        let path = Path::at(place.root);
        let lx = x0 + if place.indent { 10.0 } else { 0.0 };
        ui.label(lx, y + 3.0, &place.label, theme::DIM);
        let slot = Rect::new(x0 + 78.0, y, col - 86.0, ROW - 2.0);
        let held = c.get(&path);
        let lifted_here = lifted.as_ref().is_some_and(|l| l.from == path);
        let fill = if lifted_here {
            theme::BUTTON.with_alpha(90)
        } else if pointer.is_some_and(|p| slot.contains(p)) {
            theme::BUTTON_HOT
        } else {
            theme::FIELD
        };
        ui.draw.rect(slot.x, slot.y, slot.w, slot.h, fill);
        let words = match held.and_then(|s| s.kind(items)) {
            Some(k) => {
                let n = held.map_or(1, |s| s.count);
                let name = if n > 1 {
                    format!("{} ×{n}", k.name)
                } else {
                    k.name.clone()
                };
                fit(ui, &name, slot.w - 6.0)
            }
            None => ui.t("inv.empty"),
        };
        ui.label(slot.x + 3.0, slot.y + 3.0, &words, theme::TEXT);
        if pointer.is_some_and(|p| slot.contains(p)) {
            if held.is_some() {
                hover = Some(path.clone());
            }
            if clicked {
                used = true;
                match lifted.take() {
                    Some(l) => out.push(MenuAction::Shift {
                        from: l.from,
                        count: l.count,
                        to: Target::Root(place.root),
                    }),
                    None if held.is_some() => {
                        *lifted = Some(Lifted {
                            from: path.clone(),
                            count: None,
                        });
                        *turned = false;
                    }
                    None => {}
                }
            } else if alt
                && lifted.is_none()
                && let Some(s) = held
                && s.count > 1
            {
                used = true;
                *lifted = Some(Lifted {
                    from: path.clone(),
                    count: Some(s.count / 2),
                });
            }
        }
        y += ROW;
    }

    // Containers carried: their grids.
    let mut containers: Vec<(Path, String)> = Vec::new();
    for root in [Root::Hand(Hand::Left), Root::Hand(Hand::Right), Root::Back] {
        let p = Path::at(root);
        if let Some(s) = c.get(&p)
            && let Some(k) = s.kind(items)
            && k.container
                .is_some_and(|spec| spec.grid.0 > 0 && spec.grid.1 > 0)
        {
            containers.push((p, k.name.clone()));
        }
    }
    for (i, worn) in c.worn.iter().enumerate() {
        for (pt, hung) in worn.hung.iter().enumerate() {
            if let Some(s) = hung
                && let Some(k) = s.kind(items)
                && k.container
                    .is_some_and(|spec| spec.grid.0 > 0 && spec.grid.1 > 0)
            {
                containers.push((Path::at(Root::Hung(i, pt)), k.name.clone()));
            }
        }
    }
    let (mut gx, mut gy) = (x0 + col + 8.0, 26.0f32);
    let mut row_h = 0.0f32;
    for (path, name) in &containers {
        let Some(spec) = c
            .get(path)
            .and_then(|s| s.kind(items))
            .and_then(|k| k.container)
        else {
            continue;
        };
        let (cw, ch) = (spec.grid.0 as f32 * CELL, spec.grid.1 as f32 * CELL);
        let pw = cw.max(ui.font.width(name) as f32 + 4.0) + 8.0;
        if gx + pw > w - 8.0 {
            gx = x0 + col + 8.0;
            gy += row_h + 8.0;
            row_h = 0.0;
        }
        ui.draw
            .rect(gx - 4.0, gy - 4.0, pw, ch + 18.0, theme::PANEL);
        ui.label(gx, gy - 1.0, name, theme::DIM);
        let (ox, oy) = (gx, gy + 11.0);
        for yy in 0..spec.grid.1 {
            for xx in 0..spec.grid.0 {
                ui.draw.rect(
                    ox + xx as f32 * CELL,
                    oy + yy as f32 * CELL,
                    CELL - 1.0,
                    CELL - 1.0,
                    theme::FIELD,
                );
            }
        }
        let inside = c.get(path).and_then(|s| s.contents());
        if let Some(inside) = inside {
            for (idx, placed) in inside.items.iter().enumerate() {
                let Some(k) = placed.stack.kind(items) else {
                    continue;
                };
                let (fw, fh) = if placed.turned {
                    (k.footprint.1, k.footprint.0)
                } else {
                    k.footprint
                };
                let r = Rect::new(
                    ox + placed.x as f32 * CELL,
                    oy + placed.y as f32 * CELL,
                    fw as f32 * CELL - 1.0,
                    fh as f32 * CELL - 1.0,
                );
                let here = path.inner(idx);
                let dim = lifted.as_ref().is_some_and(|l| l.from == here);
                let [cr, cg, cb] = k.color;
                ui.draw.rect(
                    r.x,
                    r.y,
                    r.w,
                    r.h,
                    Rgba([cr, cg, cb, if dim { 90 } else { 230 }]),
                );
                if placed.stack.count > 1 {
                    let n = placed.stack.count.to_string();
                    ui.label(r.x + 1.0, r.y + r.h - 9.0, &n, theme::TEXT);
                }
                if pointer.is_some_and(|p| r.contains(p)) {
                    hover = Some(here);
                }
            }
        }
        // Clicks on the grid: lift what is there, or place what is lifted.
        let grid = Rect::new(ox, oy, cw, ch);
        if let Some(p) = pointer
            && grid.contains(p)
        {
            let cx = ((p.0 - ox) / CELL).floor().clamp(0.0, 255.0) as u8;
            let cy = ((p.1 - oy) / CELL).floor().clamp(0.0, 255.0) as u8;
            let under = inside.and_then(|b| b.at(items, cx, cy));
            if clicked {
                used = true;
                match lifted.take() {
                    Some(l) => out.push(MenuAction::Shift {
                        from: l.from,
                        count: l.count,
                        to: Target::Cell {
                            container: path.clone(),
                            x: cx,
                            y: cy,
                            turned: *turned,
                        },
                    }),
                    None => {
                        if let Some(i) = under {
                            *lifted = Some(Lifted {
                                from: path.inner(i),
                                count: None,
                            });
                            *turned = inside
                                .and_then(|b| b.items.get(i))
                                .is_some_and(|p| p.turned);
                        }
                    }
                }
            } else if alt
                && lifted.is_none()
                && let Some(i) = under
                && let Some(n) = inside.and_then(|b| b.items.get(i)).map(|p| p.stack.count)
                && n > 1
            {
                used = true;
                *lifted = Some(Lifted {
                    from: path.inner(i),
                    count: Some(n / 2),
                });
            }
        }
        gx += pw + 8.0;
        row_h = row_h.max(ch + 18.0);
    }

    // A lifted thing: turned by the right button, put down by a click elsewhere.
    if let Some(l) = lifted.clone() {
        if alt && !used {
            *turned = !*turned;
        } else if clicked && !used {
            out.push(MenuAction::PutDown {
                from: l.from.clone(),
                count: l.count,
            });
            *lifted = None;
        }
        if let (Some(p), Some(k)) = (pointer, c.get(&l.from).and_then(|s| s.kind(items))) {
            let (fw, fh) = if *turned {
                (k.footprint.1, k.footprint.0)
            } else {
                k.footprint
            };
            let [cr, cg, cb] = k.color;
            ui.draw.rect(
                p.0 - CELL / 2.0,
                p.1 - CELL / 2.0,
                fw as f32 * CELL - 1.0,
                fh as f32 * CELL - 1.0,
                Rgba([cr, cg, cb, 170]),
            );
        }
    }

    // What the pointer rests on.
    if lifted.is_none()
        && let (Some(path), Some(p)) = (hover, pointer)
        && let Some(s) = c.get(&path)
        && let Some(k) = s.kind(items)
    {
        let mut lines = vec![k.name.clone()];
        if s.count > 1 {
            lines.push(ui.lang.format("inv.count", &[("n", &s.count.to_string())]));
        }
        lines.push(ui.lang.format("inv.mass", &[("kg", &kg(s.mass(items)))]));
        let tw = lines.iter().map(|l| ui.font.width(l)).max().unwrap_or(0) as f32 + 6.0;
        let th = lines.len() as f32 * 10.0 + 4.0;
        let (tx, ty) = (
            (p.0 + 10.0).min(w - tw - 2.0),
            (p.1 + 10.0).min(h - th - 2.0),
        );
        ui.draw.rect(tx, ty, tw, th, Rgba([8, 8, 10, 235]));
        for (k, line) in lines.iter().enumerate() {
            ui.label(tx + 3.0, ty + 2.0 + k as f32 * 10.0, line, theme::TEXT);
        }
    }

    // What it all weighs.
    let load = c.load(items, view.body_kg);
    let words = if load.share < 0.1 {
        "inv.load.light"
    } else if load.share < 0.2 {
        "inv.load.fine"
    } else if load.share < 0.4 {
        "inv.load.slow"
    } else if load.share < 0.5 {
        "inv.load.heavy"
    } else {
        "inv.load.too_heavy"
    };
    let mut line = ui.lang.format(
        "inv.load",
        &[
            ("kg", &kg(load.carried_kg)),
            ("pct", &format!("{:.0}", load.share * 100.0)),
            ("words", &ui.t(words)),
        ],
    );
    if load.dragged_kg > 0.0 {
        line = format!(
            "{line}; {}",
            ui.lang.format("inv.drag", &[("kg", &kg(load.dragged_kg))])
        );
    }
    ui.label(x0, h - 20.0, &line, theme::TEXT);
    let hint = ui.t("inv.hint");
    ui.label(x0, h - 11.0, &hint, theme::DIM);
    false
}

fn kg(v: f32) -> String {
    if v < 1.0 {
        format!("{:.2}", v)
    } else {
        format!("{:.1}", v)
    }
}

/// Text cut to a width, with an ellipsis.
fn fit(ui: &Ui<'_>, text: &str, width: f32) -> String {
    if (ui.font.width(text) as f32) <= width {
        return text.to_owned();
    }
    let mut s = String::new();
    for ch in text.chars() {
        let next = format!("{s}{ch}…");
        if ui.font.width(&next) as f32 > width {
            break;
        }
        s.push(ch);
    }
    format!("{s}…")
}
