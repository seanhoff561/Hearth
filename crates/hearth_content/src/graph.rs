//! `hearth content graph`: the knowledge graph, the process graph (material flows) and the
//! food webs as Graphviz DOT and as self-contained SVG (simple layered layout, no external
//! tools), with an HTML index in `docs/generated/`.

use std::fmt::Write as _;
use std::path::Path;

use rustc_hash::FxHashMap;

use crate::content::Content;
use crate::schema::process::Match;
use crate::schema::{Entry, Status};

/// Node kinds (drawn with different colours).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Knowledge(u8),
    Process,
    Item,
    Material,
    Plant,
    Animal,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub kind: Kind,
    pub planned: bool,
}

/// A directed graph; edges point from prerequisite/input/food to dependent/output/eater.
#[derive(Debug, Clone, Default)]
pub struct Graph {
    pub title: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<(usize, usize)>,
    index: FxHashMap<String, usize>,
}

impl Graph {
    fn new(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            ..Default::default()
        }
    }

    fn node(&mut self, id: &str, label: &str, kind: Kind, planned: bool) -> usize {
        if let Some(&i) = self.index.get(id) {
            return i;
        }
        self.nodes.push(Node {
            id: id.to_owned(),
            label: label.to_owned(),
            kind,
            planned,
        });
        self.index.insert(id.to_owned(), self.nodes.len() - 1);
        self.nodes.len() - 1
    }

    fn edge(&mut self, a: usize, b: usize) {
        if a != b && !self.edges.contains(&(a, b)) {
            self.edges.push((a, b));
        }
    }
}

pub fn knowledge_graph(c: &Content) -> Graph {
    let mut g = Graph::new("Knowledge graph");
    for k in c.knowledge.iter() {
        g.node(
            k.id(),
            &k.name,
            Kind::Knowledge(k.era),
            k.status == Status::Planned,
        );
    }
    for k in c.knowledge.iter() {
        let b = g.index[k.id()];
        for r in &k.requires {
            if let Some(&a) = g.index.get(r.as_str()) {
                g.edge(a, b);
            }
        }
    }
    g
}

fn match_node(g: &mut Graph, c: &Content, m: &Match) -> usize {
    match m {
        Match::Item(r) => {
            let label = c
                .items
                .get(r.as_str())
                .map_or(r.path(), |i| i.name.as_str())
                .to_owned();
            g.node(r.as_str(), &label, Kind::Item, false)
        }
        Match::Form { form, .. } => {
            let label = c
                .forms
                .get(form.as_str())
                .map_or(form.path().to_owned(), |f| {
                    f.name.replace("{material}", "any").trim().to_owned()
                });
            g.node(form.as_str(), &label, Kind::Item, false)
        }
        Match::Material(r) => {
            let label = c
                .materials
                .get(r.as_str())
                .map_or(r.path(), |m| m.name.as_str())
                .to_owned();
            g.node(r.as_str(), &label, Kind::Material, false)
        }
        Match::Tag(t) => g.node(&format!("#{t}"), &format!("any {t}"), Kind::Item, false),
        Match::Garment(r) => {
            let label = c.garments.get(r.as_str()).map_or(r.path().to_owned(), |g| {
                g.name.replace("{material}", "any").trim().to_owned()
            });
            g.node(r.as_str(), &label, Kind::Item, false)
        }
    }
}

pub fn process_graph(c: &Content) -> Graph {
    let mut g = Graph::new("Process graph (material flows)");
    for p in c.processes.iter() {
        let pn = g.node(p.id(), &p.name, Kind::Process, p.status == Status::Planned);
        for i in &p.inputs {
            let a = match_node(&mut g, c, &i.item);
            g.edge(a, pn);
        }
        for o in p.outputs.iter().chain(&p.byproducts) {
            let b = match_node(&mut g, c, &o.item);
            g.edge(pn, b);
        }
    }
    g
}

