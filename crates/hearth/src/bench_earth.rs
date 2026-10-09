//! The Earth-scale benchmarks (E4.1 §3), headless on the Earth-sized planet:
//! - `hearth bench globe`: the globe's map as it is made (rows of it, the time for all of it
//!   projected from them), hovering over random points (the line about the place under the
//!   cursor), clicking (the start spot found, then the place's card) and the places suggested;
//! - `hearth bench creator`: the character creator's preview while sliders are dragged, a change
//!   a frame: the main thread's time a frame and how many frames the person shown lags behind;
//! - `hearth bench load`: from "Play" to the player in control and on to the whole render
//!   distance, a new world (with the menus' work before it still running, as the game leaves
//!   it) and then its save: each stage's time, the main thread's frames, memory at its peak, the
//!   CPU each thread used.
//!
//! Each prints the fine terrain tiles built and for whom (`hearth_core::prof`), and `--json FILE`
//! writes its numbers for the perf gate.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use hearth_core::prof;
use hearth_math::PlanetSize;
use hearth_render::GpuContext;
use hearth_worldgen::{PlanetGrid, Terrain, WorldGenSettings};

pub const HELP: &str = "\
hearth bench globe|creator|load — the Earth-scale benchmarks (E4.1)

USAGE:
    hearth bench globe   [--seed N] [--points N] [--clicks N] [--budget S] [--json FILE]
    hearth bench creator [--software] [--frames N] [--json FILE]
    hearth bench load    [--seed N] [--software] [--limit S] [--size WxH] [--render-distance N]
                         [--lod N] [--no-menus] [--fly S] [--speed M/S] [--json FILE]
    hearth bench earth-judge --baseline FILE[,FILE...] --candidate FILE[,FILE...] [--gate PCT]

    globe    the planet's map made from its grid and read back from where it is kept, --points
             random points hovered (default 1000) and --clicks clicks (default 10), each within
             --budget seconds (default 60), and the places suggested
    creator  sliders dragged on the creator's preview, a change a frame for --frames frames
             each (default 60): the main thread's time a frame, how far the person shown lags
    load     from Play to the player in control and to the whole render distance, a new world
             and then its save, within --limit seconds each (default 300); --no-menus leaves out
             the menus' work that runs on behind a new world (the globe's map, the places);
             --fly S flies the new world's player across the planet for S seconds after it
             loads (at --speed, default 60 m/s), the memory looked at each minute
    earth-judge  the perf gate's verdict on saved runs of the three (E4.1 §5): each gated
             number's median against the baseline's, no more than PCT % (default 5) worse
             beyond a small floor, and no tile of the finest level built for a coarse caller
    --seed N         the planet's seed (default 1)
    --software       the software (CPU) adapter
    --json FILE      also write the numbers as JSON";

/// What the benchmarks are told.
#[derive(Debug, Clone)]
struct Opts {
    seed: u64,
    points: usize,
    clicks: usize,
    budget: f64,
    frames: usize,
    software: bool,
    limit: f64,
    size: (u32, u32),
    render_distance: Option<u32>,
    lod: Option<u32>,
    menus: bool,
    /// After the new world has loaded: flown across the planet this long (s), the memory
    /// looked at each minute (the soak of E4.1 §4.7).
    fly: f64,
    /// The flight's speed (m/s).
    speed: f64,
    json: Option<PathBuf>,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            seed: 1,
            points: 1000,
            clicks: 10,
            budget: 60.0,
            frames: 60,
            software: false,
            limit: 300.0,
            size: (1280, 720),
            render_distance: None,
            lod: None,
            menus: true,
            fly: 0.0,
            speed: 60.0,
            json: None,
        }
    }
}

impl Opts {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut o = Self::default();
        let mut it = args.iter();
        let num = |v: Option<&String>, name: &str| -> Result<f64, String> {
            v.and_then(|s| s.parse::<f64>().ok())
                .ok_or_else(|| format!("{name} needs a number"))
        };
        while let Some(a) = it.next() {
            match a.as_str() {
                "--seed" => o.seed = num(it.next(), a)? as u64,
                "--points" => o.points = num(it.next(), a)? as usize,
                "--clicks" => o.clicks = num(it.next(), a)? as usize,
                "--budget" => o.budget = num(it.next(), a)?,
                "--frames" => o.frames = (num(it.next(), a)? as usize).max(2),
                "--limit" => o.limit = num(it.next(), a)?,
                "--render-distance" => o.render_distance = Some(num(it.next(), a)? as u32),
                "--lod" => o.lod = Some(num(it.next(), a)? as u32),
                "--fly" => o.fly = num(it.next(), a)?,
                "--speed" => o.speed = num(it.next(), a)?,
                "--software" => o.software = true,
                "--no-menus" => o.menus = false,
                "--size" => {
                    let s = it.next().ok_or("--size needs WxH")?;
                    let (w, h) = s.split_once('x').ok_or("--size needs WxH")?;
                    o.size = (
                        w.parse().map_err(|_| "--size needs WxH")?,
                        h.parse().map_err(|_| "--size needs WxH")?,
                    );
                }
                "--json" => o.json = Some(PathBuf::from(it.next().ok_or("--json needs a file")?)),
                other => return Err(format!("unknown argument {other:?}")),
            }
        }
        Ok(o)
    }
}

