//! Resting and waiting (E §4.3, Amendment P §7.1): the player lies down to sleep, or to rest
//! until something — some hours, dusk, the work left to itself nearby done — and the world goes
//! faster meanwhile, smoothly up to the world's sleep speed (`time.ron`). Nothing is skipped:
//! the world lives every tick of it, only more of them to the second. Anything that needs the
//! player ends it: the body's needs (cold, heat, rain, pain, hunger, thirst), a hurt, an animal
//! come near. This is the only way time goes faster than it is lived (Creative's time speed and
//! tests' apart); work in hand never does.

use glam::DVec3;
use hearth_body::Wake;
use hearth_fauna::live::Live;
use hearth_fauna::species::Catalog;
use hearth_items::WorldItems;
use hearth_player::Player;
use hearth_protocol::{Rest, RestEnd, Rested};

/// The longest a rest goes on (hours): the morning or the dusk that does not come in a polar
/// summer or winter, a sleep that never comes.
pub const MOST_H: f64 = 24.0;
/// How near work left to itself is to be waited on (m).
pub const WAITING_M: f64 = 12.0;
/// How near an animal the size of a person's dog or more comes before it ends a rest (m).
const NEAR_M: f64 = 10.0;
/// How near one that has turned on the person is when it ends a rest (m).
const HOSTILE_M: f64 = 40.0;
/// Animals lighter than this (kg) come and go about a resting person.
const SMALL_KG: f32 = 15.0;

/// A rest under way.
#[derive(Debug, Clone)]
pub struct Resting {
    pub rest: Rest,
    /// The tick it began.
    since: u64,
    /// Ticks of it asleep.
    slept: u64,
    /// Whether the sun was up when last looked.
    sun_up: bool,
    /// The things left to their work it waits on.
    waiting: Vec<u64>,
    /// How many injuries the body had.
    hurt: usize,
}

/// What a tick of rest sees.
#[derive(Debug, Clone, Default)]
pub struct Seen {
    pub ticks: u64,
    pub ticks_per_hour: f64,
    pub sun_up: bool,
    /// Why the body woke this tick, if it did.
    pub woke: Option<Wake>,
    /// What would not let the body rest awake.
    pub needs: Option<Wake>,
    /// The nearest animal that ends a rest, by name.
    pub animal: Option<String>,
}

impl Resting {
    /// A rest begun at `ticks`, or why it cannot be.
    pub fn begin(
        rest: Rest,
        ticks: u64,
        player: &Player,
        sun_up: bool,
        items: &WorldItems,
    ) -> Result<Self, RestEnd> {
        let waiting = if rest == Rest::UntilDone {
            waiting_near(items, player.mover.pos)
        } else {
            Vec::new()
        };
        if rest == Rest::UntilDone && waiting.is_empty() {
            return Err(RestEnd::NothingWaiting);
        }
        Ok(Self {
            rest,
            since: ticks,
            slept: 0,
            sun_up,
            waiting,
            hurt: player.body.injuries.len(),
        })
    }

    /// Lives a tick of the rest, `advanced` of the world's: how it ended, when it does.
    pub fn tick(
        &mut self,
        player: &Player,
        items: &WorldItems,
        advanced: u64,
        seen: &Seen,
    ) -> Option<RestEnd> {
        if player.asleep {
            self.slept += advanced;
        }
        let rose = seen.sun_up && !self.sun_up;
        let set = !seen.sun_up && self.sun_up;
        self.sun_up = seen.sun_up;
        if player.body.injuries.len() > self.hurt {
            return Some(RestEnd::Hurt);
        }
        if let Some(why) = seen.woke.filter(|w| *w != Wake::Rested) {
            return Some(RestEnd::Woke(why));
        }
        if !player.asleep
            && let Some(why) = seen.needs.filter(|w| *w != Wake::Rested)
        {
            return Some(RestEnd::Needs(why));
        }
        if let Some(name) = &seen.animal {
            return Some(RestEnd::Animal(name.clone()));
        }
        let hours = self.hours(seen);
        let came = match self.rest {
            Rest::SleepUntilMorning => rose,
            Rest::SleepUntilRested => seen.woke == Some(Wake::Rested),
            Rest::Hours(h) => hours >= h as f64,
            Rest::UntilDusk => set,
            Rest::UntilDone => self
                .waiting
                .iter()
                .all(|id| items.get(*id).is_none_or(|w| w.work.is_none())),
        };
        (came || hours >= MOST_H).then_some(RestEnd::Came)
    }

