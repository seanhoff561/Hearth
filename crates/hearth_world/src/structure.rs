//! Whether what is built stands (V2-8 (b), docs/design/building.md "Stability").
//!
//! The pieces put up are members of a structure. Posts, and walls, whole blocks and panels with
//! something under them, are columns: they carry what rests on them down to what they stand
//! on, and are crushed or (a slender post) buckle when it is too much. Beams, lintels, layers
//! and roofs — and walls, blocks or panels with nothing under them — lie in runs along a line
//! (a roof's along its slope), spanning between what holds them up: what is under any of them,
//! and what their ends rest against. A run bends under its own weight and what rests on it, and
//! breaks where the moment is more than its pieces bear, or more than the joint between two of
//! them carries — stones laid end to end carry none, so a run of slabs cannot span — or where
//! it reaches further than it can without sagging past use. The natural ground, and land not
//! loaded, holds whatever rests on it.
//!
//! Loads are gathered from the top down: what a run bears it passes to its supports, what a
//! column bears to what is under it. A change anywhere has the connected structure reckoned
//! again, whole, so the outcome does not hang on the order of the changes.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use hearth_content::building::Member;
use hearth_content::schema::station::PieceShape;
use hearth_math::{BlockPos, Direction};

/// What a structure sees at a place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cell {
    /// Nothing that holds anything up: air, water, plants.
    Open,
    /// The natural ground, or land not loaded: it holds whatever rests on it.
    Ground,
    /// A piece: its member (an index into the members) and the way it faces.
    Piece(u32, Direction),
}

/// The most pieces reckoned together. A larger structure is reckoned about the change only,
/// what lies beyond holding as it did.
pub const MOST: usize = 8192;

/// What a reckoning found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reckoning {
    /// The pieces that fail, by place.
    pub failed: Vec<BlockPos>,
    /// How hard each piece is pressed: the largest share of what it can bear that it bears
    /// (over one, it fails), by place.
    pub stress: Vec<(BlockPos, f32)>,
}

/// The places a piece touches: its faces, and for a roof the next up and down its slope.
fn touching(p: BlockPos, cell: Cell, members: &[Member]) -> Vec<BlockPos> {
    let mut out: Vec<BlockPos> = Direction::ALL.iter().map(|d| p.offset(*d)).collect();
    if let Cell::Piece(m, f) = cell
        && members
            .get(m as usize)
            .is_some_and(|m| m.shape == PieceShape::Roof)
    {
        out.push(p.offset(f).up());
        out.push(p.offset(f.opposite()).down());
    }
    out
}

/// The pieces connected to any of `seeds` (or touching them, edges and corners too), at most
/// [`MOST`], sorted by place.
pub fn connected(
    look: &dyn Fn(BlockPos) -> Cell,
    members: &[Member],
    seeds: &[BlockPos],
) -> Vec<BlockPos> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::new();
    for s in seeds {
        for dy in -1..=1 {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let p = BlockPos::new(s.x + dx, s.y + dy, s.z + dz);
                    if matches!(look(p), Cell::Piece(..)) && seen.insert(p) {
                        queue.push_back(p);
                    }
                }
            }
        }
    }
    while let Some(p) = queue.pop_front() {
        for q in touching(p, look(p), members) {
            if seen.len() >= MOST {
                break;
            }
            if matches!(look(q), Cell::Piece(..)) && seen.insert(q) {
                queue.push_back(q);
            }
        }
    }
    seen.into_iter().collect()
}

/// Whether a piece of shape `under` bears one of shape `over` resting on it.
fn bears(under: PieceShape, over: PieceShape) -> bool {
    use PieceShape as S;
    match under {
        S::Post | S::Wall | S::Block | S::Beam | S::Layer => true,
        // Wattle and hides hold up a covering, not a frame.
        S::Panel => matches!(over, S::Roof | S::Layer | S::Panel),
        S::Roof => false,
    }
}

