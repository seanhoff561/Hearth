//! V2-9, the vertical slice: a year in the temperate forest from a loincloth in spring. The
//! V2-5 bot (`bot`) gets fire, a spear, clothing and dried meat by discovery; then, before the
//! cold, it learns the windbreak by piling brush, comes to the lean-to, and puts one up over
//! its bed — hazel posts, a ridge pole, a bough roof — with brush walls at its ends; then it
//! lives day by day through the autumn and the winter to the spring a year on: water, food
//! (a roe deer dies near the camp every day or two, as in V2-5), the fire, firewood, sleep.
//!
//! It is not part of the check: run it with `scripts/slice-year.sh` (some minutes). Each day
//! goes to `bench-out/slice/year.log`; copies of the world at its moments go to
//! `bench-out/slice/<moment>/` for screenshots (`--screenshot save=...`).

mod bot;
mod common;

use std::io::Write;
use std::path::{Path, PathBuf};

use bot::Bot;
use common::temp;
use glam::DVec3;
use hearth_math::{BlockPos, Direction};
use hearth_protocol::AimAt;

/// Where the year's log and moments go.
fn out_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench-out/slice")
}

/// The year's log: a line a day, and what happened.
struct Log(std::fs::File);

impl Log {
    fn line(&mut self, bot: &Bot, s: &str) {
        let day = bot.w.ticks as f64 / bot.w.ticks_per_day;
        let _ = writeln!(self.0, "[day {day:5.2}] {s}");
        bot.say(s);
    }

    /// How the day ended: the season, the weather, the body, the stores, what is known.
    fn day(&mut self, bot: &Bot) {
        let season = format!("{:?}", season_of(bot)).to_lowercase();
        let (core, air, rain, wind, food, water) =
            bot.w
                .body
                .as_ref()
                .map_or((0.0, 0.0, 0.0, 0.0, String::new(), String::new()), |b| {
                    (
                        b.status.core_c,
                        b.exposure.air_c,
                        b.exposure.rain_mm_h,
                        b.exposure.wind_m_s,
                        format!("{:?}", b.status.hunger),
                        format!("{:?}", b.status.thirst),
                    )
                });
        let line = format!(
            "{season}: air {air:.0} °C, rain {rain:.1} mm/h, wind {wind:.1} m/s; core {core:.1} °C, \
             {food}, {water}; fire {:?}, fuel {}; meat {} cooked, {} raw, {} dried; {} eaten so \
             far; {} known",
            bot.fire_state(),
            bot.fuel(),
            bot.count("cooked_meat"),
            bot.count("cut/meat"),
            bot.count("dried_meat"),
            bot.eaten,
            bot.w.knowledge.known.len(),
        );
        self.line(bot, &line);
    }
}

/// The season where the camp is (by its hemisphere).
fn season_of(bot: &Bot) -> hearth_content::schema::Season {
    let southern = bot.w.generator.planet().latitude(bot.camp.z) < 0.0;
    bot.w.calendar.at(bot.w.ticks).season(southern)
}

/// Keeps a copy of the world as it is now, for screenshots.
fn keep(bot: &mut Bot, dir: &Path, name: &str, log: &mut Log) {
    let to = out_dir().join(name);
    bot.w.keep(dir, &to);
    log.line(bot, &format!("MOMENT {name} kept"));
}

/// Open ground with room above it at a place beside camp.
fn open(bot: &mut Bot, dx: i32, dz: i32) -> bool {
    let g = bot.site(dx, dz);
    bot.w.solid(g)
        && !bot.w.solid(g.up())
        && !bot.w.solid(g.up().up())
        && !bot.w.solid(g.up().up().up())
}

/// Gathers sticks and twigs, and poles, until there are `sticks`, `twigs` and `poles` at camp.
fn stock_wood(bot: &mut Bot, sticks: u32, twigs: u32, poles: u32) {
    let wood = |_: &str, m: Option<&str>| m.is_some_and(|m| m.ends_with("_wood"));
    for _ in 0..12 {
        let (s, t, p) = (bot.count("stick/"), bot.count("twig/"), bot.count("pole/"));
        if s >= sticks && t >= twigs && p >= poles {
            return;
        }
        if s < sticks || t < twigs {
            bot.gather(48, wood, "break_deadwood", 10);
        }
        if p < poles {
            bot.gather(48, wood, "pull_dead_pole", 4);
        }
        bot.upkeep();
    }
}

/// Puts up a piece at camp (what it takes being at camp), as the person faces `yaw`.
fn put_up(bot: &mut Bot, process: &str, aim: AimAt, yaw: f32) -> bool {
    bot.home();
    bot.w.turn(yaw);
    let (done, words) = bot.w.act(process, aim);
    if !done {
        bot.say(&format!("{process}: {words}"));
    }
    done
}

