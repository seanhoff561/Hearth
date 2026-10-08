//! A moving body: its box, stance, velocity and breath, and one step of movement.

use glam::{DVec2, DVec3};
use hearth_math::{Aabb, Axis, BlockPos};
use serde::{Deserialize, Serialize};

use crate::{GRAVITY, Ground, Terrain};

/// Width of a person's box (m).
pub const WIDTH: f64 = 0.5;
/// A step taken without breaking stride, and the highest scrambled while walking (m).
pub const STEP_FREE: f64 = 0.6;
pub const STEP_SCRAMBLE: f64 = 1.05;
/// Highest ledge a person can pull up onto, from the feet (m).
pub const REACH: f64 = 1.9;
/// A boat floats once the water is this deep where it is (m), and the paddler sits this far under
/// the surface.
const BOAT_FLOATS_M: f64 = 0.35;
const BOAT_SEAT_M: f64 = 0.1;
/// Speeds of crouching and crawling (m/s).
const CROUCH_M_S: f64 = 0.8;
const CRAWL_M_S: f64 = 0.4;
/// Air drag: a falling person's terminal speed is about 55 m/s.
const AIR_DRAG: f64 = GRAVITY / (55.0 * 55.0);
/// Quadratic drag of water on a body moving through it (per m).
const WATER_DRAG: f64 = 0.7;
/// Share of a fall's speed that entering water passes on as an impact.
const WATER_ENTRY: f64 = 0.35;
/// Longest movement substep (s).
const MAX_DT: f64 = 1.0 / 60.0;

/// How the body is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Stance {
    #[default]
    Standing,
    Crouching,
    Crawling,
    Swimming,
    Climbing,
}

impl Stance {
    /// Height of the body's box (m).
    pub fn height(self) -> f64 {
        match self {
            Stance::Standing | Stance::Climbing => 1.75,
            Stance::Crouching => 1.3,
            Stance::Crawling | Stance::Swimming => 0.6,
        }
    }

    /// Height of the eyes above the box's bottom (m).
    pub fn eye(self) -> f64 {
        match self {
            Stance::Standing | Stance::Climbing => 1.62,
            Stance::Crouching => 1.15,
            Stance::Crawling => 0.45,
            Stance::Swimming => 0.5,
        }
    }
}

/// Pace on foot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Gait {
    #[default]
    Walk,
    Jog,
    Sprint,
}

/// What the player (or an animal's mind) wants this step.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Intent {
    /// Direction to go, world x and z, up to unit length (slower when shorter).
    pub wish: DVec2,
    pub gait: Gait,
    /// Jump; with a wall ahead, climb onto it; in water, swim up.
    pub jump: bool,
    pub crouch: bool,
    pub crawl: bool,
    /// Swim down, climb down.
    pub descend: bool,
}

/// What the body allows (from its effects, stamina and load).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ability {
    pub walk_m_s: f64,
    pub jog_m_s: f64,
    pub sprint_m_s: f64,
    pub swim_m_s: f64,
    /// Sprinting allowed now.
    pub sprint: bool,
    /// Height of a standing jump (m); 0 when jumping is not possible.
    pub jump_m: f64,
    /// Strength and two hands to pull up onto a ledge.
    pub climb: bool,
    /// How long the breath can be held (s).
    pub breath_s: f64,
    /// 0–1: an exhausted swimmer slips under.
    pub stamina: f64,
    /// Paddling speed afloat in a boat (V2-12: a dugout dragged into the water); 0 without one.
    pub boat_m_s: f64,
}

impl Ability {
    /// A healthy adult.
    pub fn human() -> Self {
        Self {
            walk_m_s: 1.4,
            jog_m_s: 3.0,
            sprint_m_s: 6.5,
            swim_m_s: 0.8,
            sprint: true,
            jump_m: 0.45,
            climb: true,
            breath_s: 45.0,
            stamina: 1.0,
            boat_m_s: 0.0,
        }
    }
}

/// What the body did, for its metabolism and animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Motion {
    #[default]
    Still,
    Walking,
    Jogging,
    Sprinting,
    Crouching,
    Crawling,
    Wading,
    Swimming,
    /// Afloat in a boat, paddling (V2-12).
    Paddling,
    Climbing,
    Falling,
}

/// The outcome of a step.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Report {
    pub motion: Motion,
    /// Horizontal speed (m/s).
    pub speed: f64,
    /// A landing this step: the impact speed after the ground's or the water's cushioning (m/s).
    pub landed: Option<f64>,
    /// Share of the body under water.
    pub immersion: f64,
    pub eyes_under: bool,
    /// Seconds without air.
    pub airless_s: f64,
    /// Scrambling up a step or climbing: hard work for the moment.
    pub straining: bool,
    /// The rise over the run of the step on the ground (positive uphill), for the effort.
    pub grade: f64,
    /// Sliding down ground too steep (or slippery) to stand on.
    pub sliding: bool,
}

/// A climb in progress: from the current height up to `to`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Climb {
    pub to: DVec3,
    pub from_y: f64,
    pub left_s: f64,
    pub total_s: f64,
}