/// What holds a run up at a point.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Support {
    Ground,
    Piece(usize),
    /// Another roof leaning against this one at the ridge: it holds this one's top without
    /// taking its weight, which goes down the slope.
    Apex,
}

/// What a place is, to the reckoning.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Seen {
    Open,
    Ground,
    Piece(usize),
}

struct Piece {
    pos: BlockPos,
    m: Member,
    facing: Direction,
    /// The run it lies in, if it is not a column.
    run: Option<usize>,
    /// What rests on it (N).
    load: f32,
    stress: f32,
}

struct Run {
    blocks: Vec<usize>,
    /// Supports along it: where (a block's middle at its index, an end half a block beyond)
    /// and what.
    supports: Vec<(f32, Support)>,
    covering: bool,
    level: i32,
}

/// The axis a run lies along, as a step from one of its pieces to the next (a roof's step
/// also rises a block).
fn run_step(shape: PieceShape, facing: Direction) -> Direction {
    use PieceShape as S;
    let along = |d: Direction| match d {
        Direction::North | Direction::South => Direction::South,
        _ => Direction::East,
    };
    let across = |d: Direction| match d {
        Direction::North | Direction::South => Direction::East,
        _ => Direction::South,
    };
    match shape {
        S::Beam => along(facing),
        S::Roof => facing,
        S::Wall | S::Panel => across(facing),
        S::Layer | S::Block | S::Post => Direction::East,
    }
}