/// The shelter before the cold: the windbreak learned by piling brush (its ends), the lean-to
/// come to, and one put up over the bed — posts two high at the ends of a ridge behind it, the
/// ridge pole, a bough roof sloping down over the bed.
fn shelter(bot: &mut Bot, log: &mut Log) -> bool {
    let south = 0.0_f32;
    let east = std::f32::consts::FRAC_PI_2;
    // The windbreak: brush piled at the ends of where the lean-to will be and about it, until
    // the windbreak is known and the lean-to has come to mind.
    for (dx, dz) in [(-1, 1), (3, 1), (-1, 2), (3, 2), (-1, 0), (3, 0)] {
        if bot.knows("windbreak_shelter") && bot.knows("lean_to") {
            break;
        }
        if !open(bot, dx, dz) {
            continue;
        }
        stock_wood(bot, 24, 0, 0);
        let g = bot.site(dx, dz);
        put_up(
            bot,
            "place_brush_wall",
            AimAt::Block { pos: g, top: true },
            east,
        );
    }
    log.line(
        bot,
        &format!("windbreak known: {}", bot.knows("windbreak_shelter")),
    );
    // The lean-to comes of knowing the windbreak (by inference, in time).
    for _ in 0..12 {
        if bot.knows("lean_to") {
            break;
        }
        bot.pass_time(6.0);
    }
    if !bot.knows("lean_to") {
        log.line(bot, "the lean-to did not come to mind");
        return false;
    }
    log.line(bot, "knows the lean-to");
    // The ridge behind the bed: posts two high at either end, the pole between (where the
    // ground is, found before anything stands on it).
    for dx in [0, 2] {
        if !open(bot, dx, 2) {
            log.line(bot, &format!("no room for a post at {dx}, 2"));
            return false;
        }
    }
    let ground = [bot.site(0, 2), bot.site(2, 2)];
    stock_wood(bot, 0, 0, 5);
    for g in ground {
        for up in 0..2 {
            let below = BlockPos::new(g.x, g.y + up, g.z);
            if !put_up(
                bot,
                "place_post",
                AimAt::Block {
                    pos: below,
                    top: true,
                },
                south,
            ) {
                return false;
            }
        }
    }
    let post = BlockPos::new(ground[0].x, ground[0].y + 2, ground[0].z);
    if !put_up(
        bot,
        "place_beam",
        AimAt::Beside {
            pos: post,
            face: Direction::East,
        },
        east,
    ) {
        return false;
    }
    // The bough roof, sloping down from the ridge over the bed.
    for dx in 0..3 {
        stock_wood(bot, 20, 40, 2);
        let ridge = BlockPos::new(post.x + dx, post.y, post.z);
        if !put_up(
            bot,
            "place_brush_roof",
            AimAt::Beside {
                pos: ridge,
                face: Direction::North,
            },
            south,
        ) {
            return false;
        }
    }
    bot.w.run(20);
    log.line(bot, "a lean-to over the bed");
    true
}

/// A hide cape of two scraped hides tied with cord, put on.
fn cape(bot: &mut Bot, log: &mut Log) {
    let (made, worn) = bot::goals_cape(bot);
    log.line(bot, &format!("a hide cape: made {made}, worn {worn}"));
}

/// A day of living at camp: a drink in the morning; fresh meat when it runs low (a kill
/// butchered) and some of it cooked; wood stocked for the night; the rest of the day and the
/// night passed by the fire (the bot's upkeep eats, drinks and keeps the fire meanwhile).
fn a_day(bot: &mut Bot) {
    let end = bot.w.ticks + bot.w.ticks_per_day as u64;
    bot.drink_up(true);
    if bot.fresh("cut/meat") < 4 && bot.knows("butchery") {
        bot.butcher_one();
    }
    bot.cook(4);
    bot.firewood();
    let left = end.saturating_sub(bot.w.ticks) as f64 / bot.w.ticks_per_day * 24.0;
    bot.pass_time(left.max(1.0));
}

#[test]
#[ignore]
fn a_year_from_a_loincloth() {
    let _ = std::fs::create_dir_all(out_dir());
    let mut log = Log(std::fs::File::create(out_dir().join("year.log")).expect("the year's log"));
    let dir = temp("slice-year");
    let mut bot = bot::start(&dir, 7);
    bot.strict = false;
    let begun = bot.w.ticks;
    let year = (bot.w.calendar.days_per_season * 4) as f64 * bot.w.ticks_per_day;
    log.line(
        &bot,
        &format!("a loincloth in spring; camp at {:?}", bot.camp),
    );
    keep(&mut bot, &dir, "00-start", &mut log);
    // What survival needs first, by discovery (the V2-5 bot's phases): fire and a bed, stone
    // tools, butchery; hides and cord.
    bot::goals_fire(&mut bot);
    bot::goals_tools(&mut bot);
    bot::goals_butchery(&mut bot);
    log.day(&bot);
    keep(&mut bot, &dir, "01-fire-and-food", &mut log);
    bot::goals_hides(&mut bot);
    bot::goals_cord(&mut bot);
    // A hide cape against the nights.
    cape(&mut bot, &mut log);
    log.day(&bot);
    keep(&mut bot, &dir, "02-cape", &mut log);
    // The shelter before the cold.
    let sheltered = shelter(&mut bot, &mut log);
    log.day(&bot);
    keep(&mut bot, &dir, "03-shelter", &mut log);
    // Day by day to the spring a year on.
    let mut last_season = usize::MAX;
    while ((bot.w.ticks - begun) as f64) < year {
        a_day(&mut bot);
        log.day(&bot);
        let season = season_of(&bot) as usize;
        if season != last_season {
            last_season = season;
            let name = format!("{:?}", season_of(&bot)).to_lowercase();
            let n = 4 + season;
            keep(&mut bot, &dir, &format!("{n:02}-{name}"), &mut log);
        }
        if bot.w.body.as_ref().is_some_and(|b| b.dead.is_some()) {
            break;
        }
    }
    let dead = bot.w.body.as_ref().and_then(|b| b.dead.clone());
    log.line(
        &bot,
        &format!(
            "a year on: {} (sheltered {sheltered}); learned {:?}",
            match &dead {
                Some(d) => format!("died of {d:?}"),
                None => "alive".to_owned(),
            },
            bot.w.learned
        ),
    );
    keep(&mut bot, &dir, "09-end", &mut log);
    let feet = bot.w.mover.pos;
    log.line(
        &bot,
        &format!("ends at {:.1}, {:.1}, {:.1}", feet.x, feet.y, feet.z),
    );
    let _ = DVec3::ZERO;
    assert!(dead.is_none(), "died: {dead:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