/// A moving body.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Mover {
    /// The middle of the feet.
    pub pos: DVec3,
    pub vel: DVec3,
    pub on_ground: bool,
    pub stance: Stance,
    /// Breath left to hold (s) and time without air (s).
    pub breath_s: f64,
    pub airless_s: f64,
    pub climb: Option<Climb>,
    /// Time left scrambling up a step (s).
    pub scramble_s: f64,
    /// Fastest downward speed since leaving the ground (m/s).
    pub fall_speed: f64,
    /// Whether the body was in water on the last step.
    pub wet: bool,
    /// How tall the body stands to a grown one of its kind (a child's less than 1): its box and
    /// eyes are as much lower.
    #[serde(default = "grown")]
    pub scale: f64,
    /// The natural ground's normal under the feet when last on it (zero: none, or level).
    #[serde(default)]
    pub ground_n: DVec3,
}

fn grown() -> f64 {
    1.0
}

impl Mover {
    pub fn new(pos: DVec3) -> Self {
        Self {
            pos,
            vel: DVec3::ZERO,
            on_ground: false,
            stance: Stance::Standing,
            breath_s: 45.0,
            airless_s: 0.0,
            climb: None,
            scramble_s: 0.0,
            fall_speed: 0.0,
            wet: false,
            scale: 1.0,
            ground_n: DVec3::ZERO,
        }
    }

    /// The height of its box as it stands now (m).
    pub fn height(&self) -> f64 {
        self.stance.height() * self.scale
    }

    /// The height of its eyes above its feet as it stands now (m).
    pub fn eye_height(&self) -> f64 {
        self.stance.eye() * self.scale
    }

    /// The body's box.
    pub fn bounds(&self) -> Aabb {
        Aabb::from_feet(self.pos, WIDTH * self.scale.max(0.6), self.height())
    }

    /// Where the eyes are.
    pub fn eye(&self) -> DVec3 {
        self.pos + DVec3::new(0.0, self.eye_height(), 0.0)
    }
}

/// Advances a body by `dt` seconds of play.
pub fn step(t: &impl Terrain, m: &mut Mover, i: &Intent, a: &Ability, dt: f64) -> Report {
    let n = (dt / MAX_DT).ceil().max(1.0) as usize;
    let h = dt / n as f64;
    let mut out = Report::default();
    for _ in 0..n {
        let r = substep(t, m, i, a, h);
        out.landed = match (out.landed, r.landed) {
            (Some(x), Some(y)) => Some(x.max(y)),
            (x, y) => x.or(y),
        };
        out.straining |= r.straining;
        out.sliding |= r.sliding;
        out.grade = r.grade;
        out.motion = r.motion;
        out.speed = r.speed;
        out.immersion = r.immersion;
        out.eyes_under = r.eyes_under;
        out.airless_s = r.airless_s;
    }
    out
}

/// Creative's flight (Amendment P §3.1): the body moves at a velocity (m/s) without gravity,
/// stopped by what is solid — or, passing `through`, through it (no-clip). Returns the movement
/// made.
pub fn fly(t: &impl Terrain, m: &mut Mover, v: DVec3, dt: f64, through: bool) -> DVec3 {
    let d = v * dt;
    let moved = if through {
        d
    } else {
        // In pieces no longer than half a block, so nothing thin is passed.
        let n = (d.length() / 0.5).ceil().max(1.0) as usize;
        let mut scratch = Vec::new();
        let mut moved = DVec3::ZERO;
        for _ in 0..n {
            let b = m.bounds();
            let step = sweep(t, &b, d / n as f64, &mut scratch);
            m.pos += step;
            moved += step;
        }
        m.pos -= moved;
        moved
    };
    m.pos += moved;
    m.pos.x = t.wrap_x(m.pos.x);
    m.vel = if dt > 0.0 { moved / dt } else { DVec3::ZERO };
    m.on_ground = false;
    moved
}

/// The water's surface over the body's column, if water reaches the feet.
fn water_level(t: &impl Terrain, pos: DVec3) -> Option<f64> {
    let x = pos.x.floor() as i32;
    let z = pos.z.floor() as i32;
    let feet = pos.y.floor() as i32;
    // Up from the feet's block while the water is full (the body is at most two blocks tall).
    let mut level = None;
    for y in feet..feet + 4 {
        let fill = t.water(BlockPos::new(x, y, z));
        if fill <= 0.0 {
            break;
        }
        level = Some(y as f64 + fill);
        if fill < 1.0 {
            break;
        }
    }
    level.filter(|&l| l > pos.y)
}

/// Whether a box overlaps what is solid: the boxes, or the natural ground's field from
/// `clearance` above its bottom up (a body's room is checked clear of the slope its feet stand
/// on; probes for ground use none).
fn collides_above(t: &impl Terrain, b: &Aabb, clearance: f64, scratch: &mut Vec<Aabb>) -> bool {
    boxes_hit(t, b, scratch) || field_hit(t, b, clearance)
}

