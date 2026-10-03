//! Agents (V2-11, v2 §8.4): beings that live as the player lives — a body with the player's
//! physiology at their own size and in their own coat, a mind with needs, what it senses and
//! remembers and a choice of what to do next, what they know (the player's knowledge state:
//! nodes, insight and skills), what they carry (the player's carrying), the group they belong to
//! and their group's ways — and make things by the same processes as the player, through the same
//! engine. Hominins (*Australopithecus*) now; people later (v2 §17).
//!
//! The crate is the agents' own: who they are ([`Kind`], [`Agent`]), how they choose
//! ([`mind`]), the company they keep ([`SocialGroup`]) and how they work ([`work`]). The world
//! they move in, and when they do what, is the game's (it tells their minds what is about them and
//! carries out what they choose).

pub mod agent;
pub mod group;
pub mod kind;
pub mod live;
pub mod mind;
pub mod watch;
pub mod work;

pub use agent::{Agent, growth_of, stature};
pub use group::{Culture, Places, SocialGroup};
pub use kind::{Kind, Kinds, body_of};
pub use live::{AgentView, AgentWorld, Done, FoodHere, Hominins, Now, Person};
pub use mind::{Doing, Intent, Mind, Needs, Offer, Situation, Threat, choose};
pub use watch::{WATCH_M, scatter_near, seen, watched};
pub use work::{Pile, REACH_M, Things, finish_work, plan_work};
