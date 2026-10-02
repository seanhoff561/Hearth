//! Animals (v2 §7, V2-7): the species as the simulation reads them, the habitat of each
//! ecological cell, and the populations of the world advanced through the years.

pub mod body;
pub mod ecology;
pub mod habitat;
pub mod live;
pub mod species;

pub use ecology::{Cause, Ecology, Group, Region};
pub use habitat::{Habitat, Land};
pub use species::{Catalog, Forage, Species};