/// Whether a box overlaps the blocks' boxes (not the natural ground).
fn boxes_hit(t: &impl Terrain, b: &Aabb, scratch: &mut Vec<Aabb>) -> bool {
    scratch.clear();
    t.boxes(&b.inflate(DVec3::splat(-1e-4)), scratch);
    scratch
        .iter()
        .any(|o| o.intersects(&b.inflate(DVec3::splat(-1e-4))))
}

fn collides(t: &impl Terrain, b: &Aabb, scratch: &mut Vec<Aabb>) -> bool {
    collides_above(t, b, FIELD_CLEARANCE, scratch)
}

/// Above the feet, the field is checked for a body's room from this high (m): a slope as steep
/// as can be walked rises this much across half a body's width.
const FIELD_CLEARANCE: f64 = 0.3;

/// Whether the natural ground reaches into a box (sampled through its middle and round its
/// edge, every 0.35 m up from `clearance` above its bottom).
fn field_hit(t: &impl Terrain, b: &Aabb, clearance: f64) -> bool {
    let c = (b.min + b.max) * 0.5;
    if t.depth(c).is_none() {
        return false;
    }
    let rx = (b.max.x - b.min.x) * 0.5 * 0.95;
    let rz = (b.max.z - b.min.z) * 0.5 * 0.95;
    let lo = b.min.y + clearance.min(b.max.y - b.min.y);
    let n = (((b.max.y - lo) / 0.35).ceil() as usize).max(1);
    for k in 0..=n {
        let y = lo + (b.max.y - lo) * k as f64 / n as f64;
        for (dx, dz) in RING {
            let q = DVec3::new(c.x + dx * rx, y, c.z + dz * rz);
            if t.depth(q).is_some_and(|d| d > 0.0) {
                return true;
            }
        }
    }
    false
}

/// The middle and eight points round a circle (unit radius).
const D: f64 = std::f64::consts::FRAC_1_SQRT_2;
const RING: [(f64, f64); 9] = [
    (0.0, 0.0),
    (1.0, 0.0),
    (D, D),
    (0.0, 1.0),
    (-D, D),
    (-1.0, 0.0),
    (-D, -D),
    (0.0, -1.0),
    (D, -D),
];

/// The highest surface of the natural ground in a column between `bottom` and `top`: down from
/// the top in fifths of a metre to the first point inside, then halved to a millimetre. The top
/// itself inside: the top (the column is buried there).
fn ground_at(t: &impl Terrain, x: f64, z: f64, top: f64, bottom: f64) -> Option<f64> {
    let at = |y: f64| t.depth(DVec3::new(x, y, z)).unwrap_or(-1.0);
    if at(top) > 0.0 {
        return Some(top);
    }
    let mut hi = top;
    let mut y = top;
    while y > bottom {
        let lo = (y - 0.2).max(bottom);
        if at(lo) > 0.0 {
            let (mut a, mut b) = (lo, hi);
            for _ in 0..8 {
                let m = 0.5 * (a + b);
                if at(m) > 0.0 {
                    a = m;
                } else {
                    b = m;
                }
            }
            return Some(b);
        }
        hi = lo;
        y = lo;
    }
    None
}

/// The field's outward normal at a point (its gradient, reversed).
fn field_normal(t: &impl Terrain, p: DVec3) -> DVec3 {
    let h = 0.05;
    let d = |o: DVec3| t.depth(p + o).unwrap_or(0.0) - t.depth(p - o).unwrap_or(0.0);
    let g = DVec3::new(d(DVec3::X * h), d(DVec3::Y * h), d(DVec3::Z * h));
    (-g).try_normalize().unwrap_or(DVec3::Y)
}

/// The ground a body of radius `r` at `p` stands on: the highest of the field's surfaces under
/// its middle and four points half its radius out, between `top` and `bottom`, and the normal
/// there.
fn support(t: &impl Terrain, p: DVec3, r: f64, top: f64, bottom: f64) -> Option<(f64, DVec3)> {
    let mut best: Option<(f64, DVec3)> = None;
    for (dx, dz) in [(0.0, 0.0), (0.5, 0.0), (-0.5, 0.0), (0.0, 0.5), (0.0, -0.5)] {
        let (x, z) = (p.x + dx * r, p.z + dz * r);
        if let Some(y) = ground_at(t, x, z, top, bottom)
            && best.is_none_or(|b| y > b.0)
        {
            best = Some((y, DVec3::new(x, y, z)));
        }
    }
    best.map(|(y, at)| (y, field_normal(t, at + DVec3::Y * 0.02)))
}

/// The steepest natural ground a body stands and walks up on with this footing (its friction,
/// 0.6 normal to 0.98 on ice): the slope whose tangent is the soles' grip (some 42° on dry
/// ground, a few degrees on ice).
fn max_slope_tan(ground: &Ground) -> f64 {
    (2.25 * (1.0 - ground.friction)).clamp(0.03, 1.0)
}

/// Whether ground of this normal is too steep (or slippery) to stand on with this footing.
fn too_steep(n: DVec3, ground: &Ground) -> bool {
    if n.y <= 0.0 {
        return false;
    }
    let tan_max = max_slope_tan(ground);
    let sin = (1.0 - n.y * n.y).max(0.0).sqrt();
    sin > tan_max * n.y
}

