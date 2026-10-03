//! Watching (V2-11 (d)): what a person learns from the agents. Seeing a process done shows the
//! thing done and what it rests on — the observation routes (`watch:hominin_*`) of the process's
//! knowledge node and of the nodes it requires, as the graph has them: a hominin cracking marula
//! stones on its anvil shows the hammer, the anvil and the cracking; one striking flakes shows the
//! hammer and the knapping. What they leave lying shows something too: a scatter of flakes and
//! cores is studied (`study:tool_scatter`).

use glam::DVec3;
use hearth_content::schema::knowledge::Route;
use hearth_craft::{Crafts, Graph};

use crate::sim::Done;

/// How far off a person sees the work of hands (m).
pub const WATCH_M: f64 = 40.0;
/// What a person watches lies within this of the way they face (the cosine of 60°).
const VIEW_COS: f64 = 0.5;
/// A scatter is studied from within this (m) …
const SCATTER_M: f64 = 3.0;
/// … of this many flakes and cores.
const SCATTER_PIECES: u32 = 3;

/// The triggers seeing a process done gives: the observation routes of hominins of its
/// knowledge node and of all the nodes that node requires.
pub fn watched(graph: &Graph, crafts: &Crafts, recipe: usize) -> Vec<String> {
    let Some(start) = crafts.recipes[recipe]
        .def
        .knowledge
        .as_ref()
        .and_then(|k| graph.nodes.iter().position(|n| n.id == k.as_str()))
    else {
        return Vec::new();
    };
    let mut nodes = vec![start];
    let mut out: Vec<String> = Vec::new();
    let mut k = 0;
    while k < nodes.len() {
        let n = &graph.nodes[nodes[k]];
        for r in &n.routes {
            if r.route == Route::Observation
                && r.trigger.starts_with("watch:hominin_")
                && !out.contains(&r.trigger)
            {
                out.push(r.trigger.clone());
            }
        }
        for &q in &n.requires {
            if !nodes.contains(&q) {
                nodes.push(q);
            }
        }
        k += 1;
    }
    out
}

/// What a person at `eye`, facing `yaw` (radians: 0 toward +z, turning toward +x), sees done in
/// a step: the triggers of the work finished within [`WATCH_M`] in front of them.
pub fn seen(done: &[Done], graph: &Graph, crafts: &Crafts, eye: DVec3, yaw: f32) -> Vec<String> {
    let facing = DVec3::new(yaw.sin() as f64, 0.0, yaw.cos() as f64);
    let mut out: Vec<String> = Vec::new();
    for d in done {
        let to = d.at - eye;
        let flat = DVec3::new(to.x, 0.0, to.z);
        let dist = flat.length();
        if dist > WATCH_M || (dist > 1.0 && flat.dot(facing) / dist < VIEW_COS) {
            continue;
        }
        for t in watched(graph, crafts, d.recipe) {
            if !out.contains(&t) {
                out.push(t);
            }
        }
    }
    out
}

/// Whether a scatter of knapped stone lies about a place: of the things lying about (where,
/// what, how many), [`SCATTER_PIECES`] flakes and cores within [`SCATTER_M`].
pub fn scatter_near<'a>(lying: impl Iterator<Item = (DVec3, &'a str, u16)>, at: DVec3) -> bool {
    let pieces: u32 = lying
        .filter(|(p, _, _)| (*p - at).length() <= SCATTER_M)
        .filter(|(_, id, _)| {
            let form = id.split('/').next().unwrap_or("");
            let form = form.rsplit(':').next().unwrap_or(form);
            form == "flake" || form == "core"
        })
        .map(|(_, _, n)| n as u32)
        .sum();
    pieces >= SCATTER_PIECES
}
