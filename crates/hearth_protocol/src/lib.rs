//! Messages between the game's client and its integrated server (v1 M4, v2 V2-3).
//!
//! The server owns the world: it generates, lights and meshes the terrain around the player,
//! keeps the clock, simulates the player's body in the weather and water of where they are, runs
//! the world's own changes and saves. The client renders, takes input and moves the player itself
//! (predicted against its own copy of the nearby blocks), telling the server where the player
//! went and what the movement did (landings, water, breath).
//!
//! In one process the server is a thread and these messages pass over channels carrying shared
//! data as it is (cubes, meshes, the generator). A network transport would serialise the cubes,
//! leave the meshing to the client and send the generator's seed instead of the generator.

use std::sync::Arc;

use glam::DVec3;
use hearth_body::{BodyConfig, Death, Exposure, InjuryState, Status};
use hearth_lod::LodGen;
use hearth_math::{CubePos, Planet};
use hearth_physics::{Ability, Motion, Mover};
use hearth_render::mesh::CubeMesh;
use hearth_render::precip::SkyHeights;
use hearth_render::water::WaterHeights;
use hearth_world::{BlockRegistry, Cube};
use hearth_worldgen::{PlanetGrid, WorldGenerator};

/// From the client.
#[derive(Debug, Clone)]
pub enum ToServer {
    /// Where the player's own movement took them, and what it did since the last report.
    Moved(Moved),
    /// Lie down to sleep (true) or get up.
    Sleep(bool),
    /// Put the player at a place (the globe's choice, a debug move): the server finds solid
    /// ground there.
    Place(DVec3),
    /// Live on as a new person after death (by the world's death rules).
    Respawn,
    /// Debug: move the clock on (or back) by game hours.
    SkipHours(f64),
    /// Debug: extra ticks per second of play (0 for none).
    TimeWarp(f64),
    /// Stop the clock and the body (a single-player menu is open), or go on.
    Pause(bool),
    /// How much terrain to keep around the player (cubes).
    View { radius: i32, vertical: i32 },
    /// Save and stop.
    Quit,
}

/// A report of the player's movement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moved {
    pub mover: Mover,
    /// The hardest landing since the last report (m/s, after cushioning).
    pub landed: Option<f64>,
    pub motion: Motion,
    pub speed: f64,
    pub straining: bool,
    /// Share of the body in water.
    pub immersion: f64,
    /// Seconds without air.
    pub airless_s: f64,
}

/// What the client needs once the world is ready.
pub struct Ready {
    pub planet: Planet,
    pub grid: Arc<PlanetGrid>,
    /// For distant terrain and tints.
    pub generator: Arc<WorldGenerator>,
    pub lod: Arc<LodGen>,
    pub reg: Arc<BlockRegistry>,
    pub body: Arc<BodyConfig>,
    pub vertical_scale: f32,
    /// The world's calendar, the clock and where the player is.
    pub calendar: hearth_env::Calendar,
    pub ticks: u64,
    pub player: Mover,
    /// How the player looks.
    pub appearance: hearth_character::Appearance,
}

/// The player's body as the client shows it and lets it move.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyView {
    pub status: Status,
    pub ability: Ability,
    pub asleep: bool,
    pub dead: Option<Death>,
    pub injuries: Vec<InjuryState>,
    /// Illnesses (content ids) whose symptoms have begun.
    pub illnesses: Vec<String>,
    /// The weather, water and shelter the body is in.
    pub exposure: Exposure,
}

/// From the server.
pub enum ToClient {
    Ready(Box<Ready>),
    /// A cube's blocks and light, for the client's own collision.
    Cube(CubePos, Arc<Cube>),
    Mesh(Box<CubeMesh>),
    Unload(CubePos),
    /// What covers the sky and where the water's surface lies around the player.
    Heights(Box<SkyHeights>, Box<WaterHeights>),
    /// The world clock at the end of a tick.
    Clock(u64),
    Body(Box<BodyView>),
    /// The server put the player somewhere (a place chosen, a new life).
    Placed(Mover),
    /// The world was saved.
    Saved,
    Failed(String),
}
