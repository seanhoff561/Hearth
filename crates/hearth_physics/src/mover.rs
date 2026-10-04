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
        out.motion = r.motion;
        out.speed = r.speed;
        out.immersion = r.immersion;
        out.eyes_under = r.eyes_under;
        out.airless_s = r.airless_s;
    }
    out
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

fn collides(t: &impl Terrain, b: &Aabb, scratch: &mut Vec<Aabb>) -> bool {
    scratch.clear();
    t.boxes(&b.inflate(DVec3::splat(-1e-4)), scratch);
    scratch
        .iter()
        .any(|o| o.intersects(&b.inflate(DVec3::splat(-1e-4))))
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
    if !collides(
        t,
        &Aabb::from_feet(m.pos + ahead + DVec3::Y * 0.05, width, 0.5),
        s,
    ) {
        return None;
    }
    let mut dy = 0.125;
    while dy <= REACH * m.scale + 1e-9 {
        let top = m.pos + ahead + DVec3::Y * dy;
        let spot = Aabb::from_feet(top, width, h);
        let rise = Aabb::from_feet(m.pos + DVec3::Y * dy, width, h);
        let support = Aabb::from_feet(top - DVec3::Y * 0.1, width * 0.6, 0.1);
        if !collides(t, &spot, s) && !collides(t, &rise, s) && collides(t, &support, s) {
            return Some(top);
        }
        dy += 0.0625;
    }
    None
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
        t.ground(BlockPos::containing(m.pos - DVec3::Y * 0.05))
    } else {
        Ground::default()
    };

    // Stance: swim when the water is over the chest and the feet find no bottom (and keep
    // swimming until they stand in shallower water); otherwise as asked, if there is room.
    let swimming = if m.stance == Stance::Swimming {
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
    }
    // Pushing through foliage and brush: the densest the body is in slows it.
    let thicket = {
        let b = m.bounds();
        let (lo, hi) = b.block_range();
        let mut most = 0.0f64;
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                for x in lo.x..=hi.x {
                    most = most.max(t.drag(BlockPos::new(x, y, z)));
                }
            }
        }
        most.clamp(0.0, 0.9)
    };
    speed *= 1.0 - thicket;
    if m.scramble_s > 0.0 {
        m.scramble_s -= dt;
        speed *= 0.35;
        rep.straining = true;
    }
    let target = wish * speed;
    let rate = if m.stance == Stance::Swimming {
        3.0
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
    if m.stance == Stance::Swimming {
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
            if !collides(
                t,
                &Aabb::new(below.min, DVec3::new(below.max.x, start.min.y, below.max.z)),
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
    if blocked && was_on_ground && m.stance != Stance::Swimming {
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
    m.pos += moved;
    m.pos.x = t.wrap_x(m.pos.x);
    if (moved.x - d.x).abs() > 1e-9 {
        m.vel.x = 0.0;
    }
    if (moved.z - d.z).abs() > 1e-9 {
        m.vel.z = 0.0;
    }
    let hit_floor = d.y < 0.0 && moved.y > d.y + 1e-9;
    if (moved.y - d.y).abs() > 1e-9 {
        m.vel.y = 0.0;
    }
    m.on_ground = if hit_floor {
        true
    } else if d.y <= 0.0 {
        collides(t, &m.bounds().offset(-DVec3::Y * 0.02), &mut s)
    } else {
        false
    };

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
            let cushion = t
                .ground(BlockPos::containing(m.pos - DVec3::Y * 0.05))
                .cushion;
            rep.landed = Some(m.fall_speed * (1.0 - 0.5 * cushion.clamp(0.0, 1.0)));
        }
        m.fall_speed = 0.0;
    }
    if m.on_ground || m.stance == Stance::Swimming || ladder {
        m.fall_speed = 0.0;
    }
    m.wet = in_water;

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
    rep.immersion = (depth / m.height()).clamp(0.0, 1.0);
    rep.eyes_under = eyes_under;
    rep.airless_s = m.airless_s;
    rep.speed = hspeed;
    if rep.motion == Motion::Still {
        rep.motion = match m.stance {
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