/// Reckons the structure of `pieces` (from [`connected`]): what each bears, and what fails.
/// `pressing` is the weight of natural ground (N) bearing down on a piece at a place from the
/// ground over it (a tunnel's roof on its timbers).
pub fn reckon(
    look: &dyn Fn(BlockPos) -> Cell,
    pressing: &dyn Fn(BlockPos) -> f32,
    members: &[Member],
    pieces: &[BlockPos],
) -> Reckoning {
    let mut ps: Vec<Piece> = Vec::with_capacity(pieces.len());
    let mut at: BTreeMap<BlockPos, usize> = BTreeMap::new();
    for p in pieces {
        if let Cell::Piece(m, facing) = look(*p)
            && let Some(m) = members.get(m as usize)
        {
            at.insert(*p, ps.len());
            let load = if look(p.up()) == Cell::Ground {
                pressing(*p)
            } else {
                0.0
            };
            ps.push(Piece {
                pos: *p,
                m: *m,
                facing,
                run: None,
                load,
                stress: 0.0,
            });
        }
    }
    let seen = |p: BlockPos| -> Seen {
        match at.get(&p) {
            Some(i) => Seen::Piece(*i),
            // A piece beyond the reckoning holds as it did.
            None => match look(p) {
                Cell::Open => Seen::Open,
                Cell::Ground | Cell::Piece(..) => Seen::Ground,
            },
        }
    };
    use PieceShape as S;
    // Columns stand on what bears them; the rest lie in runs.
    let column = |i: usize, ps: &[Piece]| -> bool {
        let p = &ps[i];
        match p.m.shape {
            S::Post => true,
            S::Wall | S::Block | S::Panel => match seen(p.pos.down()) {
                Seen::Ground => true,
                Seen::Piece(j) => bears(ps[j].m.shape, p.m.shape),
                Seen::Open => false,
            },
            S::Beam | S::Layer | S::Roof => false,
        }
    };
    let is_column: Vec<bool> = (0..ps.len()).map(|i| column(i, &ps)).collect();
    // A layer lies along the way it is held at both ends, or else at one, or else east.
    let layer_step = |i: usize, ps: &[Piece]| -> Direction {
        let mut best = (0, i32::MAX, Direction::East);
        for step in [Direction::East, Direction::South] {
            let same = |q: BlockPos| matches!(seen(q), Seen::Piece(j) if ps[j].m.shape == S::Layer);
            let (mut a, mut b) = (ps[i].pos, ps[i].pos);
            while same(a.offset(step.opposite())) {
                a = a.offset(step.opposite());
            }
            while same(b.offset(step)) {
                b = b.offset(step);
            }
            let holds = |q: BlockPos| match seen(q) {
                Seen::Ground => true,
                Seen::Piece(j) => bears(ps[j].m.shape, S::Layer),
                Seen::Open => false,
            };
            let ends = holds(a.offset(step.opposite())) as i32 + holds(b.offset(step)) as i32;
            let len = (b.x - a.x).abs() + (b.z - a.z).abs();
            if ends > best.0 || (ends == best.0 && len < best.1) {
                best = (ends, len, step);
            }
        }
        best.2
    };
    let steps: Vec<Direction> = (0..ps.len())
        .map(|i| {
            if ps[i].m.shape == S::Layer {
                layer_step(i, &ps)
            } else {
                run_step(ps[i].m.shape, ps[i].facing)
            }
        })
        .collect();
    // The next piece of a run from a piece, forward or back.
    let next = |i: usize, forward: bool, ps: &[Piece]| -> Option<usize> {
        let p = &ps[i];
        let step = steps[i];
        let q = match (p.m.shape, forward) {
            (S::Roof, true) => p.pos.offset(step).up(),
            (S::Roof, false) => p.pos.offset(step.opposite()).down(),
            (_, true) => p.pos.offset(step),
            (_, false) => p.pos.offset(step.opposite()),
        };
        match seen(q) {
            Seen::Piece(j)
                if !is_column[j]
                    && ps[j].m.shape == p.m.shape
                    && steps[j] == step
                    && (p.m.shape != S::Roof || ps[j].facing == p.facing) =>
            {
                Some(j)
            }
            _ => None,
        }
    };
    let mut runs: Vec<Run> = Vec::new();
    for i in 0..ps.len() {
        if is_column[i] || ps[i].run.is_some() {
            continue;
        }
        let mut first = i;
        let mut guard = 0;
        while let Some(j) = next(first, false, &ps) {
            first = j;
            guard += 1;
            if guard > MOST {
                break;
            }
        }
        let mut blocks = vec![first];
        while let Some(j) = next(*blocks.last().unwrap_or(&first), true, &ps) {
            if blocks.contains(&j) || blocks.len() > MOST {
                break;
            }
            blocks.push(j);
        }
        let r = runs.len();
        for b in &blocks {
            ps[*b].run = Some(r);
        }
        let shape = ps[first].m.shape;
        let step = steps[first];
        let holds = |q: BlockPos, ps: &[Piece]| -> Option<Support> {
            match seen(q) {
                Seen::Ground => Some(Support::Ground),
                Seen::Piece(j) if ps[j].run != Some(r) && bears(ps[j].m.shape, shape) => {
                    Some(Support::Piece(j))
                }
                Seen::Piece(j)
                    if shape == S::Roof
                        && ps[j].m.shape == S::Roof
                        && ps[j].facing == ps[first].facing.opposite() =>
                {
                    Some(Support::Apex)
                }
                _ => None,
            }
        };
        let mut supports = Vec::new();
        let n = blocks.len();
        let start = ps[blocks[0]].pos.offset(step.opposite());
        let end = ps[blocks[n - 1]].pos.offset(step);
        if let Some(s) = holds(start, &ps).filter(|s| *s != Support::Apex) {
            supports.push((-0.5, s));
        }
        for (k, b) in blocks.iter().enumerate() {
            if let Some(s) = holds(ps[*b].pos.down(), &ps).filter(|s| *s != Support::Apex) {
                supports.push((k as f32, s));
            }
        }
        if let Some(s) = holds(end, &ps) {
            supports.push((n as f32 - 0.5, s));
        }
        let level = blocks.iter().map(|b| ps[*b].pos.y).max().unwrap_or(0);
        runs.push(Run {
            blocks,
            supports,
            covering: matches!(shape, S::Roof | S::Layer | S::Panel),
            level,
        });
    }
    // From the top down: at each level the coverings, then the frame's runs (each before the
    // runs it rests on), then the columns.
    let mut items: Vec<(i32, u8, BlockPos, Item)> = Vec::new();
    for (r, run) in runs.iter().enumerate() {
        let class = if run.covering { 0 } else { 1 };
        items.push((-run.level, class, ps[run.blocks[0]].pos, Item::Run(r)));
    }
    for (i, p) in ps.iter().enumerate() {
        if is_column[i] {
            items.push((-p.pos.y, 2, p.pos, Item::Column(i)));
        }
    }
    items.sort();
    let items = order_runs(items, &runs, &ps);
    for item in items {
        match item {
            Item::Run(r) => bend(&runs[r], &mut ps),
            Item::Column(i) => press(i, &mut ps, &seen, &is_column),
        }
    }
    let mut out = Reckoning::default();
    for p in &ps {
        let s = if p.stress.is_finite() {
            p.stress
        } else {
            f32::MAX
        };
        out.stress.push((p.pos, s));
        if s > 1.0 {
            out.failed.push(p.pos);
        }
    }
    out
}