/// The surface under the feet: the block just under them, or a little deeper where the smooth
/// ground's surface lies within an empty voxel's height.
fn ground_under(t: &impl Terrain, pos: DVec3) -> Ground {
    for down in [0.05, 0.4, 0.8] {
        let p = BlockPos::containing(pos - DVec3::Y * down);
        if t.depth(p.as_dvec3() + DVec3::splat(0.5))
            .is_none_or(|d| d > -1.0)
        {
            return t.ground(p);
        }
    }
    t.ground(BlockPos::containing(pos - DVec3::Y * 0.05))
}

/// Moves a box by `d` through the boxes around it, one axis at a time (Y, X, Z). Returns the
/// movement made.
fn sweep(t: &impl Terrain, b: &Aabb, d: DVec3, scratch: &mut Vec<Aabb>) -> DVec3 {
    scratch.clear();
    let area = b.expand_towards(d).inflate(DVec3::splat(0.01));
    t.boxes(&area, scratch);
    let mut b = *b;
    let mut out = DVec3::ZERO;
    for axis in [Axis::Y, Axis::X, Axis::Z] {
        let k = axis.index();
        let mut off = d[k];
        if off == 0.0 {
            continue;
        }
        for o in scratch.iter() {
            off = b.clip_axis(o, axis, off);
        }
        let mut v = DVec3::ZERO;
        v[k] = off;
        b = b.offset(v);
        out[k] = off;
    }
    out
}

/// The ledge ahead a climber can pull up onto: the lowest free spot within reach above the
/// obstacle in front, with room to rise there.
fn find_ledge(t: &impl Terrain, m: &Mover, fwd: DVec2, s: &mut Vec<Aabb>) -> Option<DVec3> {
    // A child reaches and rises as much less as it is smaller.
    let width = WIDTH * m.scale.max(0.6);
    let ahead = DVec3::new(fwd.x, 0.0, fwd.y) * (width * 0.5 + 0.3);
    let h = Stance::Standing.height() * m.scale;
    // Something must be in the way at the feet.
    if !collides_above(
        t,
        &Aabb::from_feet(m.pos + ahead + DVec3::Y * 0.05, width, 0.5),
        0.0,
        s,
    ) {
        return None;
    }
    let mut dy = 0.125;
    while dy <= REACH * m.scale + 1e-9 {
        let top = m.pos + ahead + DVec3::Y * dy;
        // The feet on the ledge's top (the natural ground's surface at most a couple of
        // centimetres into them), room to rise there (the body's middle half) and ground under.
        let spot = Aabb::from_feet(top, width, h);
        let rise = Aabb::from_feet(m.pos + DVec3::Y * dy, width * 0.5, h);
        let support = Aabb::from_feet(top - DVec3::Y * 0.1, width * 0.6, 0.1);
        if !collides_above(t, &spot, 0.02, s)
            && !collides_above(t, &rise, 0.02, s)
            && collides_above(t, &support, 0.0, s)
        {
            return Some(top);
        }
        dy += 0.0625;
    }
    None
}

/// What a step on the natural ground came to.
#[derive(Debug, Clone, Copy, Default)]
struct FieldContact {
    on_ground: bool,
    /// Came down onto it this step.
    landed: bool,
    /// Stopped by ground too high or too steep to go up.
    blocked: bool,
    normal: DVec3,
    /// How far the feet rose onto it.
    rise: f64,
}

