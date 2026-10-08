//! The minimal globe spawn picker (v2 §16; finished with the world-creation screens in V2-15):
//! the world-map key opens the planet as a globe; dragging turns it, the wheel zooms, the place
//! under the cursor is described in the window title, and a click goes there — from the sea or
//! a lake, to the nearest coast.

use std::f64::consts::{FRAC_PI_2, PI};
use std::sync::Arc;
use std::thread::JoinHandle;

use glam::{DVec3, Vec2};
use hearth_math::Planet;
use hearth_render::GpuContext;
use hearth_render::globe::{GlobeRenderer, GlobeView};
use hearth_worldgen::Terrain;

/// Width of the globe's map in texels (twice its height): 32 blocks a texel on the standard
/// planet.
pub const MAP_WIDTH: usize = 2048;

/// An equirectangular map of the planet for the globe: the biome at each texel's point, land
/// shaded by its relief (lit from the north-west); rows from the north pole to the south,
/// columns eastward from longitude 0. Past the world's pole edges (about 85°) the edge goes on.
pub fn planet_map(terrain: &Terrain, width: usize) -> Vec<[u8; 4]> {
    use rayon::prelude::*;
    let planet = terrain.planet();
    let height = width / 2;
    let edge = planet.latitude(-planet.pole_edge_z() + 1.0);
    let lat_of = |y: usize| (FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * PI).clamp(-edge, edge);
    let c = planet.circumference_f64();
    let rows: Vec<Vec<(f32, [u8; 3], bool)>> = (0..height)
        .into_par_iter()
        .map(|y| {
            let z = planet.z_for_latitude(lat_of(y)) as i32;
            (0..width)
                .map(|x| {
                    let s = terrain.sample(((x as f64 + 0.5) / width as f64 * c) as i32, z);
                    (s.height, s.biome.color(), s.is_underwater())
                })
                .collect()
        })
        .collect();
    // World blocks between texel centres: the same east–west everywhere, more north–south
    // toward the poles (the world is a Mercator projection).
    let dx = c / width as f64;
    let mut out = Vec::with_capacity(width * height);
    for (y, row) in rows.iter().enumerate() {
        let dz = planet.radius() / lat_of(y).cos() * PI / height as f64;
        for (x, &(h, color, wet)) in row.iter().enumerate() {
            let shade = if wet {
                1.0
            } else {
                let west = row[(x + width - 1) % width].0;
                let north = rows[y.saturating_sub(1)][x].0;
                let slope = (h - west) as f64 / dx + (h - north) as f64 / dz;
                (1.0 + 1.2 * slope).clamp(0.6, 1.4)
            };
            let [r, g, b] = color.map(|v| (v as f64 * shade).round().min(255.0) as u8);
            out.push([r, g, b, 255]);
        }
    }
    out
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
    let s = terrain.sample(x, z);
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

/// The globe's state in the preview.
pub struct GlobePicker {
    pub open: bool,
    pub view: GlobeView,
    renderer: Option<GlobeRenderer>,
    /// The map being made (on its own thread, the first time the globe opens).
    building: Option<JoinHandle<Vec<[u8; 4]>>>,
    /// The cursor (pixels), where the button went down while it is held, and whether it has
    /// moved since (a drag, not a click).
    cursor: Option<Vec2>,
    press: Option<Vec2>,
    dragged: bool,
    /// The frame's size at the last draw.
    size: (u32, u32),
    /// The planet's map as made, to be uploaded when it is ready.
    base: Option<Vec<[u8; 4]>>,
    map_changed: bool,
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
        }
    }
}

impl GlobePicker {
    /// Opens the globe centred on (lat, lon) and starts making its map if there is none yet.
    pub fn open(&mut self, terrain: &Arc<Terrain>, lat: f32, lon: f32) {
        self.open = true;
        self.view.lat = lat;
        self.view.lon = lon;
        let has_map = self.renderer.as_ref().is_some_and(GlobeRenderer::has_map);
        if !has_map && self.building.is_none() {
            let terrain = terrain.clone();
            match std::thread::Builder::new()
                .name("globe map".into())
                .spawn(move || planet_map(&terrain, MAP_WIDTH))
            {
                Ok(handle) => self.building = Some(handle),
                Err(e) => log::error!("could not start making the globe's map: {e}"),
            }
        }
    }

    pub fn close(&mut self) {
        self.open = false;
        self.press = None;
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
            renderer.set_map(ctx, MAP_WIDTH as u32, (MAP_WIDTH / 2) as u32, base);
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
