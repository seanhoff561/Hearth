//! What the player hears (V2-3): footsteps by the ground underfoot and the gait, landings,
//! splashes and strokes, the weather where they stand (dulled under a roof), the echo of a
//! cave, and their own heart and breath as the body labours, bleeds, is chilled or holds its
//! breath under water. Captions say the same in words when the options ask for them.

use glam::DVec3;
use hearth_audio::{Ambience, Bus, Command, Sound, Surface};
use hearth_math::BlockPos;
use hearth_physics::{Motion, Mover, Report};
use hearth_protocol::BodyView;
use hearth_world::{BlockRegistry, CubeMap};

/// Seconds a caption stays after its sound.
const CAPTION_S: f64 = 3.0;
/// Seconds between looks at how shut in the player is.
const ENCLOSURE_S: f64 = 0.25;

/// The heart and the breath as heard.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rhythms {
    pub heart_bpm: f32,
    pub heart: f32,
    pub breaths_per_min: f32,
    pub breath: f32,
    pub shiver: f32,
    /// 0–1: so weak the world sounds far away.
    pub weak: f32,
}

impl Default for Rhythms {
    fn default() -> Self {
        Self {
            heart_bpm: 62.0,
            heart: 0.0,
            breaths_per_min: 12.0,
            breath: 0.0,
            shiver: 0.0,
            weak: 0.0,
        }
    }
}

#[derive(Default)]
pub struct Hearing {
    /// Sounds to play, taken each frame.
    pub out: Vec<Command>,
    /// Distance since the last footstep or stroke (m), and which foot is next.
    stride: f64,
    foot: bool,
    was_immersed: f64,
    was_under: bool,
    /// Longest without air in this dive (s).
    held_s: f64,
    /// Exertion felt (0–1), easing up and slowly back down; the cold shock's gasping (0–1).
    effort: f64,
    shock: f64,
    /// In cold water since its shock (no second gasp until out).
    dipped: bool,
    injuries: usize,
    enclosed: f32,
    buried: f32,
    since_enclosure: f64,
    pub rhythms: Rhythms,
    /// Captions (language keys) and the seconds left to show each.
    captions: Vec<(&'static str, f64)>,
    /// Seconds to the next rumble of an empty stomach and dry swallow, and the state of the
    /// generator spacing them.
    growl_in: f64,
    swallow_in: f64,
    luck: u32,
}

impl Hearing {
    fn play(&mut self, sound: Sound, pan: f32, caption: &'static str) {
        self.out.push(Command::Play {
            sound,
            bus: Bus::Players,
            gain: 1.0,
            pan,
        });
        self.caption(caption);
    }

    /// A random time between `lo` and `hi` (s), so cues don't come like clockwork.
    fn between(&mut self, lo: f64, hi: f64) -> f64 {
        if self.luck == 0 {
            self.luck = 0x9e37_79b9;
        }
        self.luck ^= self.luck << 13;
        self.luck ^= self.luck >> 17;
        self.luck ^= self.luck << 5;
        lo + (hi - lo) * (self.luck as f64 / u32::MAX as f64)
    }

    fn caption(&mut self, key: &'static str) {
        match self.captions.iter_mut().find(|(k, _)| *k == key) {
            Some((_, left)) => *left = CAPTION_S,
            None => self.captions.push((key, CAPTION_S)),
        }
    }