/// What is reckoned in turn: a run, or a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Item {
    Run(usize),
    Column(usize),
}

/// Orders the frame's runs at each level so that a run comes before those it rests on (Kahn's
/// order; in a ring of runs resting on each other, by place).
fn order_runs(items: Vec<(i32, u8, BlockPos, Item)>, runs: &[Run], ps: &[Piece]) -> Vec<Item> {
    let mut out = Vec::with_capacity(items.len());
    let mut i = 0;
    while i < items.len() {
        let (level, class) = (items[i].0, items[i].1);
        let mut j = i;
        while j < items.len() && items[j].0 == level && items[j].1 == class {
            j += 1;
        }
        let group: Vec<Item> = items[i..j].iter().map(|x| x.3).collect();
        i = j;
        if class != 1 || group.len() < 2 {
            out.extend(group);
            continue;
        }
        // Where each run of the group is in it, and which of them each rests on.
        let mut slot: BTreeMap<usize, usize> = BTreeMap::new();
        for (k, item) in group.iter().enumerate() {
            if let Item::Run(r) = item {
                slot.insert(*r, k);
            }
        }
        let mut after: Vec<Vec<usize>> = vec![Vec::new(); group.len()];
        let mut waiting = vec![0usize; group.len()];
        for (k, item) in group.iter().enumerate() {
            let Item::Run(r) = item else {
                continue;
            };
            let mut on: Vec<usize> = runs[*r]
                .supports
                .iter()
                .filter_map(|(_, s)| match s {
                    Support::Piece(p) => ps[*p].run.and_then(|q| slot.get(&q).copied()),
                    _ => None,
                })
                .filter(|q| *q != k)
                .collect();
            on.sort_unstable();
            on.dedup();
            for q in on {
                after[k].push(q);
                waiting[q] += 1;
            }
        }
        let mut ready: BTreeSet<usize> = (0..group.len()).filter(|k| waiting[*k] == 0).collect();
        let mut done = vec![false; group.len()];
        let mut left = group.len();
        while left > 0 {
            let k = match ready.pop_first() {
                Some(k) => k,
                // A ring: the first left, by place.
                None => match (0..group.len()).find(|k| !done[*k]) {
                    Some(k) => k,
                    None => break,
                },
            };
            if done[k] {
                continue;
            }
            done[k] = true;
            left -= 1;
            out.push(group[k]);
            for q in &after[k] {
                waiting[*q] = waiting[*q].saturating_sub(1);
                if waiting[*q] == 0 && !done[*q] {
                    ready.insert(*q);
                }
            }
        }
    }
    out
}

