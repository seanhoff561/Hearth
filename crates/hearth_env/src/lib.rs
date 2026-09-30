//! The planet's environment over time (v2 §4): calendar and clock, sun, moon and stars,
//! seasonal climate from the planet's normals, day-scale weather, year-scale snowpack and ice,
//! and vegetation phenology. Everything is a pure function of the world seed, place and time,
//! so the server, the client and headless tools agree.

pub mod astro;
pub mod calendar;
pub mod climate;
pub mod phenology;
pub mod rivers;
pub mod sky;
pub mod tint;
pub mod weather;

pub use calendar::{Calendar, Moment};
pub use climate::{Normals, SeasonalCover};
pub use rivers::RiverRegimes;
pub use weather::{Precip, WeatherModel, WeatherState};
