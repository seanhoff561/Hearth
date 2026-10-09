//! The minimal globe spawn picker (v2 §16; finished with the world-creation screens in V2-15):
//! the world-map key opens the planet as a globe; dragging turns it, the wheel zooms, the place
//! under the cursor is described in the window title, and a click goes there — from the sea or
//! a lake, to the nearest coast.
//!
//! All of it at the scale it is seen at (E4.1 §4.2): the map and the line about the place
//! under the cursor come from the planet grid alone (`Terrain::grid_column`), the map made with
//! the planet and kept beside its cache; a click's details are looked for on a thread of their
//! own ([`Details`]), the globe turning on meanwhile.

use std::f64::consts::{FRAC_PI_2, PI};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;

use glam::{DVec3, Vec2};
use hearth_math::Planet;
use hearth_render::GpuContext;
use hearth_render::globe::{GlobeRenderer, GlobeView};
use hearth_worldgen::Terrain;

/// Width of the globe's map in texels (twice its height): its grid's own cells, about 20 km on
/// the Earth, 32 blocks on the standard planet.
pub const MAP_WIDTH: usize = 2048;

/// Width of its relief (heights) in texels: twice the map's, about 10 km a texel on the Earth,
/// bicubic between the grid's cells (Amendment T §2.4).
pub const RELIEF_WIDTH: usize = 2 * MAP_WIDTH;

/// The map's file format (its first bytes: `HGLB` and this), bumped when the map is drawn
/// otherwise.
const MAP_FORMAT: u32 = 3;

/// The globe's map (Amendment T §2.4): equirectangular, rows from the north pole to the south,
/// columns eastward from longitude 0, from the planet grid alone; its heights at twice the
/// resolution of the rest. The shader colours and lights it (relief, hypsometric tints, the
/// sea's depths, rivers, ice; by biome, by climate or as plain relief).
#[derive(Debug, Clone, PartialEq)]
pub struct GlobeMap {
    pub width: usize,
    pub height: usize,
    /// The planet's radius (m).
    pub radius_m: f32,
    /// The ground's colour by its biome (sRGB) and its water (alpha: 0 land, 128 a lake, 255
    /// the sea).
    pub color: Vec<[u8; 4]>,
    /// The river through it (its discharge, logarithmic: 2 m³/s nought, 20,000 the most), ice
    /// (255 an ice sheet or glacier, 180 the polar sea's pack), the mean temperature (−40 to
    /// 40 °C) and the rain (the square root of its share of 4,000 mm a year).
    pub facts: Vec<[u8; 4]>,
    /// The surface's height, or under water its floor's (m), as half floats, `relief_width`
    /// by half that.
    pub elevation: Vec<u16>,
    pub relief_width: usize,
}

impl GlobeMap {
    /// The map as the renderer takes it.
    pub fn layers(&self) -> hearth_render::globe::MapLayers<'_> {
        hearth_render::globe::MapLayers {
            width: self.width as u32,
            height: self.height as u32,
            color: &self.color,
            facts: &self.facts,
            relief_width: self.relief_width as u32,
            elevation: &self.elevation,
        }
    }

    /// Whether a texel is under water (the sea or a lake).
    pub fn wet(&self, i: usize) -> bool {
        self.color[i][3] >= 64
    }

    /// The surface's height at a texel of the relief (m).
    pub fn elevation_m(&self, i: usize) -> f32 {
        f16_to_f32(self.elevation[i])
    }
}

/// A half float's value.
pub fn f16_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = ((h >> 10) & 0x1f) as i32;
    let m = (h & 0x3ff) as f32;
    sign * match e {
        0 => m * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    }
}