/// The run's bending: the moment at each piece's middle and each joint from its loads and
/// supports (each span between supports simply held, beyond the outermost a cantilever), its
/// reach, and its supports' shares of the load passed on. Linear in the run's length.
fn bend(run: &Run, ps: &mut [Piece]) {
    let n = run.blocks.len();
    let mut sup = run.supports.clone();
    sup.sort_by(|a, b| a.0.total_cmp(&b.0));
    if sup.is_empty() {
        for b in &run.blocks {
            ps[*b].stress = f32::INFINITY;
        }
        return;
    }
    let loads: Vec<f64> = run
        .blocks
        .iter()
        .map(|b| (ps[*b].m.weight + ps[*b].load) as f64)
        .collect();
    // The loads and their moments about the run's start, summed from it.
    let mut pre = vec![(0.0_f64, 0.0_f64); n + 1];
    for (k, w) in loads.iter().enumerate() {
        pre[k + 1] = (pre[k].0 + w, pre[k].1 + w * k as f64);
    }
    // The loads strictly between two places along the run: their sum and moment about its
    // start.
    let between = |a: f64, b: f64| -> (f64, f64) {
        let lo = (a.floor() as i64 + 1).max(0);
        let hi = (b.ceil() as i64 - 1).min(n as i64 - 1);
        if hi < lo {
            return (0.0, 0.0);
        }
        let (lo, hi) = (lo as usize, hi as usize);
        (pre[hi + 1].0 - pre[lo].0, pre[hi + 1].1 - pre[lo].1)
    };
    // Checkpoints every half block from the start's end (-0.5) to the far end (n - 0.5): a
    // piece's middle at a whole number, a joint between.
    let x_of = |j: usize| j as f64 * 0.5 - 0.5;
    let mut moments = vec![0.0_f64; 2 * n + 1];
    let mut reactions = vec![0.0_f64; sup.len()];
    let xs: Vec<f64> = sup.iter().map(|s| s.0 as f64).collect();
    let (first, last) = (xs[0], xs[xs.len() - 1]);
    // What rests right over a support goes into it.
    for (i, x) in xs.iter().enumerate() {
        if x.fract().abs() < 1e-6 && *x >= 0.0 && (*x as usize) < n {
            reactions[i] += loads[*x as usize];
        }
    }
    // Before the first support and beyond the last: cantilevers from it.
    reactions[0] += between(-1.0, first).0;
    reactions[xs.len() - 1] += between(last, n as f64).0;
    for (j, m) in moments.iter_mut().enumerate() {
        let c = x_of(j);
        if c <= first {
            let (w, wx) = between(-1.0, c);
            *m += c * w - wx;
        }
        if c >= last {
            let (w, wx) = between(c, n as f64);
            *m += wx - c * w;
        }
    }
    // Between supports: each span simply held at both ends.
    for i in 0..xs.len() - 1 {
        let (a, b) = (xs[i], xs[i + 1]);
        if b - a < 1e-6 {
            continue;
        }
        let (w, wx) = between(a, b);
        let ra = (b * w - wx) / (b - a);
        reactions[i] += ra;
        reactions[i + 1] += (wx - a * w) / (b - a);
        let j_lo = ((a + 0.5) * 2.0).floor() as usize + 1;
        let j_hi = (((b + 0.5) * 2.0).ceil() as usize).saturating_sub(1);
        let hi = j_hi.min(2 * n);
        if j_lo <= hi {
            for (j, m) in moments.iter_mut().enumerate().take(hi + 1).skip(j_lo) {
                let c = x_of(j);
                let (wc, wxc) = between(a, c);
                *m += ra * (c - a) - (c * wc - wxc);
            }
        }
    }
    // What each checkpoint bears.
    let mem: Vec<Member> = run.blocks.iter().map(|b| ps[*b].m).collect();
    let m = |k: usize| mem[k];
    let held_end = |x: f32| sup.iter().find(|s| (s.0 - x).abs() < 1e-3).map(|s| s.1);
    let end_share = |s: Support, k: usize, ps: &[Piece]| match s {
        Support::Ground => m(k).continuity,
        Support::Piece(j) => m(k).continuity.min(ps[j].m.continuity),
        Support::Apex => 0.0,
    };
    let start_share = held_end(-0.5).map_or(0.0, |s| end_share(s, 0, ps));
    let last_share = held_end(n as f32 - 0.5).map_or(0.0, |s| end_share(s, n - 1, ps));
    let (s_first, s_last) = (first as f32, last as f32);
    for (j, moment) in moments.iter().enumerate() {
        let (x, moment) = (x_of(j) as f32, moment.abs() as f32);
        if moment <= 1e-3 {
            continue;
        }
        let ratio = |cap: f32| {
            if cap > 0.0 {
                moment / cap
            } else {
                f32::INFINITY
            }
        };
        if j == 0 {
            // The joint with the support at the start: the first piece's.
            let s = ratio(m(0).moment * start_share);
            raise(ps, run.blocks[0], s);
        } else if j == 2 * n {
            let s = ratio(m(n - 1).moment * last_share);
            raise(ps, run.blocks[n - 1], s);
        } else if j % 2 == 1 {
            let k = (j - 1) / 2;
            raise(ps, run.blocks[k], ratio(m(k).moment));
        } else {
            let k = j / 2 - 1;
            let cap = m(k).moment.min(m(k + 1).moment) * m(k).continuity.min(m(k + 1).continuity);
            let s = ratio(cap);
            // Beyond the supports the outer piece breaks off; between them both give.
            if x < s_first {
                raise(ps, run.blocks[k], s);
            } else if x > s_last {
                raise(ps, run.blocks[k + 1], s);
            } else {
                raise(ps, run.blocks[k], s);
                raise(ps, run.blocks[k + 1], s);
            }
        }
    }
    // How far each piece reaches from what holds it: beyond the outermost supports from them,
    // between two the span they hold.
    let mut b = 0;
    for k in 0..n {
        let x = k as f32;
        let reach = m(k).reach.max(1e-3);
        let r = if x < s_first {
            (s_first - x + 0.5) / reach
        } else if x > s_last {
            (x - s_last + 0.5) / reach
        } else {
            while b + 1 < sup.len() && sup[b].0 < x {
                b += 1;
            }
            let a = b.saturating_sub(1);
            if (sup[b].0 - x).abs() < 1e-6 {
                0.0
            } else {
                (sup[b].0 - sup[a].0) / (2.0 * reach)
            }
        };
        raise(ps, run.blocks[k], r);
    }
    // The supports take their shares; the ridge's goes down the slope to the lowest support.
    let lowest = sup.iter().position(|s| s.1 != Support::Apex).unwrap_or(0);
    for (i, (_, s)) in sup.iter().enumerate() {
        let target = if *s == Support::Apex {
            sup[lowest].1
        } else {
            *s
        };
        if let Support::Piece(j) = target {
            ps[j].load += reactions[i] as f32;
        }
    }
}