/// The numbers measured, printed as they come.
#[derive(Default)]
struct Report {
    rows: Vec<(String, f64)>,
}

impl Report {
    fn add(&mut self, name: &str, value: f64, unit: &str) {
        println!("  {name:<52} {value:>12.3} {unit}");
        self.rows.push((name.to_owned(), value));
    }

    fn section(&self, title: &str) {
        println!("\n{title}");
    }

    /// The fine terrain tiles built since `before`, by level and caller.
    fn tiles(&mut self, what: &str, before: &[(String, u64)]) {
        let now = tile_counts();
        let mut any = false;
        for (name, n) in &now {
            let was = before
                .iter()
                .find(|(b, _)| b == name)
                .map_or(0, |(_, v)| *v);
            if *n > was {
                any = true;
                self.add(&format!("{what}: {name}"), (*n - was) as f64, "tiles");
            }
        }
        if !any {
            self.add(&format!("{what}: refinement tiles built"), 0.0, "tiles");
        }
    }

    fn write_json(&self, path: &Path, kind: &str) -> anyhow::Result<()> {
        let map: serde_json::Map<String, serde_json::Value> = self
            .rows
            .iter()
            .map(|(k, v)| (format!("{kind}: {k}"), serde_json::json!(v)))
            .collect();
        std::fs::write(path, serde_json::to_string_pretty(&map)?)?;
        println!("\nwritten to {}", path.display());
        Ok(())
    }
}

/// The refinement tiles built so far, by level and caller (`relief.tile.<level>.<caller>`).
fn tile_counts() -> Vec<(String, u64)> {
    prof::counters()
        .into_iter()
        .filter(|(k, _)| k.starts_with("relief.tile."))
        .collect()
}

/// The batches of cubes the server has streamed so far.
fn stream_batches() -> u64 {
    prof::zones()
        .into_iter()
        .find(|(k, _)| *k == "stream.batch")
        .map_or(0, |(_, z)| z.count)
}

