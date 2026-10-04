//! The world as people meet it. What they **sense** ([`Senses`]) is read by everyone deciding at
//! once, so it is shared and read-only; what they **do** to it ([`World`]) — taking things up,
//! laying them down, calling, bending a nest — happens one person at a time, in a fixed order.
//! The game answers both from its loaded world; a test, from a small one of its own.

use glam::DVec3;
use hearth_body::Exposure;
use hearth_craft::engine::Surroundings;
use hearth_fauna::live::Ground;
use hearth_items::Stack;

use crate::species::Species;
use crate::work::Things;

/// What there is to eat where a person stands.
#[derive(Debug, Clone, PartialEq)]
pub struct FoodHere {
    /// The food (a material) and how much a minute of feeding gives (kg).
    pub material: String,
    pub kg_min: f32,
    /// Nuts to gather to crack later (a material: marula stones under a marula).
    pub nuts: Option<String>,
}

/// A player's identity in the world (one per player, as their person's).
pub type PlayerId = u64;

/// A player as the people about them notice them (there may be many, D166).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerSeen {
    pub id: PlayerId,
    pub pos: DVec3,
    /// Coming on fast (running, sprinting).
    pub running: bool,
    /// Hunting near them (a throw, a kill).
    pub hunting: bool,
    /// How plain to see: 1 standing in the open, less crouched or crawling, in cover.
    pub plain: f32,
}

impl PlayerSeen {
    /// A player standing calmly in the open.
    pub fn standing(id: PlayerId, pos: DVec3) -> Self {
        Self {
            id,
            pos,
            running: false,
            hunting: false,
            plain: 1.0,
        }
    }
}

/// The moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Now {
    pub tick: u64,
    /// Local solar time, hours.
    pub hour: f32,
    /// The day of the world's calendar (days since its start, with the day's fraction).
    pub day: f64,
    /// Days in the calendar's year (ages are reckoned in its years).
    pub year_days: f64,
}

/// What the people sense of the world: read by all of them at once while they decide.
pub trait Senses: Sync {
    /// The ground: footing, the trees' trunks and limbs, the water.
    fn ground(&self) -> &(dyn Ground + Sync);
    /// The things lying within `reach` m of `at`: their ids and stacks.
    fn things_near(&self, at: DVec3, reach: f64) -> Vec<(u64, Stack)>;
    /// Where a thing lies.
    fn place_of(&self, id: u64) -> Option<DVec3>;
    /// What there is to eat where one stands, this season.
    fn food_at(&self, at: DVec3) -> Option<FoodHere>;
    /// The nearest place within `within` m with food to pick.
    fn food_near(&self, at: DVec3, within: f64) -> Option<DVec3>;
    /// The air and weather on a body at a place (up in a tree's crown or not).
    fn exposure(&self, at: DVec3, in_tree: bool) -> Exposure;
    /// The weather and the season for work at a place.
    fn surroundings(&self, at: DVec3) -> Surroundings;
    /// Hunters of people within `within` m of a place.
    fn hunters_near(&self, at: DVec3, within: f64) -> Vec<DVec3>;
    /// A place's latitude (degrees): its sunlight sets a pool's pigmentation (the equator's,
    /// where a world does not say).
    fn latitude(&self, _at: DVec3) -> f64 {
        0.0
    }
    /// What processes can be done to within `within` m of a place — a scatter of stones, a
    /// tree, a stand of nettles — where each is and how it is aimed at (none where a world does
    /// not say).
    fn targets_near(&self, _at: DVec3, _within: f64) -> Vec<(DVec3, hearth_craft::engine::Aimed)> {
        Vec::new()
    }
}

/// What the people do to the world, one at a time.
pub trait World: Senses {
    /// The things lying about, to take up and lay down.
    fn things(&mut self) -> &mut dyn Things;
    /// A call: the alarm at a hunter, or a contact call.
    fn call(&mut self, at: DVec3, alarm: bool);
    /// A nest bent in a tree's crown about a place: the trace it leaves, and where one lies on
    /// it (none where no nest could be made: it sleeps where it is).
    fn nest(&mut self, at: DVec3) -> Option<DVec3>;
    /// A process done to a target at a place, with what it does to it (a scatter of stones
    /// gathered is gone).
    fn worked(
        &mut self,
        _at: DVec3,
        _aimed: &hearth_craft::engine::Aimed,
        _effect: hearth_content::schema::process::Effect,
    ) {
    }
    /// A band of a species drawn out about a place for the first time: the world may lay there
    /// the traces of its past (its anvil and stones under a nut tree, the scatter of its
    /// knapping).
    fn settle(&mut self, _at: DVec3, _species: &Species) {}
}