pub fn food_web(c: &Content, ecosystem: &str) -> Option<Graph> {
    let eco = c.ecosystems.get(ecosystem)?;
    let mut g = Graph::new(&format!("Food web: {}", eco.name));
    for p in &eco.producers {
        let label = c
            .plants
            .get(p.as_str())
            .map_or(p.path(), |x| x.name.as_str())
            .to_owned();
        g.node(p.as_str(), &label, Kind::Plant, false);
    }
    for a in c.animals.iter().filter(|a| {
        a.habitat
            .iter()
            .chain(&a.also_in)
            .any(|h| h.as_str() == eco.id())
    }) {
        let an = g.node(a.id(), &a.name, Kind::Animal, a.status == Status::Planned);
        for f in &a.diet.foods {
            let food = if let Some(p) = c.plants.get(f.food.as_str()) {
                g.node(f.food.as_str(), &p.name, Kind::Plant, false)
            } else if let Some(prey) = c.animals.get(f.food.as_str()) {
                g.node(
                    f.food.as_str(),
                    &prey.name,
                    Kind::Animal,
                    prey.status == Status::Planned,
                )
            } else {
                let label = c
                    .materials
                    .get(f.food.as_str())
                    .map_or(f.food.path(), |m| m.name.as_str())
                    .to_owned();
                g.node(f.food.as_str(), &label, Kind::Material, false)
            };
            g.edge(food, an);
        }
    }
    Some(g)
}

fn color(kind: Kind) -> &'static str {
    match kind {
        Kind::Knowledge(era) => [
            "#e8d8b0", "#e2c98e", "#d8b56c", "#b9cf8a", "#9fc6b4", "#a7b8dc", "#c3aede", "#dca9c2",
            "#e0a7a7",
        ][era.min(8) as usize],
        Kind::Process => "#f2f2f2",
        Kind::Item => "#d6e4f0",
        Kind::Material => "#e6dccb",
        Kind::Plant => "#cfe8c4",
        Kind::Animal => "#f0d0c0",
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn to_dot(g: &Graph) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "digraph \"{}\" {{", g.title.replace('"', "'"));
    let _ = writeln!(
        s,
        "  rankdir=LR; node [shape=box, style=\"rounded,filled\", fontname=\"sans-serif\"];"
    );
    for (i, n) in g.nodes.iter().enumerate() {
        let _ = writeln!(
            s,
            "  n{i} [label=\"{}\", fillcolor=\"{}\"{}];",
            n.label.replace('"', "'"),
            color(n.kind),
            if n.planned {
                ", style=\"rounded,filled,dashed\""
            } else {
                ""
            }
        );
    }
    for (a, b) in &g.edges {
        let _ = writeln!(s, "  n{a} -> n{b};");
    }
    s.push_str("}\n");
    s
}

/// Longest-path layering, then barycentre ordering sweeps.
fn layout(g: &Graph) -> (Vec<usize>, Vec<Vec<usize>>) {
    let n = g.nodes.len();
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut succs: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(a, b) in &g.edges {
        preds[b].push(a);
        succs[a].push(b);
    }
    // Kahn's algorithm; nodes left in cycles get layer 0 relative to their visited preds.
    let mut indeg: Vec<usize> = preds.iter().map(Vec::len).collect();
    let mut layer = vec![0usize; n];
    let mut queue: Vec<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
    let mut head = 0;
    while head < queue.len() {
        let v = queue[head];
        head += 1;
        for &w in &succs[v] {
            layer[w] = layer[w].max(layer[v] + 1);
            indeg[w] -= 1;
            if indeg[w] == 0 {
                queue.push(w);
            }
        }
    }
    let depth = layer.iter().copied().max().unwrap_or(0) + 1;
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); depth];
    for (v, &l) in layer.iter().enumerate() {
        layers[l].push(v);
    }
    let mut pos = vec![0f32; n];
    for l in &layers {
        for (i, &v) in l.iter().enumerate() {
            pos[v] = i as f32;
        }
    }
    for sweep in 0..6 {
        let forward = sweep % 2 == 0;
        let order: Vec<usize> = if forward {
            (1..depth).collect()
        } else {
            (0..depth.saturating_sub(1)).rev().collect()
        };
        for li in order {
            let neigh = if forward { &preds } else { &succs };
            let mut keyed: Vec<(f32, usize)> = layers[li]
                .iter()
                .map(|&v| {
                    let ns = &neigh[v];
                    let k = if ns.is_empty() {
                        pos[v]
                    } else {
                        ns.iter().map(|&u| pos[u]).sum::<f32>() / ns.len() as f32
                    };
                    (k, v)
                })
                .collect();
            keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
            layers[li] = keyed.into_iter().map(|(_, v)| v).collect();
            for (i, &v) in layers[li].iter().enumerate() {
                pos[v] = i as f32;
            }
        }
    }
    (layer, layers)
}