/// Peak resident memory, watched on a thread of its own (every 50 ms) between resets.
struct Watch {
    peak: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Watch {
    fn start() -> Self {
        let peak = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (p, s) = (peak.clone(), stop.clone());
        let thread = std::thread::Builder::new()
            .name("bench watch".into())
            .spawn(move || {
                while !s.load(Ordering::Relaxed) {
                    if let Some((now, _)) = prof::memory() {
                        p.fetch_max(now, Ordering::Relaxed);
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            })
            .ok();
        Self { peak, stop, thread }
    }

    /// The peak since the last reset (MiB), and a new watch from now.
    fn take(&self) -> f64 {
        let now = prof::memory().map_or(0, |m| m.0);
        let peak = self.peak.swap(now, Ordering::Relaxed).max(now);
        peak as f64 / (1u64 << 20) as f64
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// CPU seconds by thread group (threads named alike summed: `lod-3` into `lod`).
fn cpu_by_group() -> Vec<(String, f64)> {
    let mut groups: Vec<(String, f64)> = Vec::new();
    for (name, s) in prof::thread_cpu() {
        let g = name
            .trim_end_matches(|c: char| c.is_ascii_digit())
            .trim_end_matches(['-', ' '])
            .to_owned();
        match groups.iter_mut().find(|(n, _)| *n == g) {
            Some(e) => e.1 += s,
            None => groups.push((g, s)),
        }
    }
    groups.sort_by(|a, b| b.1.total_cmp(&a.1));
    groups
}

fn cpu_since(before: &[(String, f64)]) -> Vec<(String, f64)> {
    cpu_by_group()
        .into_iter()
        .map(|(n, s)| {
            let was = before
                .iter()
                .find(|(b, _)| *b == n)
                .map_or(0.0, |(_, v)| *v);
            (n, s - was)
        })
        .filter(|(_, s)| *s > 0.05)
        .collect()
}

/// Milliseconds at a quantile of sorted durations.
fn quantile_ms(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let i = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1u64 << 20) as f64
}

/// The soak (E4.1 §4.7): the player carried eastward across the planet at `o.speed` for
/// `o.fly` seconds, high over the ground, the world streaming about them; the resident memory
/// and its largest kinds looked at each minute.
fn fly(
    r: &mut Report,
    client: &mut crate::client::Client,
    ctx: &GpuContext,
    target: &hearth_render::offscreen::OffscreenTarget,
    o: &Opts,
) -> anyhow::Result<()> {
    r.section(&format!(
        "Flying {:.0} minutes at {:.0} m/s",
        o.fly / 60.0,
        o.speed
    ));
    let format = hearth_render::offscreen::OFFSCREEN_FORMAT;
    let (w, h) = o.size;
    let mut input =
        hearth_input::InputState::new(hearth_input::ActionRegistry::with_builtins().len());
    let pad = crate::gamepad::Pad::default();
    let start = client.mover.pos;
    let t0 = Instant::now();
    let mut last = Instant::now();
    let mut minute = 0;
    let mut first: Option<u64> = None;
    let mut peak = 0u64;
    let dt = 1.0 / 60.0;
    while t0.elapsed().as_secs_f64() < o.fly {
        let f0 = Instant::now();
        let flown = t0.elapsed().as_secs_f64() * o.speed;
        let at = glam::DVec3::new(start.x + flown, start.y + 60.0, start.z);
        client.mover.pos = at;
        client.mover.vel = glam::DVec3::ZERO;
        client.pump(ctx);
        client.update(dt, &mut input, None, 1.0, &pad, 1.0);
        client.mover.pos = at;
        let mut enc = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        client.render(
            ctx,
            &mut enc,
            hearth_render::FrameTargets {
                color: &target.color_view,
                depth: &target.depth.view,
                size: (w, h),
                format,
            },
            dt as f32,
        );
        ctx.queue.submit([enc.finish()]);
        let _ = ctx.device.poll(wgpu::PollType::Poll);
        if last.elapsed() >= Duration::from_secs(60) {
            last = Instant::now();
            minute += 1;
            let resident = prof::memory().map_or(0, |m| m.0);
            first.get_or_insert(resident);
            peak = peak.max(resident);
            let parts: Vec<String> = client
                .memory()
                .iter()
                .take(7)
                .map(|(k, b)| format!("{k} {:.0}", mib(*b)))
                .collect();
            let heap = prof::heap().map_or_else(String::new, |(used, free)| {
                format!("; heap {:.0} in use, {:.0} free", mib(used), mib(free))
            });
            // Meshes the server has sent the client not yet taken (the channel's backlog, at most
            // its window), and those it holds until the client takes those.
            let unread =
                prof::counter("net.meshes.sent").saturating_sub(prof::counter("net.meshes.taken"));
            let outbox = prof::gauges()
                .into_iter()
                .find(|(k, _)| *k == "meshes waiting (server)")
                .map_or(0, |(_, b)| b);
            let heap = format!(
                "{heap}; {unread} meshes unread, {:.0} MiB waiting to be sent",
                mib(outbox)
            );
            println!(
                "  minute {minute:>2}: {:>6.0} MiB resident, {:.0} km flown ({}){heap}",
                mib(resident),
                flown / 1000.0,
                parts.join(", ")
            );
        }
        if let Some(wait) = Duration::from_secs_f64(dt).checked_sub(f0.elapsed()) {
            std::thread::sleep(wait);
        }
    }
    let end = prof::memory().map_or(0, |m| m.0);
    r.add("fly: minutes flown", minute as f64, "min");
    r.add(
        "fly: km flown",
        t0.elapsed().as_secs_f64() * o.speed / 1000.0,
        "km",
    );
    r.add(
        "fly: resident after the first minute",
        mib(first.unwrap_or(end)),
        "MiB",
    );
    r.add("fly: resident at the end", mib(end), "MiB");
    r.add("fly: resident at the most", mib(peak.max(end)), "MiB");
    Ok(())
}

/// Runs `hearth bench globe|creator|load`.
pub fn run(what: &str, args: &[String], cache_dir: Option<&Path>) -> i32 {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{HELP}");
        return 0;
    }
    let opts = match Opts::parse(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}\n\n{HELP}");
            return 2;
        }
    };
    // The game's pools (named, so their CPU time is told apart).
    hearth_core::jobs::init(0, 0);
    let _main = prof::caller("main");
    let (cores, ram) = prof::machine();
    println!(
        "{} {} · bench {what} · {cores} cores, {} RAM",
        hearth_core::GAME_NAME,
        hearth_core::GAME_VERSION,
        ram.map_or("? GB".into(), |r| format!("{:.1} GB", r as f64 / 1e9))
    );
    let result = match what {
        "globe" => globe(&opts, cache_dir),
        "creator" => creator(&opts),
        "load" => load(&opts, cache_dir),
        _ => Err(anyhow::anyhow!("unknown benchmark {what}")),
    };
    if result.is_ok() {
        println!("\nThe longest zones (all threads, summed)");
        for (name, z) in prof::zones().iter().take(16) {
            println!(
                "  {name:<28} {:>8} runs {:>10.2} s in all {:>9.1} ms at most",
                z.count,
                z.total.as_secs_f64(),
                ms(z.max)
            );
        }
    }
    match result.and_then(|r| match &opts.json {
        Some(p) => r.write_json(p, what),
        None => Ok(()),
    }) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("benchmark failed: {e:#}");
            2
        }
    }
}

/// The Earth-sized planet's settings for a seed.
fn earth(seed: u64) -> WorldGenSettings {
    WorldGenSettings {
        seed,
        planet_size: PlanetSize::Earth,
        ..Default::default()
    }
    .sanitized()
}

/// The planet's grid as the game has it for a new world: from the cache, else built (and
/// cached).
fn planet_grid(settings: &WorldGenSettings, cache_dir: Option<&Path>) -> PlanetGrid {
    crate::scene::planet_for(settings, cache_dir, None, &|_, _| {})
}