    /// Hours since it began.
    fn hours(&self, seen: &Seen) -> f64 {
        seen.ticks.saturating_sub(self.since) as f64 / seen.ticks_per_hour
    }

    /// How it went, ended by `end`.
    pub fn rested(&self, seen: &Seen, end: RestEnd) -> Rested {
        Rested {
            hours: self.hours(seen) as f32,
            slept_h: (self.slept as f64 / seen.ticks_per_hour) as f32,
            rest: self.rest,
            end,
        }
    }
}

/// The things left to their work within reach of a rest at `at`.
pub fn waiting_near(items: &WorldItems, at: DVec3) -> Vec<u64> {
    items
        .items
        .iter()
        .filter(|w| w.work.is_some() && DVec3::from_array(w.pos).distance(at) <= WAITING_M)
        .map(|w| w.id)
        .collect()
}

/// The nearest animal that ends a rest at `at`: one turned on the person, or one of some size
/// come close (the person's own kept animals apart).
pub fn animal_near(live: &Live, cat: &Catalog, at: DVec3) -> Option<String> {
    live.animals
        .iter()
        .filter(|a| !a.dead && a.kept.is_none())
        .filter_map(|a| {
            let sp = cat.species.get(a.species as usize)?;
            let d = a.pos.distance(at);
            let ends =
                (a.hostile.is_some() && d <= HOSTILE_M) || (sp.mass_kg >= SMALL_KG && d <= NEAR_M);
            ends.then(|| (d, sp.name.to_lowercase()))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, name)| name)
}