/// A column's pressing: what it bears from above and its own weight, against crushing and (a
/// post) buckling over the height of the posts it stands in; passed down to what bears it.
fn press(i: usize, ps: &mut [Piece], seen: &dyn Fn(BlockPos) -> Seen, is_column: &[bool]) {
    let total = ps[i].m.weight + ps[i].load;
    let shape = ps[i].m.shape;
    let crush = if ps[i].m.crush > 0.0 {
        total / ps[i].m.crush
    } else {
        f32::INFINITY
    };
    raise(ps, i, crush);
    if shape == PieceShape::Post {
        // The posts above and below it, standing as one.
        let is_post = |p: BlockPos| matches!(seen(p), Seen::Piece(j) if is_column[j] && ps[j].m.shape == PieceShape::Post);
        let mut h = 1.0_f32;
        let mut p = ps[i].pos;
        while is_post(p.up()) && h < 64.0 {
            p = p.up();
            h += 1.0;
        }
        let mut p = ps[i].pos;
        while is_post(p.down()) && h < 64.0 {
            p = p.down();
            h += 1.0;
        }
        let critical = std::f32::consts::PI.powi(2) * ps[i].m.stiffness / (h * h);
        let buckle = if critical > 0.0 {
            total / critical
        } else {
            f32::INFINITY
        };
        raise(ps, i, buckle);
    }
    match seen(ps[i].pos.down()) {
        Seen::Ground => {}
        Seen::Piece(j) if bears(ps[j].m.shape, shape) => ps[j].load += total,
        // Nothing under it: it falls.
        _ => raise(ps, i, f32::INFINITY),
    }
}