/// The planet's map from its grid alone. A few seconds on the reference machine (E4.1 §4.2:
/// made once, with the planet, and kept).
pub fn planet_map(terrain: &Terrain, width: usize) -> GlobeMap {
    use hearth_worldgen::region::biome::Biome;
    use rayon::prelude::*;
    let _zone = hearth_core::prof::Zone::new("globe.map");
    let planet = terrain.planet();
    let edge = planet.latitude(-planet.pole_edge_z() + 1.0);
    let c = planet.circumference_f64();
    // A texel's centre: its world (x, z), rows of `w` by `w / 2`.
    let centre = move |w: usize, x: usize, y: usize| {
        let h = w / 2;
        let lat = (FRAC_PI_2 - (y as f64 + 0.5) / h as f64 * PI).clamp(-edge, edge);
        (
            (x as f64 + 0.5) / w as f64 * c,
            planet.z_for_latitude(lat) + 0.5,
        )
    };
    let height = width / 2;
    let rows: Vec<Vec<([u8; 4], [u8; 4])>> = (0..height)
        .into_par_iter()
        .map(|y| {
            let _c = hearth_core::prof::caller("globe.map");
            (0..width)
                .map(|x| {
                    let (wx, wz) = centre(width, x, y);
                    let s = terrain.grid_column(wx, wz);
                    let wet = s.is_underwater();
                    let sea = wet && s.water <= 0.5;
                    let [r, g, b] = s.biome.color();
                    let water = match (wet, sea) {
                        (false, _) => 0,
                        (true, false) => 128,
                        (true, true) => 255,
                    };
                    let unit = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                    let river = if s.discharge > 2.0 {
                        unit((s.discharge / 2.0).log10() / 4.0)
                    } else {
                        0
                    };
                    let ice = match s.biome {
                        Biome::IceSheet | Biome::Glacier => 255,
                        Biome::PolarSea => 180,
                        _ => 0,
                    };
                    let facts = [
                        river,
                        ice,
                        unit((s.temperature + 40.0) / 80.0),
                        unit((s.precipitation.max(0.0) / 4000.0).sqrt()),
                    ];
                    ([r, g, b, water], facts)
                })
                .collect()
        })
        .collect();
    // The relief at twice the resolution: the grid's surface, bicubic (`level_height(0)` at
    // each texel's centre), each column's weights found once.
    let rw = 2 * width;
    let zs: Vec<f64> = (0..rw / 2).map(|y| centre(rw, 0, y).1).collect();
    let elevation: Vec<u16> = terrain
        .grid_heights_around(rw, &zs)
        .par_iter()
        .map(|&h| to_f16(h))
        .collect();
    let (color, facts) = rows.into_iter().flatten().unzip();
    GlobeMap {
        width,
        height,
        radius_m: planet.radius() as f32,
        color,
        facts,
        elevation,
        relief_width: rw,
    }
}

/// A half float (IEEE 754 binary16) from a single, rounded to nearest.
pub fn to_f16(x: f32) -> u16 {
    hearth_render::skin_lut::to_f16(x)
}

/// The planet's map as kept at `path` (beside the planet's cache), or made from the grid and
/// kept there (written on a thread of its own, the map not waiting on the disk; a file whole
/// or none, renamed into place): the same map for the same planet every time it is asked for.
pub fn cached_map(terrain: &Terrain, width: usize, path: Option<&Path>) -> GlobeMap {
    if let Some(map) = path.and_then(|p| load_map(p, width)) {
        return map;
    }
    let map = planet_map(terrain, width);
    if let Some(p) = path {
        let (p, kept) = (p.to_path_buf(), map.clone());
        let spawned = std::thread::Builder::new()
            .name("globe map kept".into())
            .spawn(move || {
                if let Err(e) = save_map(&p, &kept) {
                    log::warn!("could not keep the globe's map: {e}");
                }
            });
        if let Err(e) = spawned {
            log::warn!("could not keep the globe's map: {e}");
        }
    }
    map
}