pub fn to_svg(g: &Graph) -> String {
    const W: f32 = 190.0;
    const H: f32 = 30.0;
    const GX: f32 = 70.0;
    const GY: f32 = 12.0;
    let (layer, layers) = layout(g);
    let tallest = layers.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let width = layers.len() as f32 * (W + GX) + GX;
    let height = tallest as f32 * (H + GY) + GY + 40.0;
    let mut xy = vec![(0f32, 0f32); g.nodes.len()];
    for (li, l) in layers.iter().enumerate() {
        // Centre each column vertically.
        let offset = (tallest - l.len()) as f32 * (H + GY) / 2.0;
        for (i, &v) in l.iter().enumerate() {
            xy[v] = (
                GX / 2.0 + li as f32 * (W + GX),
                40.0 + offset + i as f32 * (H + GY),
            );
        }
    }
    let _ = layer;
    let mut s = String::new();
    let _ = write!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" \
         viewBox=\"0 0 {width:.0} {height:.0}\" font-family=\"sans-serif\" font-size=\"11\">\
         <rect width=\"100%\" height=\"100%\" fill=\"#fcfbf8\"/>\
         <text x=\"12\" y=\"24\" font-size=\"16\" font-weight=\"bold\">{}</text>",
        esc(&g.title)
    );
    for &(a, b) in &g.edges {
        let (ax, ay) = xy[a];
        let (bx, by) = xy[b];
        let (x1, y1, x2, y2) = (ax + W, ay + H / 2.0, bx, by + H / 2.0);
        let dx = ((x2 - x1) / 2.0).max(30.0);
        let _ = write!(
            s,
            "<path d=\"M{x1:.1},{y1:.1} C{:.1},{y1:.1} {:.1},{y2:.1} {x2:.1},{y2:.1}\" fill=\"none\" \
             stroke=\"#8c8c8c\" stroke-width=\"1\" opacity=\"0.7\"/>",
            x1 + dx,
            x2 - dx
        );
    }
    for (i, n) in g.nodes.iter().enumerate() {
        let (x, y) = xy[i];
        let label = if n.label.chars().count() > 30 {
            format!("{}…", n.label.chars().take(29).collect::<String>())
        } else {
            n.label.clone()
        };
        let _ = write!(
            s,
            "<g><title>{}</title><rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{W}\" height=\"{H}\" rx=\"6\" \
             fill=\"{}\" stroke=\"#555\" stroke-width=\"1\"{}/>\
             <text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text></g>",
            esc(&n.id),
            color(n.kind),
            if n.planned {
                " stroke-dasharray=\"4 3\""
            } else {
                ""
            },
            x + W / 2.0,
            y + H / 2.0 + 4.0,
            esc(&label)
        );
    }
    s.push_str("</svg>\n");
    s
}

/// Writes every graph as `.dot` and `.svg` plus `index.html` into `dir`.
pub fn write_all(c: &Content, dir: &Path) -> std::io::Result<Vec<String>> {
    std::fs::create_dir_all(dir)?;
    let mut graphs = vec![
        ("knowledge".to_owned(), knowledge_graph(c)),
        ("processes".to_owned(), process_graph(c)),
    ];
    for e in c.ecosystems.iter() {
        if let Some(g) = food_web(c, e.id()) {
            graphs.push((format!("foodweb_{}", e.id().replace([':', '/'], "_")), g));
        }
    }
    let mut written = Vec::new();
    let mut html = String::from(
        "<!doctype html><meta charset=\"utf-8\"><title>Content graphs</title>\
         <style>body{font-family:sans-serif;margin:16px;background:#f6f4ef}\
         section{margin:24px 0}div.g{overflow:auto;border:1px solid #ccc;background:#fff}</style>\
         <h1>Content graphs</h1><p>Generated by <code>hearth content graph</code>. Dashed boxes are \
         planned (data only). Hover a box for its id.</p>",
    );
    for (name, g) in &graphs {
        let svg = to_svg(g);
        std::fs::write(dir.join(format!("{name}.dot")), to_dot(g))?;
        std::fs::write(dir.join(format!("{name}.svg")), &svg)?;
        let _ = write!(
            html,
            "<section><h2>{}</h2><p>{} nodes, {} edges · <a href=\"{name}.svg\">svg</a> · \
             <a href=\"{name}.dot\">dot</a></p><div class=\"g\">{svg}</div></section>",
            esc(&g.title),
            g.nodes.len(),
            g.edges.len()
        );
        written.push(name.clone());
    }
    std::fs::write(dir.join("index.html"), html)?;
    Ok(written)
}