fn raise(ps: &mut [Piece], i: usize, s: f32) {
    if s > ps[i].stress || s.is_nan() {
        ps[i].stress = if s.is_nan() { f32::INFINITY } else { s };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POST: Member = Member {
        shape: PieceShape::Post,
        weight: 7.8,
        moment: 267.0,
        continuity: 0.4,
        crush: 22_000.0,
        stiffness: 565.0,
        reach: 2.0,
    };
    const BEAM: Member = Member {
        shape: PieceShape::Beam,
        ..POST
    };
    /// A granite slab 0.3 m wide and 0.2 deep, laid without mortar.
    const LINTEL: Member = Member {
        shape: PieceShape::Beam,
        weight: 1590.0,
        moment: 20_800.0,
        continuity: 0.0,
        crush: 1e9,
        stiffness: 1e7,
        reach: 10.0,
    };
    /// A course of dry stone half a metre thick: it bends not at all.
    const WALL: Member = Member {
        shape: PieceShape::Wall,
        weight: 9930.0,
        moment: 0.0,
        continuity: 0.0,
        crush: 1e10,
        stiffness: 1e9,
        reach: 50.0,
    };
    const ROOF: Member = Member {
        shape: PieceShape::Roof,
        weight: 65.0,
        moment: 345.0,
        continuity: 0.4,
        crush: 1e4,
        stiffness: 100.0,
        reach: 2.0,
    };

    /// A little world: the ground below y = 0, and pieces.
    struct World {
        members: Vec<Member>,
        pieces: BTreeMap<BlockPos, (u32, Direction)>,
    }

    impl World {
        fn new() -> Self {
            World {
                members: vec![POST, BEAM, LINTEL, WALL, ROOF],
                pieces: BTreeMap::new(),
            }
        }
        fn put(&mut self, m: u32, x: i32, y: i32, z: i32, f: Direction) {
            self.pieces.insert(BlockPos::new(x, y, z), (m, f));
        }
        fn look(&self, p: BlockPos) -> Cell {
            match self.pieces.get(&p) {
                Some((m, f)) => Cell::Piece(*m, *f),
                None if p.y < 0 => Cell::Ground,
                None => Cell::Open,
            }
        }
        fn reckon(&self) -> Reckoning {
            let look = |p| self.look(p);
            let seeds: Vec<BlockPos> = self.pieces.keys().copied().collect();
            let all = connected(&look, &self.members, &seeds);
            reckon(&look, &|_| 0.0, &self.members, &all)
        }
    }

    /// Two posts two high, `gap` beams between them, and a roof over it all.
    fn lean_to(gap: i32) -> World {
        let mut w = World::new();
        for x in [0, gap + 1] {
            w.put(0, x, 0, 0, Direction::North);
            w.put(0, x, 1, 0, Direction::North);
        }
        for x in 1..=gap {
            w.put(1, x, 1, 0, Direction::East);
        }
        for x in 0..=gap + 1 {
            w.put(4, x, 1, 1, Direction::North);
            w.put(4, x, 0, 2, Direction::North);
        }
        w
    }

    #[test]
    fn a_lean_to_stands() {
        let r = lean_to(3).reckon();
        assert!(r.failed.is_empty(), "{:?}", r.failed);
        let worst = r.stress.iter().map(|s| s.1).fold(0.0, f32::max);
        assert!(worst > 0.05 && worst < 1.0, "{worst}");
    }

    #[test]
    fn a_ridge_too_long_for_its_poles_breaks() {
        let r = lean_to(9).reckon();
        assert!(!r.failed.is_empty());
        // The beams give, not the posts.
        assert!(
            r.failed.iter().all(|p| p.y == 1 && p.z <= 1),
            "{:?}",
            r.failed
        );
    }

    #[test]
    fn a_post_with_nothing_under_it_falls() {
        let mut w = World::new();
        w.put(0, 0, 2, 0, Direction::North);
        assert_eq!(w.reckon().failed, vec![BlockPos::new(0, 2, 0)]);
    }

    #[test]
    fn one_slab_spans_a_doorway_two_do_not() {
        // Walls either side of a gap, a lintel course over it, wall over the lintels.
        let doorway = |gap: i32| {
            let mut w = World::new();
            for y in 0..3 {
                w.put(3, 0, y, 0, Direction::North);
                w.put(3, gap + 1, y, 0, Direction::North);
            }
            for x in 1..=gap {
                w.put(2, x, 2, 0, Direction::East);
                w.put(3, x, 3, 0, Direction::North);
            }
            w.reckon()
        };
        let one = doorway(1);
        assert!(one.failed.is_empty(), "{:?}", one.failed);
        let two = doorway(2);
        assert!(
            two.failed.contains(&BlockPos::new(1, 2, 0)),
            "{:?}",
            two.failed
        );
    }

    #[test]
    fn dry_stone_over_nothing_falls() {
        let mut w = World::new();
        w.put(3, 0, 0, 0, Direction::North);
        w.put(3, 0, 1, 0, Direction::North);
        w.put(3, 1, 1, 0, Direction::North);
        let r = w.reckon();
        assert_eq!(r.failed, vec![BlockPos::new(1, 1, 0)]);
    }

    #[test]
    fn a_span_bends_as_beam_theory_says() {
        // Three beams of 100 N between end supports 3 m apart: the middle bears 1.5 × 100 × 1.5
        // less 100 × 1 = 125 N·m, half of its 250.
        let mut w = World::new();
        w.members.push(Member {
            weight: 100.0,
            moment: 250.0,
            continuity: 1.0,
            reach: 100.0,
            ..BEAM
        });
        for y in 0..2 {
            w.put(0, 0, y, 0, Direction::North);
            w.put(0, 4, y, 0, Direction::North);
        }
        for x in 1..=3 {
            w.put(5, x, 1, 0, Direction::East);
        }
        let r = w.reckon();
        let mid = r
            .stress
            .iter()
            .find(|(p, _)| *p == BlockPos::new(2, 1, 0))
            .map(|s| s.1);
        assert!(mid.is_some_and(|s| (s - 0.5).abs() < 1e-3), "{mid:?}");
    }

    #[test]
    fn a_great_wall_is_reckoned_quickly() {
        // Dry stone two hundred long and twenty high, over a row of lintels on posts.
        let mut w = World::new();
        for x in 0..200 {
            if x % 2 == 0 {
                w.put(3, x, 0, 0, Direction::North);
            }
            w.put(2, x, 1, 0, Direction::East);
            for y in 2..22 {
                w.put(3, x, y, 0, Direction::North);
            }
        }
        let t = std::time::Instant::now();
        let r = w.reckon();
        let took = t.elapsed();
        println!("{} pieces in {took:?}", r.stress.len());
        assert_eq!(r.stress.len(), 200 * 21 + 100);
        assert!(took.as_millis() < 250, "{took:?}");
    }

    #[test]
    fn the_same_whatever_the_order() {
        let a = lean_to(4);
        let mut b = World::new();
        for (p, (m, f)) in a.pieces.iter().rev() {
            b.put(*m, p.x, p.y, p.z, *f);
        }
        assert_eq!(a.reckon(), b.reckon());
    }
}
