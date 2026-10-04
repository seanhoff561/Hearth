//! People (V2.1): every hominin and human — and, in its components' types, the player — is a
//! [`Person`]: an id with components (its life history, the player's body physiology at its
//! species' and its own size, a mind, what it knows, its band, what it carries, where it is, its
//! own random stream), within the ranges its [`Species`] profile sets. *Australopithecus* now;
//! the genome and phenotype (H1), the psyche (H2) and the rest of V2.1 join as their milestones
//! land (`docs/design/humans/`).
//!
//! The crate is the people's own: who they are ([`species`], [`person`]), how they choose
//! ([`mind`]), the company they keep ([`band`]), how they work ([`work`]) and what watching them
//! teaches ([`watch`]), the registry that keeps them and lives them ([`sim`]), their saves
//! ([`save`]) and the developer's inspector ([`inspect`]). The world they move in is the game's:
//! it answers what they sense ([`Senses`]) and carries out what they do to it ([`World`]).

pub mod band;
pub mod birth;
pub mod conflict;
pub mod council;
pub mod culture;
pub mod family;
pub mod genome;
pub mod inspect;
pub mod kin;
pub mod language;
pub mod learning;
pub mod life;
pub mod lineage;
pub mod looks;
pub mod memory;
pub mod mind;
pub mod person;
pub mod plan;
pub mod psyche;
pub mod repute;
pub mod save;
pub mod sim;
pub mod species;
pub mod speech;
pub mod strangers;
pub mod ties;
pub mod watch;
pub mod work;
pub mod world;

pub use band::{Band, Culture, Places};
pub use birth::Birth;
pub use conflict::{Quarrel, Rung, Ways, WaysSet};
pub use culture::{CultureValues, Generator, Generators};
pub use family::{Founding, Household, PlayerKin, Sibling};
pub use genome::{Genetics, Genome, Phenotype};
pub use hearth_content::schema::humans::LifeStage;
pub use language::{Language, LanguageDefs, MeaningDef};
pub use learning::{Lore, NodeFacts, Transmission};
pub use lineage::{Kinship, Pedigree};
pub use looks::{Eyes, Look, look};
pub use memory::{Happened, Memory, PlaceKind, Who};
pub use mind::{Doing, Intent, Mind, Needs, Offer, Situation, Stranger, Threat, choose};
pub use person::{
    Cause, Died, Event, LifeEvent, LifeHistory, Person, PersonId, Place, Possessions, Social, Tier,
};
pub use plan::{Plan, Step, Want};
pub use psyche::{Feeling, Psyche, PsycheDefs, Tendency, Value};
pub use save::{FORMAT, PeopleSave};
pub use sim::{Done, Numbers, People, PersonView};
pub use species::{Species, SpeciesSet, body_of, grown_height_m};
pub use speech::{Act, Gesture, Heard, Said, Tongue, Word};
pub use strangers::Guest;
pub use watch::{WATCH_M, scatter_near, seen, watched};
pub use work::{Pile, REACH_M, Things, finish_work, plan_work};
pub use world::{FoodHere, Now, PlayerId, PlayerSeen, Senses, World};