/// The places' finder as the menus make it for a new world (`App::make_planet`).
fn finder(terrain: &Arc<Terrain>) -> anyhow::Result<Arc<crate::places::Finder>> {
    let (content, report) = hearth_content::Content::load(&[crate::scene::data_pack_dir()]);
    let content = Arc::new(content.ok_or_else(|| {
        anyhow::anyhow!(
            "game data failed to load: {} problems",
            report.sorted().len()
        )
    })?);
    let defs = hearth_world::datapack::load_block_defs(&[crate::scene::data_pack_dir()])?;
    let reg = Arc::new(hearth_world::BlockRegistry::build(defs)?);
    let wg = hearth_worldgen::WorldGenerator::new(terrain.clone(), &reg, &content)?;
    Ok(Arc::new(crate::places::Finder::new(
        Arc::new(wg),
        content,
        reg,
    )))
}

/// The share of a globe map's area that is land (water is drawn in its biome's colour,
/// unshaded; a texel's area goes with the cosine of its latitude).
fn land_share(map: &[[u8; 4]], terrain: &Terrain) -> f64 {
    let water: Vec<[u8; 3]> = hearth_worldgen::region::biome::Biome::ALL
        .iter()
        .filter(|b| b.is_water())
        .map(|b| b.color())
        .collect();
    let w = crate::globe::MAP_WIDTH;
    let h = w / 2;
    let planet = terrain.planet();
    let edge = planet.latitude(-planet.pole_edge_z() + 1.0);
    let (mut land, mut all) = (0.0, 0.0);
    for (y, row) in map.chunks_exact(w).enumerate().take(h) {
        let lat = std::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / h as f64 * std::f64::consts::PI;
        if lat.abs() > edge {
            continue;
        }
        for px in row {
            all += lat.cos();
            if !water.contains(&[px[0], px[1], px[2]]) {
                land += lat.cos();
            }
        }
    }
    land / all.max(1e-9)
}

/// A point on the sphere, evenly over its area: (latitude, longitude) in radians.
fn random_point(rng: &mut hearth_math::hash::Rng) -> (f32, f32) {
    let u = rng.next_f32() * 2.0 - 1.0;
    let lon = (rng.next_f32() * 2.0 - 1.0) * std::f32::consts::PI;
    (u.asin(), lon)
}

fn globe(o: &Opts, cache_dir: Option<&Path>) -> anyhow::Result<Report> {
    let mut r = Report::default();
    let watch = Watch::start();
    let settings = earth(o.seed);
    r.section("The planet (as the menus make it for a new world)");
    let t = Instant::now();
    let grid = planet_grid(&settings, cache_dir);
    r.add("planet grid ready", t.elapsed().as_secs_f64(), "s");
    let t = Instant::now();
    let terrain = Arc::new(Terrain::new(Arc::new(grid)));
    r.add(
        "terrain prepared (rivers, realms, levels)",
        t.elapsed().as_secs_f64(),
        "s",
    );

    // The map as the menus make it with the planet (`globe::cached_map`): from the grid, then
    // read back from where it is kept.
    r.section("The globe's map (globe::planet_map, from the planet grid)");
    let before = tile_counts();
    let cpu0 = cpu_by_group();
    let kept = std::env::temp_dir().join(format!("hearth-bench-globe-{}.zst", std::process::id()));
    let _ = std::fs::remove_file(&kept);
    let t = Instant::now();
    let map = crate::globe::cached_map(&terrain, crate::globe::MAP_WIDTH, Some(&kept));
    r.add(
        "map: made from the grid (and kept)",
        t.elapsed().as_secs_f64(),
        "s",
    );
    let t = Instant::now();
    let again = crate::globe::cached_map(&terrain, crate::globe::MAP_WIDTH, Some(&kept));
    r.add(
        "map: read back from where it is kept",
        t.elapsed().as_secs_f64(),
        "s",
    );
    let _ = std::fs::remove_file(&kept);
    anyhow::ensure!(again == map, "the map kept is the map made");
    r.add("map: land by area", 100.0 * land_share(&map, &terrain), "%");
    r.tiles("map", &before);
    for (g, s) in cpu_since(&cpu0) {
        r.add(&format!("map: CPU of {g}"), s, "s");
    }
    r.add("map: peak resident", watch.take(), "MiB");

    // Hovering: the line about the place under the cursor, each frame it moves.
    r.section("Hovering (globe::describe at random points)");
    let mut rng = hearth_math::hash::Rng::new(o.seed ^ 0x4807e5);
    let before = tile_counts();
    let mut times = Vec::new();
    let t = Instant::now();
    for _ in 0..o.points {
        let (lat, lon) = random_point(&mut rng);
        let t0 = Instant::now();
        let line = {
            let _c = prof::caller("globe.hover");
            crate::globe::describe(&terrain, lat, lon)
        };
        times.push(ms(t0.elapsed()));
        std::hint::black_box(line);
        if t.elapsed().as_secs_f64() > o.budget {
            break;
        }
    }
    let n = times.len();
    times.sort_by(f64::total_cmp);
    r.add("hover: points hovered in the budget", n as f64, "points");
    r.add("hover: median", quantile_ms(&times, 0.5), "ms");
    r.add("hover: 95th percentile", quantile_ms(&times, 0.95), "ms");
    r.add("hover: slowest", quantile_ms(&times, 1.0), "ms");
    r.tiles("hover", &before);
    r.add("hover: peak resident", watch.take(), "MiB");

    // Clicking: the start spot (spawn_near), then the place's card (Finder::verify), both on
    // the interface's thread as the Birthplace screen does them.
    r.section("Clicking (spawn_near, then the place's card)");
    let finder = finder(&terrain)?;
    let before = tile_counts();
    let (mut spot, mut card) = (Vec::new(), Vec::new());
    let t = Instant::now();
    for _ in 0..o.clicks {
        let (lat, lon) = random_point(&mut rng);
        let (x, z) = crate::globe::world_xz(terrain.planet(), lat, lon);
        let t0 = Instant::now();
        let (x, z) = {
            let _c = prof::caller("globe.click.spot");
            terrain.spawn_near(x, z)
        };
        spot.push(ms(t0.elapsed()));
        let t1 = Instant::now();
        let place = {
            let _c = prof::caller("globe.click.card");
            finder.verify(x, z, crate::places::When::Spring)
        };
        card.push(ms(t1.elapsed()));
        std::hint::black_box(place);
        if t.elapsed().as_secs_f64() > o.budget {
            break;
        }
    }
    spot.sort_by(f64::total_cmp);
    card.sort_by(f64::total_cmp);
    r.add("click: clicks in the budget", spot.len() as f64, "clicks");
    r.add("click: start spot, median", quantile_ms(&spot, 0.5), "ms");
    r.add("click: start spot, slowest", quantile_ms(&spot, 1.0), "ms");
    r.add("click: the card, median", quantile_ms(&card, 0.5), "ms");
    r.add("click: the card, slowest", quantile_ms(&card, 1.0), "ms");
    let mut total: Vec<f64> = spot.iter().zip(&card).map(|(a, b)| a + b).collect();
    total.sort_by(f64::total_cmp);
    r.add(
        "click: to the details, median",
        quantile_ms(&total, 0.5),
        "ms",
    );
    r.add(
        "click: to the details, slowest",
        quantile_ms(&total, 1.0),
        "ms",
    );
    r.tiles("click", &before);

    r.section("The places suggested (Finder::suggest)");
    let before = tile_counts();
    let t = Instant::now();
    let places = {
        let _c = prof::caller("places.suggest");
        finder.suggest(crate::places::When::Spring, o.seed)
    };
    r.add("suggest: time", t.elapsed().as_secs_f64(), "s");
    r.add("suggest: places", places.len() as f64, "places");
    r.tiles("suggest", &before);
    r.add("peak resident", watch.take(), "MiB");
    Ok(r)
}