/// How much faster the world goes resting (extra ticks a second of play), eased toward its
/// target, most of the way in `ramp_s`: up to `max_factor` resting, back to none after (below
/// twice as fast, at once).
pub fn ease(speed: f64, resting: bool, max_factor: f64, ramp_s: f64, dt: f64) -> f64 {
    let target = if resting {
        20.0 * (max_factor - 1.0).max(0.0)
    } else {
        0.0
    };
    let eased = speed + (target - speed) * (1.0 - (-3.0 * dt / ramp_s.max(0.1)).exp());
    if !resting && eased < 20.0 { 0.0 } else { eased }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hearth_items::{Batch, Stack};

    fn player() -> Player {
        let content = hearth_content::Content::load_base();
        let cfg = hearth_body::BodyConfig::with_rates(&content, hearth_body::Rates::authentic());
        Player::new(&cfg, DVec3::ZERO, 1)
    }

    fn seen(ticks: u64, sun_up: bool) -> Seen {
        Seen {
            ticks,
            ticks_per_hour: 72_000.0,
            sun_up,
            ..Seen::default()
        }
    }

    #[test]
    fn each_rest_ends_when_what_it_is_for_comes() {
        let p = player();
        let items = WorldItems::default();
        // Some hours.
        let mut r = Resting::begin(Rest::Hours(2.0), 0, &p, true, &items).unwrap();
        assert_eq!(r.tick(&p, &items, 1, &seen(72_000, true)), None);
        assert_eq!(
            r.tick(&p, &items, 1, &seen(144_000, true)),
            Some(RestEnd::Came)
        );
        assert_eq!(r.rested(&seen(144_000, true), RestEnd::Came).hours, 2.0);
        // Dusk: when the sun goes down, not before.
        let mut r = Resting::begin(Rest::UntilDusk, 0, &p, true, &items).unwrap();
        assert_eq!(r.tick(&p, &items, 1, &seen(10, true)), None);
        assert_eq!(r.tick(&p, &items, 1, &seen(20, false)), Some(RestEnd::Came));
        // The morning: when the sun rises.
        let mut r = Resting::begin(Rest::SleepUntilMorning, 0, &p, false, &items).unwrap();
        assert_eq!(r.tick(&p, &items, 1, &seen(10, false)), None);
        assert_eq!(r.tick(&p, &items, 1, &seen(20, true)), Some(RestEnd::Came));
        // Rested: when the body wakes rested.
        let mut r = Resting::begin(Rest::SleepUntilRested, 0, &p, false, &items).unwrap();
        let rested = Seen {
            woke: Some(Wake::Rested),
            ..seen(10, false)
        };
        assert_eq!(r.tick(&p, &items, 1, &rested), Some(RestEnd::Came));
        // A morning that never comes (a polar winter) ends after a day.
        let mut r = Resting::begin(Rest::SleepUntilMorning, 0, &p, false, &items).unwrap();
        assert_eq!(
            r.tick(&p, &items, 1, &seen(24 * 72_000, false)),
            Some(RestEnd::Came)
        );
    }

    #[test]
    fn waiting_on_work_ends_when_it_is_done_and_needs_some_left_to_it() {
        let p = player();
        let mut items = WorldItems::default();
        assert_eq!(
            Resting::begin(Rest::UntilDone, 0, &p, true, &items).unwrap_err(),
            RestEnd::NothingWaiting
        );
        let meat = items.add(Stack::one("hearth:venison"), [3.0, 0.0, 0.0], 0.0);
        items.get_mut(meat).unwrap().work = Some(Batch {
            process: "hearth:dry_meat".into(),
            hours: 0.0,
            wet_hours: 0.0,
            peak_c: 0.0,
            hot_h: 0.0,
        });
        // Too far to wait on.
        let far = items.add(Stack::one("hearth:venison"), [40.0, 0.0, 0.0], 0.0);
        items.get_mut(far).unwrap().work = items.get(meat).unwrap().work.clone();
        let mut r = Resting::begin(Rest::UntilDone, 0, &p, true, &items).unwrap();
        assert_eq!(r.waiting, vec![meat]);
        assert_eq!(r.tick(&p, &items, 1, &seen(10, true)), None);
        items.take(meat);
        assert_eq!(r.tick(&p, &items, 1, &seen(20, true)), Some(RestEnd::Came));
    }

    #[test]
    fn what_needs_the_player_ends_a_rest() {
        let mut p = player();
        let items = WorldItems::default();
        let mut r = Resting::begin(Rest::Hours(4.0), 0, &p, true, &items).unwrap();
        // Awake, the cold ends it.
        let cold = Seen {
            needs: Some(Wake::Cold),
            ..seen(10, true)
        };
        assert_eq!(
            r.tick(&p, &items, 1, &cold),
            Some(RestEnd::Needs(Wake::Cold))
        );
        // Asleep, the body's own waking does.
        p.asleep = true;
        let woke = Seen {
            woke: Some(Wake::Thirst),
            ..seen(10, true)
        };
        assert_eq!(
            r.tick(&p, &items, 1, &woke),
            Some(RestEnd::Woke(Wake::Thirst))
        );
        let wolf = Seen {
            animal: Some("wolf".into()),
            ..seen(10, true)
        };
        assert_eq!(
            r.tick(&p, &items, 1, &wolf),
            Some(RestEnd::Animal("wolf".into()))
        );
        // Asleep the whole hour of it.
        let mut r = Resting::begin(Rest::Hours(1.0), 0, &p, true, &items).unwrap();
        r.tick(&p, &items, 72_000, &seen(72_000, true));
        assert_eq!(r.rested(&seen(72_000, true), RestEnd::Came).slept_h, 1.0);
    }

    #[test]
    fn the_world_eases_up_to_the_sleep_speed_and_back() {
        let (max, ramp, dt) = (100.0, 3.0, 0.05);
        let mut speed = 0.0;
        let mut t = 0.0;
        while speed < 0.95 * 20.0 * 99.0 {
            speed = ease(speed, true, max, ramp, dt);
            t += dt;
        }
        assert!((2.5..3.5).contains(&t), "up in {t:.1} s");
        let mut t = 0.0;
        while speed > 0.0 {
            speed = ease(speed, false, max, ramp, dt);
            t += dt;
        }
        assert!(t < 5.0, "back in {t:.1} s");
    }
}