// The kept map: a header of five little-endian words (`HGLB`, the format, its width, its
// height, the planet's radius as a single's bits), then four parts each packed alone (zstd)
// after its packed length (a word): the colours, the facts, and the relief's northern and
// southern halves, its heights little-endian. The parts unpack in parallel straight into the
// map's own buffers: a third of the time of one part unpacked and then copied out (E4.1 §4.2:
// read back within 0.1 s).
fn load_map(path: &Path, width: usize) -> Option<GlobeMap> {
    use rayon::prelude::*;
    let height = width / 2;
    let n = width * height;
    let file = std::fs::read(path).ok()?;
    let (head, mut rest) = file.split_at_checked(20)?;
    let word = |b: &[u8], i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let fits = &head[..4] == b"HGLB"
        && word(head, 4) == MAP_FORMAT
        && word(head, 8) as usize == width
        && word(head, 12) as usize == height;
    if !fits {
        return None;
    }
    let mut parts = Vec::with_capacity(4);
    for _ in 0..4 {
        let (len, after) = rest.split_at_checked(4)?;
        let (part, after) = after.split_at_checked(word(len, 0) as usize)?;
        parts.push(part);
        rest = after;
    }
    let mut color = vec![[0u8; 4]; n];
    let mut facts = vec![[0u8; 4]; n];
    let mut elevation = vec![0u16; 4 * n];
    let (north, south) = elevation.split_at_mut(2 * n);
    let into: [&mut [u8]; 4] = [
        bytemuck::cast_slice_mut(&mut color),
        bytemuck::cast_slice_mut(&mut facts),
        bytemuck::cast_slice_mut(north),
        bytemuck::cast_slice_mut(south),
    ];
    let whole = parts.into_par_iter().zip(into).all(|(part, to)| {
        zstd::bulk::decompress_to_buffer(part, to).is_ok_and(|got| got == to.len())
    });
    if !rest.is_empty() || !whole {
        return None;
    }
    for h in &mut elevation {
        *h = u16::from_le(*h);
    }
    Some(GlobeMap {
        width,
        height,
        radius_m: f32::from_bits(word(head, 16)),
        color,
        facts,
        elevation,
        relief_width: 2 * width,
    })
}