/// Moves a body's step `moved` (from the boxes' sweep) against the natural ground's field: out
/// of ground standing up into its body (pushed back along the surface), its feet set on the
/// surface where it rises no more than `max_rise` and not too steep to walk up, and (with
/// `stick`, on the ground and not leaving it) kept on it going down a slope.
fn field_move(
    t: &impl Terrain,
    m: &Mover,
    moved: &mut DVec3,
    d: DVec3,
    stick: bool,
    max_rise: f64,
    ground: &Ground,
) -> FieldContact {
    let r = WIDTH * m.scale.max(0.6) * 0.5;
    let h = m.height();
    let start = m.pos;
    let mut p = start + *moved;
    let flat = |v: DVec3| (v.x * v.x + v.z * v.z).sqrt();
    let mut out = FieldContact::default();
    // Walls: ground in the body above what can be stepped onto.
    let lo = max_rise + 0.1;
    if lo < h - 0.1 {
        for _ in 0..4 {
            let mut deepest: Option<(DVec3, f64)> = None;
            let n = (((h - 0.1 - lo) / 0.35).ceil() as usize).max(1);
            for k in 0..=n {
                let y = p.y + lo + (h - 0.1 - lo) * k as f64 / n as f64;
                for (dx, dz) in &RING[1..] {
                    let q = DVec3::new(p.x + dx * r, y, p.z + dz * r);
                    if let Some(dp) = t.depth(q)
                        && dp > 0.0
                        && deepest.is_none_or(|(_, b)| dp > b)
                    {
                        deepest = Some((q, dp));
                    }
                }
            }
            let Some((q, dp)) = deepest else {
                break;
            };
            let nrm = field_normal(t, q);
            let away = DVec3::new(p.x - q.x, 0.0, p.z - q.z).normalize_or_zero();
            let push = DVec3::new(nrm.x, 0.0, nrm.z)
                .try_normalize()
                .unwrap_or(away);
            p += push * (dp + 0.005);
        }
        // Pushed back further than it went: it does not go there.
        if flat(p - (start + *moved)) > flat(*moved) + 0.05 {
            p.x = start.x;
            p.z = start.z;
            out.blocked = true;
        }
    }
    // The ground under the feet.
    let snap = if stick {
        0.25 + flat(*moved) * 1.5
    } else {
        0.0
    };
    let top = p.y.max(start.y) + max_rise;
    let bottom = p.y - snap - 0.01;
    if let Some((gy, nrm)) = support(t, p, r, top, bottom) {
        let rise = gy - start.y;
        // Too steep to walk up: judged a little beyond the feet in the way they go, so a bank's
        // edge (its top level) is stepped onto while a steep slope (steep there too) is not.
        let going = flat(*moved) > 1e-6;
        let way = DVec3::new(moved.x, 0.0, moved.z).normalize_or_zero();
        let steep = too_steep(nrm, ground)
            && [0.15, 0.3, 0.45].iter().all(|k| {
                let q = p + way * *k;
                ground_at(t, q.x, q.z, top, gy - 0.3)
                    .map(|y| field_normal(t, DVec3::new(q.x, y + 0.02, q.z)))
                    .is_none_or(|n| too_steep(n, ground))
            });
        if gy >= p.y - 1e-6 || stick {
            if rise > max_rise + 1e-6 || (steep && going && rise > 0.01 && stick) {
                // Too high, or too steep to walk up: it stays where it was across.
                p.x = start.x;
                p.z = start.z;
                out.blocked = true;
                if let Some((g2, n2)) = support(t, p, r, start.y + 0.05, start.y - snap - 0.01)
                    && (g2 >= p.y - 1e-6 || stick)
                {
                    out.landed = d.y < 0.0 && g2 >= p.y - 1e-6;
                    p.y = g2;
                    out.on_ground = true;
                    out.normal = n2;
                }
            } else {
                out.landed = d.y < 0.0 && gy >= p.y - 1e-6;
                p.y = gy;
                out.on_ground = true;
                out.normal = nrm;
                out.rise = rise.max(0.0);
            }
        }
    }
    *moved = p - start;
    out
}

