//! The hands' natural uses (`interaction/`, Amendment P §5.2): with what a hand holds, on what is
//! looked at, what that hand does when its button is pressed. The rules are tried in their
//! order and the first that can be done now is the hand's; nothing irreversible or risky is a
//! default (those are in the action menu only).

use serde::{Deserialize, Serialize};

use super::entry;
use super::item::Use;
use super::process::BlockMatch;

entry! {
    /// A default use of a hand.
    pub struct Intent in "interaction", schema 1, name name {
        pub name: String,
        /// The hand hint's word or two ("snap off").
        pub hint: String,
        /// What is looked at.
        pub target: IntentTarget,
        /// What the hand holds.
        #[serde(default)]
        pub holding: Holding,
        /// What it does.
        pub act: IntentAct,
        /// Order among the rules (lower first).
        #[serde(default)]
        pub order: i32,
    }
}

/// What a hand's use is done to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IntentTarget {
    /// Nothing looked at (Amendment E §3.2: the thing in hand's own use, or a blow).
    Nothing,
    /// A block, by id, material or name.
    Block(BlockMatch),
    /// Any block (the act's own process says which it is done to).
    AnyBlock,
    /// A thing lying in the world.
    Thing,
    /// Water to hand.
    Water,
    /// A fire burning.
    Fire,
    /// A live animal.
    Animal,
}

/// What the hand holds for a rule to apply.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum Holding {
    /// Nothing: an empty hand (a hand that holds something may put it away to do this).
    #[default]
    Empty,
    /// Anything or nothing.
    Anything,
    /// A thing with this property at least (`chopping`, `hard_hammer`).
    Property { property: String, min: f32 },
    /// A thing whose own use is this (a spear thrusts).
    Use(Use),
    /// A weapon: a thing whose own use is a blow (thrust, swing, slash, stab, strike).
    Weapon,
    /// A container for water.
    Vessel,
    /// Something to eat.
    Food,
}

/// What the hand does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IntentAct {
    /// The first process with this verb that can be done on the target with what the hand
    /// holds as its tool.
    Verb(String),
    /// That process.
    Process(String),
    /// Lift the thing looked at.
    PickUp,
    /// Drink: from the water with cupped hands, or from the vessel held.
    Drink,
    /// Fill the vessel held from the water.
    Fill,
    /// Eat what is held.
    Eat,
    /// A blow with what is held (or the fist).
    Blow,
}
