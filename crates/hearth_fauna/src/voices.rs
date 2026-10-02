//! Voices (V2-7 (i), docs/design/fauna.md "Calls and ambient life"): the calls animals make
//! and when — the alarm of one that starts to run, the distress of one hurt or caught, the
//! threat of one that turns on a person, and those of habit while their occasion holds: a stag
//! roaring in the rut, a herd's contact, a pack's howl by night, an owl's hoot, a fox's bark, a
//! bird's song at dawn and through the day; and the chorus of the animals about the player that
//! are not in the world, from the populations.

use glam::DVec3;
use hearth_content::schema::fauna::CallWhen;
use hearth_math::hash::Rng;
use serde::{Deserialize, Serialize};

use crate::ecology::{Ecology, REGION_LEN, dist, poisson};
use crate::live::{Now, awake};
use crate::species::Species;

/// A call made: whose (species), which of its calls (its place in the species' `calls`), from
/// where.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Called {
    pub species: u16,
    pub call: u8,
    pub pos: DVec3,
}

/// The first of a species' calls for an occasion.
pub fn call_for(sp: &Species, when: CallWhen) -> Option<u8> {
    sp.calls
        .iter()
        .position(|c| c.when == when)
        .map(|i| i as u8)
}

/// How often an animal makes its call for an occasion now (calls an hour as it lives them,
/// second by second, whatever the calendar's pace; none when the occasion does not hold): a
/// stag in the rut about once a minute at dusk, by night and at dawn; a herd's contact calls
/// now and then while awake; a wolf pack's howl a few times an hour by night, an owl's hoot a
/// few times a minute or two; a fox's bark while it is about; a bird's song every ten seconds
/// or so at dawn and a fifth as often through the day, most in spring, hardly at all in winter;
/// a woodpecker's drumming in spring mornings.
pub fn calls_now(sp: &Species, when: CallWhen, male_adult: bool, season: usize, now: &Now) -> f32 {
    use hearth_content::schema::fauna::CallKind;
    let h = now.hour;
    let dark = now.air.light < 0.15;
    let kind = sp.calls.iter().find(|c| c.when == when).map(|c| c.kind);
    // Song and drumming by the season (0 spring … 3 winter).
    let singing = [1.0, 0.6, 0.15, 0.05][season.min(3)];
    match (when, kind) {
        (CallWhen::Rut, _) => {
            let dusk_or_dawn = !(0.32..0.72).contains(&h);
            if male_adult && sp.rut.is_some_and(|s| s as usize == season) && dusk_or_dawn {
                60.0
            } else {
                0.0
            }
        }
        (CallWhen::Contact, Some(CallKind::Caw)) if !dark => 30.0,
        (CallWhen::Contact, _) if awake(sp.activity, h) => 10.0,
        (CallWhen::Territory, Some(CallKind::Howl)) if dark => 3.0,
        (CallWhen::Territory, Some(CallKind::Hoot)) if dark => 20.0,
        (CallWhen::Territory, Some(CallKind::Drum)) if !dark && h < 0.5 => 30.0 * singing,
        (CallWhen::Territory, Some(CallKind::Howl | CallKind::Hoot | CallKind::Drum)) => 0.0,
        (CallWhen::Territory, _) if awake(sp.activity, h) => 10.0,
        (CallWhen::Dawn, _) if !dark => {
            let dawn = if (0.2..0.35).contains(&h) { 1.0 } else { 0.2 };
            300.0 * dawn * singing
        }
        (CallWhen::Night, _) if dark => 10.0,
        _ => 0.0,
    }
}

/// The calls of the animals about the player that are not in the world, this step of `dt`
/// seconds: the small birds of the cells within 200 m (the dawn chorus and the day's song,
/// owls by night, woodpeckers drumming in spring), and from farther off (3 km) the packs
/// howling by night, taking it up together, and the stags roaring in the rut, from where
/// their groups are.
pub fn chorus(eco: &Ecology, at: DVec3, now: &Now, dt: f32, rng: &mut Rng) -> Vec<Called> {
    let cat = eco.catalog.clone();
    let hour_s = 3600.0;
    let f = if now.southern {
        (now.year_frac + 0.5).rem_euclid(1.0)
    } else {
        now.year_frac
    };
    let season = ((f * 4.0).floor() as usize).min(3);
    let wrap = eco.wrap_m();
    let around = eco.cells_around;
    let mut out = Vec::new();
    let mut cells = Vec::new();
    for r in eco.regions.values() {
        r.cells_within(around, [at.x, at.z], 200.0, &mut cells);
        for &c in &cells {
            let centre = r.cell_centre(c);
            for (slot, &si) in r.pool_species.iter().enumerate() {
                let sp = &cat.species[si as usize];
                let n = r.adults[slot * REGION_LEN + c];
                if n <= 0.0 || sp.calls.is_empty() {
                    continue;
                }
                for when in [CallWhen::Dawn, CallWhen::Territory, CallWhen::Night] {
                    let Some(call) = call_for(sp, when) else {
                        continue;
                    };
                    // The males sing.
                    let rate = calls_now(sp, when, true, season, now);
                    let k = poisson(rng, n * 0.5 * rate * dt / hour_s);
                    for _ in 0..k {
                        let p = DVec3::new(
                            centre[0] + (rng.next_f64() - 0.5) * 256.0,
                            at.y,
                            centre[1] + (rng.next_f64() - 0.5) * 256.0,
                        );
                        out.push(Called {
                            species: si,
                            call,
                            pos: p,
                        });
                    }
                }
            }
        }
        for g in r.groups.iter().filter(|g| !g.live && g.size() > 0) {
            if dist(g.pos, [at.x, at.z], wrap) > 3000.0 {
                continue;
            }
            let sp = &cat.species[g.species as usize];
            for (when, singers, together) in [
                (CallWhen::Territory, 1, true),
                (CallWhen::Rut, g.males as u32, false),
            ] {
                let Some(call) = call_for(sp, when) else {
                    continue;
                };
                // Only the loud carry so far.
                if singers == 0 || sp.calls[call as usize].loudness_db < 95.0 {
                    continue;
                }
                let rate = calls_now(sp, when, true, season, now);
                let k = poisson(rng, singers as f32 * rate * dt / hour_s);
                for _ in 0..k {
                    let voices = if together { g.adults().max(1) } else { 1 };
                    for _ in 0..voices {
                        let p = DVec3::new(
                            g.pos[0] + (rng.next_f64() - 0.5) * 60.0,
                            at.y,
                            g.pos[1] + (rng.next_f64() - 0.5) * 60.0,
                        );
                        out.push(Called {
                            species: g.species,
                            call,
                            pos: p,
                        });
                    }
                }
            }
        }
    }
    out
}