fn substep(t: &impl Terrain, m: &mut Mover, i: &Intent, a: &Ability, dt: f64) -> Report {
    let mut s = Vec::with_capacity(32);
    let mut rep = Report::default();

    // A climb in progress: rise, then step onto the ledge.
    if let Some(mut c) = m.climb {
        c.left_s -= dt;
        let k = (1.0 - c.left_s / c.total_s).clamp(0.0, 1.0);
        m.pos.y = c.from_y + (c.to.y - c.from_y) * k;
        m.vel = DVec3::ZERO;
        if c.left_s <= 0.0 {
            m.pos = c.to;
            m.pos.x = t.wrap_x(m.pos.x);
            m.climb = None;
            m.stance = Stance::Standing;
            m.on_ground = true;
        } else {
            m.climb = Some(c);
        }
        rep.motion = Motion::Climbing;
        rep.straining = true;
        rep.airless_s = m.airless_s;
        return rep;
    }

    // Water around the body.
    let level = water_level(t, m.pos);
    let depth = level.map_or(0.0, |l| l - m.pos.y);
    let submerged = (depth / (Stance::Standing.height() * m.scale)).clamp(0.0, 1.0);
    let ground = if m.on_ground {
        ground_under(t, m.pos)
    } else {
        Ground::default()
    };

    // Afloat in a boat (V2-12): sitting in it once the water is deep enough to float it (knee
    // deep), until it grounds in the shallows.
    let boating = a.boat_m_s > 0.0 && level.is_some() && !(m.on_ground && depth < BOAT_FLOATS_M);
    // Stance: swim when the water is over the chest and the feet find no bottom (and keep
    // swimming until they stand in shallower water); otherwise as asked, if there is room.
    let swimming = if boating {
        false
    } else if m.stance == Stance::Swimming {
        level.is_some() && !(m.on_ground && submerged < 0.6)
    } else {
        (submerged > 0.72 && !m.on_ground) || submerged > 0.85
    };
    let wanted = if swimming {
        Stance::Swimming
    } else if i.crawl {
        Stance::Crawling
    } else if i.crouch {
        Stance::Crouching
    } else {
        Stance::Standing
    };
    if wanted != m.stance {
        let taller = wanted.height() > m.stance.height();
        let width = WIDTH * m.scale.max(0.6);
        let fits = !taller
            || !collides(
                t,
                &Aabb::from_feet(m.pos, width, wanted.height() * m.scale),
                &mut s,
            );
        if fits {
            m.stance = wanted;
        } else if wanted == Stance::Standing
            && !collides(
                t,
                &Aabb::from_feet(m.pos, width, Stance::Crouching.height() * m.scale),
                &mut s,
            )
        {
            m.stance = Stance::Crouching;
        }
    }

    // Horizontal: toward the wished velocity, quickly on firm ground, slowly in the air.
    let wish = if i.wish.length_squared() > 1.0 {
        i.wish.normalize()
    } else {
        i.wish
    };
    let mut speed = match m.stance {
        _ if boating => a.boat_m_s,
        Stance::Swimming => a.swim_m_s,
        Stance::Crawling => CRAWL_M_S,
        Stance::Crouching => CROUCH_M_S.min(a.walk_m_s),
        _ => match i.gait {
            Gait::Sprint if a.sprint => a.sprint_m_s,
            Gait::Sprint | Gait::Jog => a.jog_m_s,
            Gait::Walk => a.walk_m_s,
        },
    };
    if m.on_ground {
        speed *= ground.speed;
        if m.stance != Stance::Swimming {
            speed *= 1.0 - 0.75 * submerged.min(0.8);
        }
        // Up and down slopes (S §8.2): Tobler's hiking function, fastest a little downhill,
        // slower up and down steep ground.
        if m.ground_n.y > 0.1
            && let Some(w) = i.wish.try_normalize()
        {
            let grade = -(m.ground_n.x * w.x + m.ground_n.z * w.y) / m.ground_n.y;
            let tobler = |g: f64| (-3.5 * (g + 0.05).abs()).exp();
            speed *= (tobler(grade) / tobler(0.0)).clamp(0.2, 1.2);
        }
    }
    // Pushing through plants and foliage (Amendment P §10.1): each column of them the body's
    // box overlaps slows it by their density times the square of how high up the body they
    // reach, as a share of its height — grass at the ankles hardly at all, a waist-high stand
    // a little, a shrub the body pushes through with its whole height much — weighted by the
    // share of the box's footprint the column takes.
    let thicket = {
        let b = m.bounds();
        let (lo, hi) = b.block_range();
        let height = (b.max.y - b.min.y).max(0.1);
        let foot = ((b.max.x - b.min.x) * (b.max.z - b.min.z)).max(1e-9);
        let mut slow = 0.0f64;
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let wide = (b.max.x.min(x as f64 + 1.0) - b.min.x.max(x as f64)).max(0.0);
                let deep = (b.max.z.min(z as f64 + 1.0) - b.min.z.max(z as f64)).max(0.0);
                let share = wide * deep / foot;
                if share <= 0.0 {
                    continue;
                }
                let (mut reach, mut dense) = (0.0, 0.0);
                for y in lo.y..=hi.y {
                    if let Some(p) = t.plant(BlockPos::new(x, y, z)) {
                        let floor = y as f64;
                        let len = ((floor + p.top).min(b.max.y) - floor.max(b.min.y)).max(0.0);
                        reach += len;
                        dense += len * p.drag;
                    }
                }
                if reach > 0.0 {
                    let r = (reach / height).min(1.0);
                    slow += share * (dense / reach) * r * r;
                }
            }
        }
        slow.clamp(0.0, 0.9)
    };
    speed *= 1.0 - thicket;
    if m.scramble_s > 0.0 {
        m.scramble_s -= dt;
        speed *= 0.35;
        rep.straining = true;
    }
    let target = wish * speed;
    let rate = if boating {
        // A hull glides on and answers the paddle slowly.
        1.5
    } else if m.stance == Stance::Swimming {
        3.0
    } else if m.on_ground && too_steep(m.ground_n, &ground) {
        // Sliding: the feet find little purchase.
        0.3
    } else if m.on_ground {
        12.0 * ((1.0 - ground.friction) / 0.4).clamp(0.05, 1.0)
    } else {
        0.5
    };
    let k = 1.0 - (-rate * dt).exp();
    let h = DVec2::new(m.vel.x, m.vel.z);
    let h = h + (target - h) * k;
    m.vel.x = h.x;
    m.vel.z = h.y;

    // Vertical: swim toward floating with the face out, climb ladders, or fall.
    let ladder = {
        let b = m.bounds().inflate(DVec3::new(0.15, 0.0, 0.15));
        let (lo, hi) = b.block_range();
        (lo.y..=hi.y).any(|y| {
            (lo.z..=hi.z).any(|z| (lo.x..=hi.x).any(|x| t.climbable(BlockPos::new(x, y, z))))
        })
    };
    if boating {
        // Seated in the hull, a hand's breadth under the surface.
        let float_y = level.unwrap_or(m.pos.y) - BOAT_SEAT_M;
        let vy = ((float_y - m.pos.y) * 3.0).clamp(-1.0, 1.0);
        m.vel.y += (vy - m.vel.y) * (1.0 - (-4.0 * dt).exp());
    } else if m.stance == Stance::Swimming {
        // Afloat with the eyes just out of the water.
        let float_y = level.unwrap_or(m.pos.y) - Stance::Swimming.eye() * m.scale + 0.1;
        let mut vy = ((float_y - m.pos.y) * 2.0).clamp(-1.2, 1.2);
        if i.jump {
            vy += 0.8;
        }
        if i.descend {
            vy -= 1.2;
        }
        if a.stamina < 0.05 && !i.jump {
            vy -= 0.35;
        }
        m.vel.y += (vy - m.vel.y) * (1.0 - (-3.0 * dt).exp());
    } else if ladder && (i.jump || i.descend || wish.length_squared() > 0.01) && a.climb {
        m.vel.y = if i.descend {
            -0.6
        } else if i.jump || wish.length_squared() > 0.01 {
            0.6
        } else {
            0.0
        };
        rep.motion = Motion::Climbing;
        rep.straining = true;
    } else {
        m.vel.y -= GRAVITY * (1.0 - 0.9 * submerged) * dt;
        m.vel.y -= AIR_DRAG * m.vel.y * m.vel.y.abs() * dt;
        if submerged > 0.0 {
            m.vel.y -= WATER_DRAG * submerged * m.vel.y * m.vel.y.abs() * dt;
        }
    }

    // Jumping, or pulling up onto a ledge ahead.
    if i.jump && m.on_ground && m.stance != Stance::Swimming && m.stance != Stance::Crawling {
        let fwd = wish.try_normalize();
        let ledge = fwd
            .filter(|_| a.climb && !ladder)
            .and_then(|f| find_ledge(t, m, f, &mut s))
            .filter(|top| top.y - m.pos.y > STEP_FREE);
        if let Some(top) = ledge {
            let rise = top.y - m.pos.y;
            let total = 0.6 + 1.2 * rise / REACH;
            m.climb = Some(Climb {
                to: top,
                from_y: m.pos.y,
                left_s: total,
                total_s: total,
            });
            m.stance = Stance::Climbing;
            m.vel = DVec3::ZERO;
            rep.motion = Motion::Climbing;
            rep.straining = true;
            return rep;
        } else if a.jump_m > 0.0 {
            m.vel.y = (2.0 * GRAVITY * a.jump_m * ground.jump).sqrt();
            m.on_ground = false;
        }
    } else if i.jump && m.stance == Stance::Swimming && a.climb {
        // Out of the water onto a bank.
        if let Some(f) = wish.try_normalize()
            && let Some(top) = find_ledge(t, m, f, &mut s)
            && top.y - m.pos.y <= 1.3
        {
            let total = 1.2;
            m.climb = Some(Climb {
                to: top,
                from_y: m.pos.y,
                left_s: total,
                total_s: total,
            });
            m.vel = DVec3::ZERO;
            rep.motion = Motion::Climbing;
            rep.straining = true;
            return rep;
        }
    }

    // Move through the blocks.
    let was_on_ground = m.on_ground;
    let start = m.bounds();
    let mut d = m.vel * dt;
    // Crouching keeps the feet on the edge.
    if m.stance == Stance::Crouching && was_on_ground {
        for k in [0usize, 2] {
            if d[k] == 0.0 {
                continue;
            }
            let mut probe = DVec3::ZERO;
            probe[k] = d[k];
            let below = start.offset(probe - DVec3::Y * 0.6);
            if !collides_above(
                t,
                &Aabb::new(below.min, DVec3::new(below.max.x, start.min.y, below.max.z)),
                0.0,
                &mut s,
            ) {
                d[k] = 0.0;
                m.vel[k] = 0.0;
            }
        }
    }
    let mut moved = sweep(t, &start, d, &mut s);
    // A step up: blocked in stride on the ground, try lifting the box over the obstacle.
    let blocked = (moved.x - d.x).abs() > 1e-6 || (moved.z - d.z).abs() > 1e-6;
    // Afloat, the bank is stepped up onto from the hull.
    if blocked && (was_on_ground || boating) && m.stance != Stance::Swimming {
        let max_step = match m.stance {
            Stance::Crawling => 0.3,
            _ if i.gait == Gait::Sprint && a.sprint => STEP_FREE,
            _ => STEP_SCRAMBLE,
        };
        let up = sweep(t, &start, DVec3::Y * max_step, &mut s).y;
        let lifted = start.offset(DVec3::Y * up);
        let across = sweep(t, &lifted, DVec3::new(d.x, 0.0, d.z), &mut s);
        let raised = lifted.offset(across);
        let down = sweep(t, &raised, DVec3::Y * (-up + d.y.min(0.0)), &mut s);
        let stepped = DVec3::new(across.x, up + down.y, across.z);
        let gain = |v: DVec3| v.x * v.x + v.z * v.z;
        if gain(stepped) > gain(moved) + 1e-8 && stepped.y > 1e-6 {
            if stepped.y > STEP_FREE && m.scramble_s <= 0.0 {
                m.scramble_s = 0.45;
            }
            moved = stepped;
            m.vel.y = 0.0;
        }
    }
    // The natural ground (S §8.1): the feet set on the field's surface, the body kept out of
    // ground too steep to step onto.
    let contact = if t.depth(m.pos).is_some() && m.stance != Stance::Swimming && !boating {
        let max_rise = match m.stance {
            Stance::Crawling => 0.3,
            _ if i.gait == Gait::Sprint && a.sprint => STEP_FREE,
            _ => STEP_SCRAMBLE,
        };
        let stick = was_on_ground && d.y <= 0.0;
        let c = field_move(t, m, &mut moved, d, stick, max_rise, &ground);
        if c.rise > STEP_FREE && m.scramble_s <= 0.0 {
            m.scramble_s = 0.45;
        }
        c
    } else {
        FieldContact::default()
    };
    m.pos += moved;
    m.pos.x = t.wrap_x(m.pos.x);
    if (moved.x - d.x).abs() > 1e-9 && !contact.on_ground {
        m.vel.x = 0.0;
    }
    if (moved.z - d.z).abs() > 1e-9 && !contact.on_ground {
        m.vel.z = 0.0;
    }
    if contact.blocked {
        m.vel.x = 0.0;
        m.vel.z = 0.0;
    }
    let hit_floor = (d.y < 0.0 && moved.y > d.y + 1e-9) || contact.landed;
    if (moved.y - d.y).abs() > 1e-9 || contact.on_ground {
        m.vel.y = 0.0;
    }
    m.on_ground = if hit_floor || contact.on_ground {
        true
    } else if d.y <= 0.0 {
        boxes_hit(t, &m.bounds().offset(-DVec3::Y * 0.02), &mut s)
    } else {
        false
    };
    m.ground_n = if contact.on_ground {
        contact.normal
    } else {
        DVec3::ZERO
    };
    // Ground too steep (or slippery) to stand on: sliding down it, the faster the steeper,
    // the grip of the soles against it.
    if contact.on_ground {
        let tan_max = max_slope_tan(&ground_under(t, m.pos));
        let n = contact.normal;
        let cos = n.y.clamp(0.0, 1.0);
        let sin = (1.0 - cos * cos).sqrt();
        if sin > tan_max * cos {
            let down = DVec3::new(n.x, 0.0, n.z).normalize_or_zero();
            let a = GRAVITY * (sin - 0.7 * tan_max * cos);
            m.vel += down * a * dt;
            rep.sliding = true;
        }
        let run = (moved.x * moved.x + moved.z * moved.z).sqrt();
        rep.grade = if run > 1e-6 { moved.y / run } else { 0.0 };
    }

    // Landings: on the ground, or into water (which passes on a share of the speed).
    let level = water_level(t, m.pos);
    let in_water = level.is_some_and(|l| l - m.pos.y > 0.3);
    if !m.on_ground && m.stance != Stance::Swimming && !ladder {
        m.fall_speed = m.fall_speed.max(-m.vel.y);
    }
    if in_water && !m.wet && m.fall_speed > 2.0 {
        rep.landed = Some(m.fall_speed * WATER_ENTRY);
        m.fall_speed = 0.0;
    }
    if m.on_ground && !was_on_ground {
        if m.fall_speed > 0.5 {
            let cushion = ground_under(t, m.pos).cushion;
            rep.landed = Some(m.fall_speed * (1.0 - 0.5 * cushion.clamp(0.0, 1.0)));
        }
        m.fall_speed = 0.0;
    }
    if m.on_ground || m.stance == Stance::Swimming || ladder {
        m.fall_speed = 0.0;
    }
    m.wet = in_water && !boating;

    // Breath.
    let eyes_under = level.is_some_and(|l| l > m.pos.y + m.eye_height());
    if eyes_under {
        m.breath_s -= dt;
        if m.breath_s <= 0.0 {
            m.breath_s = 0.0;
            m.airless_s += dt;
        }
    } else {
        m.breath_s = (m.breath_s + 3.0 * dt).min(a.breath_s);
        m.airless_s = 0.0;
    }

    let hspeed = (moved.x * moved.x + moved.z * moved.z).sqrt() / dt;
    let depth = level.map_or(0.0, |l| l - m.pos.y);
    rep.immersion = if boating {
        0.0
    } else {
        (depth / m.height()).clamp(0.0, 1.0)
    };
    rep.eyes_under = eyes_under;
    rep.airless_s = m.airless_s;
    rep.speed = hspeed;
    if rep.motion == Motion::Still {
        rep.motion = match m.stance {
            _ if boating && hspeed > 0.1 => Motion::Paddling,
            _ if boating => Motion::Still,
            Stance::Swimming => Motion::Swimming,
            Stance::Climbing => Motion::Climbing,
            _ if !m.on_ground && m.fall_speed > 2.0 => Motion::Falling,
            Stance::Crawling if hspeed > 0.05 => Motion::Crawling,
            Stance::Crouching if hspeed > 0.05 => Motion::Crouching,
            _ if submerged > 0.3 && hspeed > 0.05 => Motion::Wading,
            _ if hspeed > 4.5 => Motion::Sprinting,
            _ if hspeed > 2.2 => Motion::Jogging,
            _ if hspeed > 0.3 => Motion::Walking,
            _ => Motion::Still,
        };
    }
    rep
}