    /// The captions showing and their opacity (0–1).
    pub fn captions(&self) -> impl Iterator<Item = (&'static str, f32)> + '_ {
        self.captions
            .iter()
            .map(|&(k, left)| (k, left.clamp(0.0, 1.0) as f32))
    }

    /// The player moved this step: footsteps, landings, splashes, strokes, coming up for air.
    /// `vy_before` is the vertical speed before the step. Returns the foot that came down
    /// (true for the left), if one did.
    pub fn moved(
        &mut self,
        map: &CubeMap,
        reg: &BlockRegistry,
        mover: &Mover,
        report: &Report,
        vy_before: f64,
        dt: f64,
    ) -> Option<bool> {
        let immersion = report.immersion;
        // Into the water.
        if self.was_immersed < 0.15 && immersion >= 0.15 {
            let speed = (-vy_before).max(report.speed * 0.5);
            if speed > 1.5 {
                self.play(
                    Sound::Splash {
                        speed: speed as f32,
                    },
                    0.0,
                    "subtitles.splash",
                );
            }
        }
        self.was_immersed = immersion;
        // Up for air after a long breath held.
        if report.eyes_under {
            self.held_s = self.held_s.max(report.airless_s);
        } else {
            if self.was_under && self.held_s > 5.0 {
                let force = ((self.held_s - 5.0) / 30.0).clamp(0.3, 1.0) as f32;
                self.play(Sound::Gasp { force }, 0.0, "subtitles.gasp");
            }
            self.held_s = 0.0;
        }
        self.was_under = report.eyes_under;
        if let Some(v) = report.landed
            && v > 1.5
            && immersion < 0.3
        {
            let surface = surface_under(map, reg, mover.pos);
            self.play(
                Sound::Land {
                    surface,
                    impact: v as f32,
                },
                0.0,
                "subtitles.land",
            );
            self.stride = 0.0;
        }
        // Steps, rungs and strokes.
        let (every, force) = match report.motion {
            Motion::Walking => (0.75, 0.55),
            Motion::Jogging => (1.15, 0.8),
            Motion::Sprinting => (1.6, 1.0),
            Motion::Crouching => (0.55, 0.25),
            Motion::Crawling => (0.45, 0.3),
            Motion::Wading => (0.7, 0.7),
            Motion::Climbing => (0.55, 0.45),
            Motion::Swimming => (1.1, 0.0),
            Motion::Still | Motion::Falling => (0.0, 0.0),
        };
        let moving = match report.motion {
            Motion::Climbing => mover.vel.y.abs(),
            // Treading water strokes too, more slowly.
            Motion::Swimming => report.speed.max(0.4),
            _ if mover.on_ground || report.motion == Motion::Wading => report.speed,
            _ => 0.0,
        };
        if every == 0.0 || moving < 0.05 {
            // The first step comes soon after setting off.
            self.stride = self.stride.min(0.4);
            return None;
        }
        self.stride += moving * dt;
        if self.stride < every {
            return None;
        }
        self.stride = (self.stride - every).min(every);
        self.foot = !self.foot;
        let pan = if self.foot { -0.12 } else { 0.12 };
        if report.motion == Motion::Swimming {
            self.play(
                Sound::Stroke {
                    under: report.eyes_under,
                },
                0.0,
                "subtitles.swim",
            );
            return None;
        }
        let surface = if report.motion == Motion::Wading || immersion > 0.08 {
            Surface::Shallow
        } else if report.motion == Motion::Climbing {
            climbed_surface(map, reg, mover.pos)
        } else {
            surface_under(map, reg, mover.pos)
        };
        self.play(
            Sound::Step {
                surface,
                force: force as f32,
            },
            pan,
            "subtitles.step",
        );
        Some(self.foot)
    }

    /// The body's news: new injuries, cold water's shock, the heart and the breath.
    pub fn body(&mut self, body: Option<&BodyView>, report: Option<&Report>, dt: f64) {
        let Some(b) = body else {
            self.rhythms = Rhythms::default();
            return;
        };
        if b.dead.is_some() {
            self.rhythms = Rhythms {
                heart: 0.0,
                breath: 0.0,
                ..Rhythms::default()
            };
            self.injuries = b.injuries.len();
            return;
        }
        if b.injuries.len() > self.injuries
            && let Some(new) = b.injuries.last()
        {
            let fracture = new.id.ends_with("fracture");
            self.play(
                Sound::Hurt {
                    force: 0.4 + new.severity,
                    fracture,
                },
                0.0,
                "subtitles.hurt",
            );
        }
        self.injuries = b.injuries.len();
        let e = &b.exposure;
        // Cold water takes the breath away: a gasp, then fast breathing for a minute or so.
        if e.immersion < 0.2 {
            self.dipped = false;
        }
        if e.immersion >= 0.5 && !self.dipped && e.water_c < 15.0 {
            self.dipped = true;
            let shock = ((15.0 - e.water_c) / 10.0).clamp(0.2, 1.0) as f64;
            self.shock = shock;
            self.play(
                Sound::Gasp {
                    force: shock as f32,
                },
                0.0,
                "subtitles.gasp",
            );
        }
        if e.immersion < 0.2 {
            self.shock *= (-dt / 10.0).exp();
        } else {
            self.shock *= (-dt / 40.0).exp();
        }
        let motion = report.map_or(Motion::Still, |r| r.motion);
        let work: f64 = match motion {
            Motion::Still | Motion::Falling => 0.0,
            Motion::Walking | Motion::Crouching => 0.2,
            Motion::Crawling => 0.35,
            Motion::Wading => 0.4,
            Motion::Jogging => 0.55,
            Motion::Swimming => 0.6,
            Motion::Climbing => 0.7,
            Motion::Sprinting => 0.95,
        };
        let s = &b.status;
        let tired = (1.0 - s.stamina as f64).clamp(0.0, 1.0);
        let target = work.max(tired * 0.9);
        let tau = if target > self.effort { 8.0 } else { 25.0 };
        self.effort += (target - self.effort) * (1.0 - (-dt / tau).exp());
        // An empty stomach rumbles now and then; a dry throat swallows.
        if s.hunger >= hearth_body::Hunger::Hungry && !b.asleep {
            self.growl_in -= dt;
            if self.growl_in <= 0.0 {
                let starving = s.hunger >= hearth_body::Hunger::VeryHungry;
                self.growl_in = if starving {
                    self.between(25.0, 60.0)
                } else {
                    self.between(40.0, 120.0)
                };
                let force = if starving { 1.0 } else { 0.6 };
                self.play(Sound::Stomach { force }, 0.0, "subtitles.stomach");
            }
        } else {
            self.growl_in = self.growl_in.max(20.0);
        }
        if s.thirst >= hearth_body::Thirst::Thirsty && !b.asleep {
            self.swallow_in -= dt;
            if self.swallow_in <= 0.0 {
                self.swallow_in = self.between(45.0, 120.0);
                self.play(Sound::Swallow, 0.0, "subtitles.swallow");
            }
        } else {
            self.swallow_in = self.swallow_in.max(20.0);
        }
        let core = s.core_c as f64;
        // The heart quickens with work, blood lost, fever, pain and fright, and slows as the
        // core cools.
        let mut bpm = 62.0
            + 115.0 * self.effort
            + 120.0 * s.blood_lost as f64
            + 10.0 * (core - 37.3).max(0.0)
            - 8.0 * (35.0 - core).max(0.0)
            + 25.0 * s.effects.pain as f64
            + 30.0 * self.shock;
        if b.asleep {
            bpm = bpm.min(58.0);
        }
        let bpm = bpm.clamp(30.0, 200.0) as f32;
        let airless = report.map_or(0.0, |r| r.airless_s) as f32;
        let danger = ramp(s.blood_lost, 0.15, 0.35)
            .max(ramp(airless, 15.0, 40.0))
            .max(0.6 * ramp(35.0 - s.core_c, 0.0, 3.0));
        let heart = (0.8 * ramp(bpm, 125.0, 175.0)).max(danger);
        let under = report.is_some_and(|r| r.eyes_under);
        let effort = self.effort as f32;
        let breath = if under || b.asleep {
            0.0
        } else {
            (0.9 * ramp(effort, 0.45, 1.0) + 0.8 * self.shock as f32 + 0.3 * s.effects.shivering)
                .min(1.0)
        };
        self.rhythms = Rhythms {
            heart_bpm: bpm,
            heart,
            breaths_per_min: (12.0 + 30.0 * effort + 20.0 * self.shock as f32).min(50.0),
            breath,
            shiver: s.effects.shivering,
            weak: ramp(s.blood_lost, 0.2, 0.38).max(0.8 * (1.0 - s.effects.vision)),
        };
        if heart > 0.3 {
            self.caption("subtitles.heart");
        }
        if breath > 0.3 {
            self.caption("subtitles.breath");
        }
    }

    /// How shut in the eyes are, looked at a few times a second.
    pub fn look_around(&mut self, map: &CubeMap, reg: &BlockRegistry, eye: DVec3, dt: f64) {
        self.since_enclosure += dt;
        if self.since_enclosure >= ENCLOSURE_S {
            self.since_enclosure = 0.0;
            self.enclosed = enclosure(map, reg, eye);
            self.buried = buried(map, reg, eye);
        }
    }

    /// The surroundings to sound: `weather` is the wind (m/s) and the rain (mm/h of water) at
    /// the place, `sheltered` whether a roof is overhead.
    pub fn ambience(
        &mut self,
        weather: (f32, f32),
        sheltered: bool,
        underwater: bool,
        paused: bool,
        dt: f64,
    ) -> Ambience {
        let (wind, rain) = weather;
        if !underwater && !paused {
            if wind >= 12.0 {
                self.caption("subtitles.wind.strong");
            } else if wind >= 4.0 {
                self.caption("subtitles.wind");
            }
            if rain >= 0.3 {
                self.caption(if sheltered {
                    "subtitles.rain.roof"
                } else {
                    "subtitles.rain"
                });
            }
        }
        for (_, left) in &mut self.captions {
            *left -= dt;
        }
        self.captions.retain(|(_, left)| *left > 0.0);
        let r = self.rhythms;
        Ambience {
            wind_m_s: wind,
            rain_mm_h: rain,
            sheltered: if sheltered { 1.0 } else { 0.0 },
            buried: if sheltered { self.buried } else { 0.0 },
            enclosed: self.enclosed,
            underwater,
            weak: r.weak,
            heart_bpm: r.heart_bpm,
            heart: r.heart,
            breaths_per_min: r.breaths_per_min,
            breath: r.breath,
            shiver: r.shiver,
            paused,
        }
    }
}

fn ramp(x: f32, lo: f32, hi: f32) -> f32 {
    ((x - lo) / (hi - lo)).clamp(0.0, 1.0)
}

/// What the feet stand on: a layer at the feet (snow, moss, a slab), plants underfoot, or the
/// block below (looking under the corners when standing at an edge).
pub fn surface_under(map: &CubeMap, reg: &BlockRegistry, feet: DVec3) -> Surface {
    let group = |s| Surface::of_group(&reg.block_of(s).def.sound);
    if let Some(s) = map.block(BlockPos::containing(feet + DVec3::new(0.0, 0.01, 0.0)))
        && !s.is_air()
        && reg.fluid_amount(s) == 0
    {
        if !reg.collision_shape(s).is_empty() {
            return group(s);
        }
        // Walked through: stones turning underfoot, a dusting of snow, grass and flowers.
        return match group(s) {
            Surface::Stone | Surface::Gravel => Surface::Gravel,
            g @ (Surface::Snow | Surface::Moss | Surface::Sand | Surface::Leaves) => g,
            _ => Surface::Grass,
        };
    }
    for (dx, dz) in [
        (0.0, 0.0),
        (0.3, 0.3),
        (-0.3, 0.3),
        (0.3, -0.3),
        (-0.3, -0.3),
    ] {
        let p = feet + DVec3::new(dx, -0.05, dz);
        if let Some(s) = map.block(BlockPos::containing(p))
            && !s.is_air()
            && !reg.collision_shape(s).is_empty()
        {
            return group(s);
        }
    }
    Surface::Soil
}

/// What the hands and feet climb: a ladder or vine at the body, else the face ahead.
fn climbed_surface(map: &CubeMap, reg: &BlockRegistry, feet: DVec3) -> Surface {
    let at = BlockPos::containing(feet + DVec3::new(0.0, 0.5, 0.0));
    match map.block(at) {
        Some(s) if !s.is_air() && reg.fluid_amount(s) == 0 => {
            Surface::of_group(&reg.block_of(s).def.sound)
        }
        _ => surface_under(map, reg, feet),
    }
}

/// 0 under a thin roof to 1 under six or more solid blocks: how far the weather is.
pub fn buried(map: &CubeMap, reg: &BlockRegistry, eye: DVec3) -> f32 {
    let (x, z) = (eye.x.floor() as i32, eye.z.floor() as i32);
    let y0 = eye.y.floor() as i32 + 1;
    let Some(top) = map.sky_top(x, z) else {
        return 0.0;
    };
    let solid = (y0..=top.min(y0 + 8))
        .filter(|&y| {
            map.block(BlockPos::new(x, y, z))
                .is_some_and(|s| reg.is_opaque(s))
        })
        .count();
    ramp(solid as f32, 1.0, 6.0)
}

/// Directions looked along for walls and a roof: up, the four sides, and up between them.
const LOOKS: [[f64; 3]; 9] = [
    [0.0, 1.0, 0.0],
    [1.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0],
    [0.0, 0.0, -1.0],
    [0.577, 0.577, 0.577],
    [-0.577, 0.577, 0.577],
    [0.577, 0.577, -0.577],
    [-0.577, 0.577, -0.577],
];

/// 0 in the open to 1 shut in: the share of looks that meet solid blocks within 16 m.
pub fn enclosure(map: &CubeMap, reg: &BlockRegistry, eye: DVec3) -> f32 {
    let mut hits = 0;
    for d in LOOKS {
        let d = DVec3::from_array(d);
        let mut t = 0.5;
        while t <= 16.0 {
            if let Some(s) = map.block(BlockPos::containing(eye + d * t))
                && reg.is_opaque(s)
            {
                hits += 1;
                break;
            }
            t += 0.5;
        }
    }
    hits as f32 / LOOKS.len() as f32
}
