//! `bench realism weather`: the weather model's year, hour by hour, at real places' climate
//! normals (T §3.1, weather and sky): how often it rains and for how long, how hard, and how the
//! cloud cover is spread between clear and overcast — beside what those places' records show.

use std::fmt::Write as _;

use hearth_env::{Normals, WeatherModel};
use hearth_math::{Planet, PlanetSize};

/// A place by its climate normals (latitude °, mean °C, warmest less coldest month °C, rain
/// mm a year, winter and summer dry seasons 0–1) and what its records say.
struct Place {
    name: &'static str,
    normals: (f64, f64, f64, f64, f64, f64),
    /// Hours with measurable rain (≥ 0.1 mm) in a year, as a share; and overcast hours' share.
    record: &'static str,
}

const PLACES: &[Place] = &[
    Place {
        name: "London",
        normals: (51.5, 11.3, 13.5, 600.0, 0.0, 0.1),
        record: "rain in some 6–8% of hours, mostly 1–4 h at a time; overcast the most common sky, clear skies rare",
    },
    Place {
        name: "Portland, Oregon",
        normals: (45.5, 12.5, 15.5, 1100.0, 0.0, 0.6),
        record: "rain in some 10% of hours (winter 20%, July 2%); winter overcast most days",
    },
    Place {
        name: "Des Moines, Iowa",
        normals: (41.6, 10.3, 30.0, 900.0, 0.3, 0.0),
        record: "rain in some 5–7% of hours, summer storms of 1–2 h; skies spread between clear and overcast",
    },
    Place {
        name: "Las Vegas",
        normals: (36.2, 20.5, 26.0, 110.0, 0.0, 0.3),
        record: "rain in under 1% of hours; clear or nearly so most of the year",
    },
    Place {
        name: "Manaus",
        normals: (-3.1, 27.3, 1.5, 2300.0, 0.3, 0.0),
        record: "showers of an hour or two most afternoons in the wet season; rarely a clear sky",
    },
];

pub fn run() -> anyhow::Result<()> {
    let planet = Planet::from_size(PlanetSize::Earth)?;
    let model = WeatherModel::new(7, planet);
    let mut md = String::from(
        "| Place | Wet hours | Rain spells median · p90 (h) | Rate when wet (mm/h) | Year's rain \
         (mm; normal) | Sky clear · scattered · broken · overcast | The records |\n\
         |---|---|---|---|---|---|---|\n",
    );
    for p in PLACES {
        let (lat, t, range, rain, wd, sd) = p.normals;
        let n = Normals::new(lat, t, range, rain, wd, sd);
        let z = planet.z_for_latitude(lat.to_radians());
        let x = planet.circumference_f64() * 0.3;
        let hours = 365 * 24;
        let (mut wet, mut total, mut run, mut spells) = (0usize, 0.0f64, 0usize, Vec::new());
        let mut sky = [0usize; 4];
        for h in 0..hours {
            let days = h as f64 / 24.0;
            let year = (days / 365.25).rem_euclid(1.0);
            // Local time of day as a fraction of the day.
            let local = (days.fract() + x / planet.circumference_f64()).rem_euclid(1.0);
            let w = model.sample(&n, x, z, days, year, local);
            total += w.precip_mm_h;
            if w.precip_mm_h >= 0.1 {
                wet += 1;
                run += 1;
            } else if run > 0 {
                spells.push(run);
                run = 0;
            }
            let k = match w.cloud_cover {
                c if c < 0.1 => 0,
                c if c < 0.5 => 1,
                c if c < 0.9 => 2,
                _ => 3,
            };
            sky[k] += 1;
        }
        spells.sort_unstable();
        let pick = |q: f64| {
            spells
                .get(((spells.len().max(1) - 1) as f64 * q) as usize)
                .copied()
                .unwrap_or(0)
        };
        let share = |c: usize| 100.0 * c as f64 / hours as f64;
        let _ = writeln!(
            md,
            "| {} | {:.1}% | {} · {} | {:.1} | {:.0} ({:.0}) | {:.0}% · {:.0}% · {:.0}% · {:.0}% | {} |",
            p.name,
            share(wet),
            pick(0.5),
            pick(0.9),
            total / wet.max(1) as f64,
            total,
            rain,
            share(sky[0]),
            share(sky[1]),
            share(sky[2]),
            share(sky[3]),
            p.record
        );
    }
    println!("{md}");
    std::fs::create_dir_all("bench-out/realism")?;
    std::fs::write("bench-out/realism/weather.md", md)?;
    Ok(())
}