fn save_map(path: &Path, map: &GlobeMap) -> std::io::Result<()> {
    let mut bytes = b"HGLB".to_vec();
    for v in [
        MAP_FORMAT,
        map.width as u32,
        map.height as u32,
        map.radius_m.to_bits(),
    ] {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    let heights: Vec<u8> = map.elevation.iter().flat_map(|h| h.to_le_bytes()).collect();
    let (north, south) = heights.split_at(heights.len() / 2);
    let parts: [&[u8]; 4] = [
        bytemuck::cast_slice(&map.color),
        bytemuck::cast_slice(&map.facts),
        north,
        south,
    ];
    for part in parts {
        let packed = zstd::bulk::compress(part, 3)?;
        bytes.extend_from_slice(&(packed.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&packed);
    }
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let part = path.with_extension("part");
    std::fs::write(&part, bytes)?;
    std::fs::rename(&part, path)
}

/// How near the globe is seen (its zoom) before its relief is drawn finer, from the 2.4 km
/// refinement level (Amendment T §2.4), where the window's texels come within three of that
/// level's cells (a few hundred of its tiles at most, kept once made).
pub const DETAIL_ZOOM: f32 = 10.0;

/// Texels a side of the finer relief's window.
pub const DETAIL_SIZE: usize = 512;

/// A window of the planet's relief finer than the map's, for the globe seen near: its edges
/// (radians: south, north, west, east) and its heights, `DETAIL_SIZE` square, rows from the
/// north, as half floats.
#[derive(Debug, Clone, PartialEq)]
pub struct Detail {
    pub window: [f32; 4],
    pub elevation: Vec<u16>,
}

impl Detail {
    /// The refinement level a window's texels read, if finer than the grid's: the 2.4 km
    /// level's where the texels are within three of its cells (E4.1's coarse callers read no
    /// finer than their scale).
    pub fn level_for(terrain: &Terrain, window: [f32; 4]) -> Option<usize> {
        let [south, north, west, east] = window.map(|v| v as f64);
        let span = (east - west) / DETAIL_SIZE as f64
            * terrain.planet().radius()
            * (0.5 * (south + north)).cos().max(0.05);
        (terrain.finest_level() >= 1 && span <= 3.0 * terrain.level_cell(1)).then_some(1)
    }

    /// The relief over `window` at `level`, every core of the interactive pool at it.
    pub fn make(terrain: &Terrain, window: [f32; 4], level: usize) -> Self {
        use rayon::prelude::*;
        let _zone = hearth_core::prof::Zone::new("globe.detail");
        let planet = terrain.planet();
        let [south, north, west, east] = window.map(|v| v as f64);
        let n = DETAIL_SIZE;
        let elevation = (0..n)
            .into_par_iter()
            .flat_map_iter(|j| {
                let lat = north - (j as f64 + 0.5) / n as f64 * (north - south);
                let z = planet.z_for_latitude(lat);
                (0..n).map(move |i| {
                    let lon = west + (i as f64 + 0.5) / n as f64 * (east - west);
                    to_f16(terrain.level_height(level, planet.x_for_longitude(lon), z))
                })
            })
            .collect();
        Self { window, elevation }
    }

    /// The window a view near enough shows (radians: south, north, west, east), with a margin:
    /// the frame's corners from the globe's centre.
    pub fn window_for(view: &GlobeView, size: (u32, u32)) -> [f32; 4] {
        let r = view.radius_px(size);
        let half_px = 0.5 * (size.0 as f32).hypot(size.1 as f32);
        let half = ((half_px / r).min(1.0).asin() * 1.1).min(0.6);
        let (lat, lon) = (view.lat, view.lon);
        let wide = half / lat.cos().max(0.2);
        [
            (lat - half).max(-1.5),
            (lat + half).min(1.5),
            lon - wide,
            lon + wide,
        ]
    }
}

/// The finer relief for the view, made on a thread of its own as the view settles near (E4.1:
/// never on the interface's thread), the latest wanted kept, given up when the view draws back.
#[derive(Default)]
pub struct DetailMaker {
    making: Option<JoinHandle<Detail>>,
    /// The window shown, and the zoom it was made for.
    shown: Option<([f32; 4], f32)>,
}

impl DetailMaker {
    /// A change to the finer relief shown, if there is one: a window made (`Some(Some)`), or
    /// none any more (`Some(None)`, the view drawn back). Asks for the view's window when it is
    /// near and has left the inner part of the window shown, or zoomed well in or out of it.
    pub fn update(
        &mut self,
        terrain: &Arc<Terrain>,
        view: &GlobeView,
        size: (u32, u32),
    ) -> Option<Option<Detail>> {
        if self.making.as_ref().is_some_and(JoinHandle::is_finished) {
            let made = self.making.take().and_then(|h| h.join().ok());
            if let Some(d) = made
                && view.zoom >= DETAIL_ZOOM
            {
                self.shown = Some((d.window, view.zoom));
                return Some(Some(d));
            }
        }
        if view.zoom < DETAIL_ZOOM {
            return self.shown.take().map(|_| None);
        }
        if self.making.is_some() {
            return None;
        }
        let stale = self.shown.is_none_or(|([s, n, w, e], zoom)| {
            let (lat, lon) = (view.lat, view.lon);
            let inner = |lo: f32, hi: f32, v: f32| {
                let m = 0.3 * (hi - lo);
                v > lo + m && v < hi - m
            };
            let lon = lon
                + std::f32::consts::TAU * ((0.5 * (w + e) - lon) / std::f32::consts::TAU).round();
            !inner(s, n, lat) || !inner(w, e, lon) || !(0.67..1.5).contains(&(view.zoom / zoom))
        });
        let window = Detail::window_for(view, size);
        if let (true, Some(level)) = (stale, Detail::level_for(terrain, window)) {
            let terrain = terrain.clone();
            match std::thread::Builder::new()
                .name("globe detail".into())
                .spawn(move || {
                    hearth_core::jobs::install(hearth_core::jobs::Priority::Interactive, || {
                        Detail::make(&terrain, window, level)
                    })
                }) {
                Ok(h) => self.making = Some(h),
                Err(e) => log::error!("could not start the globe's detail: {e}"),
            }
        }
        None
    }
}

/// The world column at a latitude and longitude (radians), within the pole edges.
pub fn world_xz(planet: &Planet, lat: f32, lon: f32) -> (i32, i32) {
    let edge = planet.pole_edge_z() - 1.0;
    let z = planet.z_for_latitude(lat as f64).clamp(-edge, edge);
    (planet.x_for_longitude(lon as f64) as i32, z as i32)
}

/// Latitude and longitude (radians) of a world position.
pub fn lat_lon(planet: &Planet, pos: DVec3) -> (f32, f32) {
    (
        planet.latitude(pos.z) as f32,
        planet.longitude(pos.x) as f32,
    )
}

/// One line about a place: where it is, its climate, biome, height (or depth of water) and
/// yearly temperature and rain.
pub fn describe(terrain: &Terrain, lat: f32, lon: f32) -> String {
    let (x, z) = world_xz(terrain.planet(), lat, lon);
    // As the globe shows it: from the planet grid (E4.1 §4.2).
    let s = terrain.grid_column(x as f64 + 0.5, z as f64 + 0.5);
    let per_m = terrain.vertical_scale() as f64;
    let relief = if s.is_underwater() {
        format!("{:.0} m of water", (s.water - s.height) as f64 / per_m)
    } else {
        format!("{:.0} m", s.height as f64 / per_m)
    };
    format!(
        "{:.1}° {} {:.1}° {} | {} {} | {} | {relief} | {:.0} °C, {:.0} mm a year",
        lat.abs().to_degrees(),
        if lat >= 0.0 { "N" } else { "S" },
        lon.abs().to_degrees(),
        if lon >= 0.0 { "E" } else { "W" },
        s.climate.code(),
        s.climate.name(),
        s.biome.name().replace('_', " "),
        s.temperature,
        s.precipitation
    )
}

/// Where the camera starts for a point of the globe: the nearest dry, gentle land (from the
/// sea, the nearest coast), a little above it.
pub fn start_at(terrain: &Terrain, lat: f32, lon: f32) -> DVec3 {
    let (x, z) = world_xz(terrain.planet(), lat, lon);
    let (x, z) = terrain.spawn_near(x, z);
    let s = terrain.sample(x, z);
    DVec3::new(
        x as f64 + 0.5,
        s.height.max(s.water) as f64 + 12.0,
        z as f64 + 0.5,
    )
}

/// What a click on the globe found: the place's card, or none where it has no fresh water near.
pub type Found = Option<crate::places::Place>;

/// The details of the place last clicked on the globe (E4.1 §4.2), looked for on a thread of
/// their own while the globe turns on: the start spot, then the place's card. Only the last
/// click's are wanted (an earlier search gives up at its next step and its answer is thrown
/// away), and each answer is kept for its place (about a kilometre), so a place clicked again is
/// answered at once.
#[derive(Default)]
pub struct Details {
    /// The click being looked into, and the way its answer comes back.
    pending: Option<((f32, f32), mpsc::Receiver<Found>)>,
    /// Counts the clicks: a search that is not the last click's gives up.
    asked: Arc<AtomicU64>,
    /// The answers found, by place.
    kept: rustc_hash::FxHashMap<(i32, i32), Found>,
}

impl Details {
    /// Looks into the place at `at` (latitude, longitude in radians): the answer at once if it
    /// was found before, else none until [`Self::poll`] gives it.
    pub fn ask(
        &mut self,
        finder: &Arc<crate::places::Finder>,
        at: (f32, f32),
        when: crate::places::When,
    ) -> Option<Found> {
        let n = self.asked.fetch_add(1, Ordering::Relaxed) + 1;
        if let Some(found) = self.kept.get(&Self::key(at)) {
            self.pending = None;
            return Some(found.clone());
        }
        let (tx, rx) = mpsc::channel();
        let (finder, asked) = (finder.clone(), self.asked.clone());
        let spawned = std::thread::Builder::new()
            .name("place details".into())
            .spawn(move || {
                let _c = hearth_core::prof::caller("globe.click");
                let wg = &finder.wg;
                let (x, z) = world_xz(wg.planet(), at.0, at.1);
                let (x, z) = wg.terrain.spawn_near(x, z);
                if asked.load(Ordering::Relaxed) == n {
                    let found = hearth_core::jobs::install(
                        hearth_core::jobs::Priority::Interactive,
                        || finder.verify(x, z, when),
                    );
                    let _ = tx.send(found);
                }
            });
        match spawned {
            Ok(_) => self.pending = Some((at, rx)),
            Err(e) => log::error!("could not look into the place: {e}"),
        }
        None
    }

    /// The answer, once it has come: the place clicked and what was found there.
    pub fn poll(&mut self) -> Option<((f32, f32), Found)> {
        let (at, rx) = self.pending.as_ref()?;
        match rx.try_recv() {
            Ok(found) => {
                let at = *at;
                self.pending = None;
                self.kept.insert(Self::key(at), found.clone());
                Some((at, found))
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.pending = None;
                None
            }
        }
    }

    /// Whether a click is being looked into.
    pub fn looking(&self) -> bool {
        self.pending.is_some()
    }

    /// A place's key: its latitude and longitude to a hundredth of a degree.
    fn key((lat, lon): (f32, f32)) -> (i32, i32) {
        (
            (lat.to_degrees() * 100.0).round() as i32,
            (lon.to_degrees() * 100.0).round() as i32,
        )
    }
}

/// The globe's state in the preview.
pub struct GlobePicker {
    pub open: bool,
    pub view: GlobeView,
    renderer: Option<GlobeRenderer>,
    /// The map being made (on its own thread, the first time the globe opens).
    building: Option<JoinHandle<GlobeMap>>,
    /// The cursor (pixels), where the button went down while it is held, and whether it has
    /// moved since (a drag, not a click).
    cursor: Option<Vec2>,
    press: Option<Vec2>,
    dragged: bool,
    /// The frame's size at the last draw.
    size: (u32, u32),
    /// The planet's map as made, to be uploaded when it is ready.
    base: Option<GlobeMap>,
    map_changed: bool,
    /// The planet (for the finer relief when the globe is seen near), and that relief.
    terrain: Option<Arc<Terrain>>,
    detail: DetailMaker,
}

impl Default for GlobePicker {
    fn default() -> Self {
        Self {
            open: false,
            view: GlobeView::default(),
            renderer: None,
            building: None,
            cursor: None,
            press: None,
            dragged: false,
            size: (1280, 720),
            base: None,
            map_changed: false,
            terrain: None,
            detail: DetailMaker::default(),
        }
    }
}

impl GlobePicker {
    /// Opens the globe centred on (lat, lon) and starts making its map if there is none yet.
    pub fn open(&mut self, terrain: &Arc<Terrain>, lat: f32, lon: f32) {
        self.open = true;
        self.terrain = Some(terrain.clone());
        self.view.lat = lat;
        self.view.lon = lon;
        let has_map =
            self.base.is_some() || self.renderer.as_ref().is_some_and(GlobeRenderer::has_map);
        if !has_map && self.building.is_none() {
            let terrain = terrain.clone();
            match std::thread::Builder::new()
                .name("globe map".into())
                .spawn(move || {
                    hearth_core::jobs::install(hearth_core::jobs::Priority::Background, || {
                        planet_map(&terrain, MAP_WIDTH)
                    })
                }) {
                Ok(handle) => self.building = Some(handle),
                Err(e) => log::error!("could not start making the globe's map: {e}"),
            }
        }
    }

    pub fn close(&mut self) {
        self.open = false;
        self.press = None;
    }

    /// The planet's map, made already (with the planet): no map is made when the globe opens.
    pub fn set_map(&mut self, map: GlobeMap) {
        if map.width == MAP_WIDTH && map.color.len() == MAP_WIDTH * MAP_WIDTH / 2 {
            self.base = Some(map);
            self.map_changed = true;
        }
    }

    /// The cursor moved to `px`; turns the globe while the button is held.
    pub fn cursor_moved(&mut self, px: Vec2) {
        if let (Some(prev), Some(press)) = (self.cursor, self.press) {
            self.view.drag(px - prev, self.size);
            if press.distance(px) > 4.0 {
                self.dragged = true;
            }
        }
        self.cursor = Some(px);
    }

    /// The button went down or up; a click (not a drag) gives the point under it.
    pub fn button(&mut self, pressed: bool) -> Option<(f32, f32)> {
        if pressed {
            self.press = self.cursor;
            self.dragged = false;
            return None;
        }
        let press = self.press.take()?;
        if self.dragged {
            return None;
        }
        self.view.pick(press, self.size)
    }

    /// The point under the cursor (latitude, longitude in radians), if it is on the globe.
    pub fn hovered(&self) -> Option<(f32, f32)> {
        self.view.pick(self.cursor?, self.size)
    }

    /// Draws the globe over the frame, with the camera's place marked; uploads the map when it
    /// is ready.
    pub fn render(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        format: wgpu::TextureFormat,
        camera: (f32, f32),
    ) {
        self.size = size;
        let renderer = self
            .renderer
            .get_or_insert_with(|| GlobeRenderer::new(ctx, format));
        if self.building.as_ref().is_some_and(JoinHandle::is_finished) {
            match self.building.take().map(JoinHandle::join) {
                Some(Ok(map)) => {
                    self.base = Some(map);
                    self.map_changed = true;
                }
                _ => log::error!("making the globe's map failed"),
            }
        }
        if self.map_changed
            && let Some(base) = &self.base
        {
            self.map_changed = false;
            renderer.set_map(ctx, &base.layers(), base.radius_m);
        }
        if let Some(t) = &self.terrain
            && let Some(change) = self.detail.update(t, &self.view, size)
        {
            renderer.set_detail(
                ctx,
                change.as_ref().map(|d| hearth_render::globe::DetailLayer {
                    window: d.window,
                    size: DETAIL_SIZE as u32,
                    elevation: &d.elevation,
                }),
            );
        }
        let hovered = self.cursor.and_then(|c| self.view.pick(c, size));
        renderer.render(ctx, enc, target, size, &self.view, Some(camera), hovered);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicks_pick_and_drags_turn() {
        let mut g = GlobePicker::default();
        let centre = Vec2::new(640.0, 360.0);
        g.cursor_moved(centre);
        assert!(g.button(true).is_none());
        let (lat, lon) = g.button(false).expect("a click on the globe");
        assert!(lat.abs() < 1e-5 && lon.abs() < 1e-5);
        // A drag turns the globe and is not a click.
        g.button(true);
        g.cursor_moved(centre + Vec2::new(60.0, 0.0));
        assert!(g.button(false).is_none());
        assert!(g.view.lon < 0.0, "dragging east brings the west into view");
        // Off the globe: nothing.
        g.cursor_moved(Vec2::new(2.0, 2.0));
        g.button(true);
        assert!(g.button(false).is_none());
    }

    #[test]
    fn world_columns_round_trip_through_latitude_and_longitude() {
        let planet = Planet::from_size(hearth_math::PlanetSize::Standard).expect("planet");
        for (x, z) in [(100.0, -2000.0), (65_000.0, 20_000.0), (32_768.0, 0.0)] {
            let (lat, lon) = lat_lon(&planet, DVec3::new(x, 0.0, z));
            let (bx, bz) = world_xz(&planet, lat, lon);
            assert!(
                planet.delta_x(x, bx as f64).abs() < 2.0 && (bz as f64 - z).abs() < 4.0,
                "({x}, {z}) -> ({bx}, {bz})"
            );
        }
        // Past the pole edge: clamped inside the world.
        let (_, z) = world_xz(&planet, 1.55, 0.0);
        assert!((z as f64).abs() < planet.pole_edge_z());
    }
}