fn creator(o: &Opts) -> anyhow::Result<Report> {
    use hearth_character::person::{Detail, meshes};
    let mut r = Report::default();
    let ctx = GpuContext::headless(o.software).or_else(|_| GpuContext::headless(true))?;
    println!("adapter: {} ({:?})", ctx.info.name, ctx.info.backend);
    r.section("A person's meshes built (hearth_character::person::meshes)");
    let a0 = hearth_character::Appearance::default();
    let worn = ["hearth:loincloth"];
    for (name, d) in [
        ("close (6 mm)", Detail::Close),
        ("near (12 mm)", Detail::Near),
        ("far (24 mm)", Detail::Far),
    ] {
        let t = Instant::now();
        std::hint::black_box(meshes(&a0, &worn, d));
        r.add(&format!("build {name}"), ms(t.elapsed()), "ms");
        let t = Instant::now();
        std::hint::black_box(hearth_character::anatomy::anatomy(&a0, d.cell()));
        r.add(
            &format!("  of which the body {name}"),
            ms(t.elapsed()),
            "ms",
        );
    }
    let t = Instant::now();
    std::hint::black_box(hearth_character::garment::fitted(&a0, &worn));
    r.add("  the garments fitted", ms(t.elapsed()), "ms");
    let t = Instant::now();
    std::hint::black_box(hearth_character::hair::hair(&a0));
    r.add("  the hair", ms(t.elapsed()), "ms");
    let t = Instant::now();
    std::hint::black_box(hearth_character::eyes::eyes(&a0));
    r.add("  the eyes", ms(t.elapsed()), "ms");

    // Sliders dragged as the creator's preview takes them (`App`'s preview: the figure set and
    // dressed, its meshes kept at Close, the frame drawn), a change a frame at 60 Hz.
    let format = hearth_render::offscreen::OFFSCREEN_FORMAT;
    let (w, h) = (960u32, 540u32);
    let target = hearth_render::offscreen::OffscreenTarget::new(&ctx, w, h);
    let mut preview = hearth_render::body::BodyPreview::new(&ctx, format);
    type Set = fn(&mut hearth_character::Appearance, f32);
    let drags: [(&str, Set); 4] = [
        ("height", |a, t| a.height_m = 1.5 + 0.4 * t),
        ("build", |a, t| a.build = t),
        ("skin tone", |a, t| a.skin_tone = t),
        ("hair colour", |a, t| {
            a.hair_color = [(40.0 + 200.0 * t) as u8, (30.0 + 120.0 * t) as u8, 20]
        }),
    ];
    let dt = 1.0 / 60.0;
    for (name, set) in drags {
        r.section(&format!("Dragging {name}"));
        let mut a = a0.clone();
        let mut fig = hearth_character::Figure::starting(a.clone());
        let mut person = crate::people::Person::new(1);
        // Built once before the drag, as when the screen opens.
        person.keep(&ctx, &fig, Detail::Close, true);
        let drive = hearth_character::Drive {
            breaths_per_min: 12.0,
            ..Default::default()
        };
        let mut history: Vec<hearth_character::Appearance> = Vec::new();
        let (mut frame_ms, mut lags) = (Vec::new(), Vec::new());
        let start = Instant::now();
        for f in 0..o.frames {
            let tick = start + Duration::from_secs_f64(f as f64 * dt);
            if let Some(wait) = tick.checked_duration_since(Instant::now()) {
                std::thread::sleep(wait);
            }
            let t0 = Instant::now();
            set(&mut a, f as f32 / (o.frames - 1) as f32);
            history.push(a.clone());
            let pose = {
                hearth_core::zone!("creator.figure");
                fig.set_appearance(&a);
                fig.dress(&hearth_character::starting_garbs(&a));
                fig.animator.update(&fig.rig, &drive, dt as f32)
            };
            {
                hearth_core::zone!("creator.keep");
                person.keep(&ctx, &fig, Detail::Close, false);
            }
            let z = prof::Zone::new("creator.frame");
            let drawn = person.frame(
                &fig,
                &pose,
                glam::Affine3A::IDENTITY,
                hearth_render::body::SkinState::default(),
                dt as f32,
                glam::Vec3::ZERO,
                [1.0, 0.0],
                false,
            );
            drop(z);
            let z = prof::Zone::new("creator.render");
            let mut enc = ctx
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            if let Some((m, frame)) = &drawn {
                preview.render(
                    &ctx,
                    &mut enc,
                    &target.color_view,
                    (w, h),
                    [w / 3, 0, w / 3, h],
                    m,
                    frame,
                    (
                        glam::Vec3::new(0.0, 1.05, 4.0),
                        glam::Vec3::new(0.0, 0.98, 0.0),
                        28.0,
                    ),
                    hearth_render::figure::PreviewLight::Daylight,
                );
            }
            drop(z);
            {
                hearth_core::zone!("creator.submit");
                ctx.queue.submit([enc.finish()]);
            }
            frame_ms.push(ms(t0.elapsed()));
            // Frames behind: since the settings whose shape the person shown has (its colours
            // are drawn as they are).
            let lag = history
                .iter()
                .rev()
                .position(|h| person.shows(h).is_some())
                .unwrap_or(history.len());
            lags.push(lag as f64);
        }
        let t_end = Instant::now();
        let mut first = None;
        while person.shows(&a) != Some(Detail::Close) && t_end.elapsed() < Duration::from_secs(30) {
            person.keep(&ctx, &fig, Detail::Close, false);
            if first.is_none() && person.shows(&a).is_some() {
                first = Some(ms(t_end.elapsed()));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let settle = ms(t_end.elapsed());
        let first = first.unwrap_or(settle);
        let _ = ctx.device.poll(wgpu::PollType::wait_indefinitely());
        frame_ms.sort_by(f64::total_cmp);
        lags.sort_by(f64::total_cmp);
        r.add(
            &format!("{name}: main thread a frame, median"),
            quantile_ms(&frame_ms, 0.5),
            "ms",
        );
        r.add(
            &format!("{name}: main thread a frame, slowest"),
            quantile_ms(&frame_ms, 1.0),
            "ms",
        );
        r.add(
            &format!("{name}: frames behind, median"),
            quantile_ms(&lags, 0.5),
            "frames",
        );
        r.add(
            &format!("{name}: frames behind, most"),
            quantile_ms(&lags, 1.0),
            "frames",
        );
        r.add(
            &format!("{name}: the last setting shown after"),
            first,
            "ms",
        );
        r.add(&format!("{name}: at full detail after"), settle, "ms");
    }
    Ok(r)
}

/// The menus' work for a new world that runs on behind it (as the game leaves it): the
/// places' finder having suggested places, and the globe's map being made on its own thread.
fn menus_behind(
    seed: u64,
    cache_dir: Option<&Path>,
) -> anyhow::Result<std::thread::JoinHandle<()>> {
    let grid = planet_grid(&earth(seed), cache_dir);
    let terrain = Arc::new(Terrain::new(Arc::new(grid)));
    let finder = finder(&terrain)?;
    let t = Instant::now();
    let places = finder.suggest(crate::places::When::Spring, seed);
    println!(
        "  (the menus: {} places suggested in {:.1} s; the globe's map now being made)",
        places.len(),
        t.elapsed().as_secs_f64()
    );
    Ok(std::thread::Builder::new()
        .name("globe map".into())
        .spawn(move || {
            let _c = prof::caller("globe.map");
            std::hint::black_box(crate::globe::planet_map(&terrain, crate::globe::MAP_WIDTH));
        })?)
}

fn load(o: &Opts, cache_dir: Option<&Path>) -> anyhow::Result<Report> {
    let mut r = Report::default();
    let watch = Watch::start();
    let ctx = GpuContext::headless(o.software).or_else(|_| GpuContext::headless(true))?;
    println!("adapter: {} ({:?})", ctx.info.name, ctx.info.backend);
    let dir = std::env::temp_dir().join(format!("hearth-bench-load-{}", std::process::id()));
    let saves = dir.join("saves");
    std::fs::create_dir_all(&saves)?;
    let (content, _) = hearth_content::Content::load(&[crate::scene::data_pack_dir()]);
    let content = content.ok_or_else(|| anyhow::anyhow!("game data failed to load"))?;
    let mut options = hearth_core::options::Options::default();
    if let Some(d) = o.render_distance {
        options.video.render_distance = d;
    }
    if let Some(l) = o.lod {
        options.video.lod_distance = l;
    }
    println!(
        "render distance {} chunks, LOD {} chunks, {}x{}",
        options.video.render_distance, options.video.lod_distance, o.size.0, o.size.1
    );
    let spec = crate::server::WorldSpec {
        name: "bench".into(),
        seed: o.seed,
        planet: PlanetSize::Earth,
        cache_dir: cache_dir.map(Path::to_path_buf),
        saves_dir: Some(saves),
        appearance: Default::default(),
        knowledge: Default::default(),
        era: "wild_earth".into(),
        shape: Default::default(),
        birthplace: None,
        mode: Some("realistic".into()),
        prepared: None,
    };
    let menus = if o.menus {
        r.section("The menus' work behind a new world");
        Some(menus_behind(o.seed, cache_dir)?)
    } else {
        None
    };
    for label in ["new world", "save"] {
        r.section(&format!("Loading: {label}"));
        let _ = watch.take();
        load_once(&mut r, label, &ctx, &spec, &options, &content, o)?;
        r.add(&format!("{label}: peak resident"), watch.take(), "MiB");
    }
    // The map thread runs until it is done (it is not stopped in the game either).
    drop(menus);
    let _ = std::fs::remove_dir_all(&dir);
    Ok(r)
}

fn load_once(
    r: &mut Report,
    label: &str,
    ctx: &GpuContext,
    spec: &crate::server::WorldSpec,
    options: &hearth_core::options::Options,
    content: &hearth_content::Content,
    o: &Opts,
) -> anyhow::Result<()> {
    let format = hearth_render::offscreen::OFFSCREEN_FORMAT;
    let (w, h) = o.size;
    let target = hearth_render::offscreen::OffscreenTarget::new(ctx, w, h);
    let before = tile_counts();
    let cpu0 = cpu_by_group();
    let (sent0, batches0) = (prof::counter("net.meshes.sent"), stream_batches());
    let t0 = Instant::now();
    let mut client = crate::client::Client::new(spec.clone(), options, format, Some(content));
    client.apply_options(options);
    let mut input =
        hearth_input::InputState::new(hearth_input::ActionRegistry::with_builtins().len());
    let pad = crate::gamepad::Pad::default();
    let (mut stage, mut stage_at) = (String::new(), 0.0f64);
    let mut stages: Vec<(String, f64)> = Vec::new();
    let mut frame_ms = Vec::new();
    let (mut ready, mut control, mut full) = (None, None, None);
    let dt = 1.0 / 60.0;
    loop {
        let f0 = Instant::now();
        {
            hearth_core::zone!("frame.pump");
            client.pump(ctx);
        }
        {
            hearth_core::zone!("frame.update");
            client.update(dt, &mut input, None, 1.0, &pad, 1.0);
        }
        let mut enc = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            hearth_core::zone!("frame.render");
            client.render(
                ctx,
                &mut enc,
                hearth_render::FrameTargets {
                    color: &target.color_view,
                    depth: &target.depth.view,
                    size: (w, h),
                    format,
                },
                dt as f32,
            );
        }
        {
            hearth_core::zone!("frame.submit");
            ctx.queue.submit([enc.finish()]);
            let _ = ctx.device.poll(wgpu::PollType::Poll);
        }
        frame_ms.push(ms(f0.elapsed()));
        let at = t0.elapsed().as_secs_f64();
        let l = client.loading();
        if l.stage != stage {
            if !stage.is_empty() {
                stages.push((std::mem::take(&mut stage), at - stage_at));
            }
            stage = l.stage.clone();
            stage_at = at;
        }
        if l.failed {
            anyhow::bail!("the world failed: {}", l.stage);
        }
        if l.ready && ready.is_none() {
            ready = Some(at);
        }
        if l.ground && control.is_none() {
            control = Some(at);
        }
        if control.is_some() && l.near.0 >= l.near.1 && l.lod.1 > 0 && l.lod.0 >= l.lod.1 {
            full = Some(at);
            break;
        }
        if at > o.limit {
            println!(
                "  stopped at the limit: cubes {}/{}, distant tiles {}/{}",
                l.near.0, l.near.1, l.lod.0, l.lod.1
            );
            break;
        }
        if let Some(wait) = Duration::from_secs_f64(dt).checked_sub(f0.elapsed()) {
            std::thread::sleep(wait);
        }
    }
    if !stage.is_empty() {
        stages.push((stage, t0.elapsed().as_secs_f64() - stage_at));
    }
    for (s, d) in &stages {
        r.add(&format!("{label}: stage \"{s}\""), *d, "s");
    }
    let never = o.limit;
    r.add(
        &format!("{label}: Play to the world shown"),
        ready.unwrap_or(never),
        "s",
    );
    r.add(
        &format!("{label}: Play to the player in control"),
        control.unwrap_or(never),
        "s",
    );
    r.add(
        &format!("{label}: Play to the whole render distance"),
        full.unwrap_or(never),
        "s",
    );
    frame_ms.sort_by(f64::total_cmp);
    r.add(
        &format!("{label}: main thread a frame, median"),
        quantile_ms(&frame_ms, 0.5),
        "ms",
    );
    r.add(
        &format!("{label}: main thread a frame, 95th"),
        quantile_ms(&frame_ms, 0.95),
        "ms",
    );
    r.add(
        &format!("{label}: main thread a frame, slowest"),
        quantile_ms(&frame_ms, 1.0),
        "ms",
    );
    // The meshes the server sent, and how many a batch of cubes made: a batch making more than
    // the client may hold unread waits a batch longer for the rest (D287).
    let sent = prof::counter("net.meshes.sent").saturating_sub(sent0);
    let batches = stream_batches().saturating_sub(batches0);
    r.add(&format!("{label}: meshes sent"), sent as f64, "meshes");
    r.add(
        &format!("{label}: meshes a batch of cubes made"),
        sent as f64 / batches.max(1) as f64,
        "meshes",
    );
    r.tiles(label, &before);
    for (g, s) in cpu_since(&cpu0) {
        r.add(&format!("{label}: CPU of {g}"), s, "s");
    }
    if ready.is_none() {
        // Its thread never got to the world: dropping it would wait on it for ever.
        println!("  the server's thread is stuck making the world: left running");
        Box::leak(Box::new(client));
        anyhow::bail!("{label}: the world never arrived within {} s", o.limit);
    }
    for (kind, bytes) in client.memory() {
        r.add(&format!("{label}: memory, {kind}"), mib(bytes), "MiB");
    }
    if o.fly > 0.0 && label == "new world" {
        fly(r, &mut client, ctx, &target, o)?;
    }
    let t = Instant::now();
    drop(client);
    r.add(
        &format!("{label}: saved and closed"),
        t.elapsed().as_secs_f64(),
        "s",
    );
    Ok(())
}

/// The numbers the perf gate holds (E4.1 §5), lower being better: each key's suffix as the
/// benchmarks write it, and the floor below which a difference is noise.
const GATED: [(&str, f64); 12] = [
    ("globe: map: made from the grid (and kept)", 0.05),
    ("globe: hover: median", 0.5),
    ("globe: click: to the details, median", 20.0),
    ("creator: build close (6 mm)", 20.0),
    ("creator: height: the last setting shown after", 20.0),
    ("load: new world: Play to the player in control", 0.2),
    ("load: save: Play to the player in control", 0.2),
    ("load: new world: Play to the whole render distance", 1.0),
    ("load: new world: peak resident", 32.0),
    ("load: save: peak resident", 32.0),
    ("load: new world: main thread a frame, median", 0.5),
    ("load: new world: main thread a frame, 95th", 5.0),
];

/// The perf gate's verdict on saved runs (`--json` of `globe`, `creator` and `load`): false if
/// a gated number's median is more than `pct` per cent (and its floor) worse than the
/// baseline's, or a tile of the finest level was built for a coarse caller.
pub fn judge(base: &[PathBuf], new: &[PathBuf], pct: f64) -> anyhow::Result<bool> {
    let read =
        |paths: &[PathBuf]| -> anyhow::Result<Vec<serde_json::Map<String, serde_json::Value>>> {
            paths
                .iter()
                .map(|p| {
                    let text = std::fs::read_to_string(p)
                        .map_err(|e| anyhow::anyhow!("{}: {e}", p.display()))?;
                    serde_json::from_str(&text).map_err(|e| anyhow::anyhow!("{}: {e}", p.display()))
                })
                .collect()
        };
    let (b, n) = (read(base)?, read(new)?);
    let median = |runs: &[serde_json::Map<String, serde_json::Value>], key: &str| {
        let mut v: Vec<f64> = runs
            .iter()
            .filter_map(|r| r.get(key).and_then(serde_json::Value::as_f64))
            .collect();
        v.sort_by(f64::total_cmp);
        (!v.is_empty()).then(|| v[v.len() / 2])
    };
    let mut ok = true;
    println!(
        "{} runs against {} of the baseline, limit +{pct} %:",
        n.len(),
        b.len()
    );
    for (key, floor) in GATED {
        let (Some(was), Some(now)) = (median(&b, key), median(&n, key)) else {
            continue;
        };
        let limit = was * (1.0 + pct / 100.0) + floor;
        let pass = now <= limit;
        ok &= pass;
        println!(
            "  {} {key}: {now:.3} against {was:.3} (limit {limit:.3})",
            if pass { "ok  " } else { "FAIL" }
        );
    }
    // The guard (E4.1 §4.1): no tile of the finest level for a caller working at a coarse scale.
    for run in &n {
        for (key, v) in run {
            if key.contains("relief.fine.") && v.as_f64().is_some_and(|x| x > 0.0) {
                ok = false;
                println!("  FAIL {key}: {v} tiles of the finest level built for a coarse caller");
            }
        }
    }
    Ok(ok)
}
