//! Making and knowing (v2 §11–12).
//!
//! * [`knowledge`]: the technology graph as a person learns it — insight from discovery
//!   triggers, discoveries and hunches in a journal, skills that grow with practice.
//! * [`engine`]: the process engine — what can be done with what is at hand and what is looked
//!   at, for how long, and what comes of it.
//! * [`fire`]: a thermal model of fires: fuel, flames, coals, heat, rain.
//! * [`food`]: what food gives the body, and spoiling.
//!
//! Everything here is pure: the server applies what it decides, the client lists what is
//! possible with the same code.

pub mod engine;
pub mod fire;
pub mod food;
pub mod knap;
pub mod knowledge;

pub use engine::{
    Aimed, Bench, Crafts, FireSeen, Handy, Lack, Outcome, Plan, Recipe, Source, Surroundings,
};
pub use fire::{Fire, FireState, Fuel};
pub use knowledge::{Event, Graph, KnowledgeState, Mode, Note, NoteKind, Skill};
