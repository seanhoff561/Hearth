//! Processes (`processes/`): transformations of materials by actions, tools and conditions
//! (v2 §11.3). The same data drives the player, hominins and future simulated humans.

use serde::{Deserialize, Serialize};

use super::material::MaterialFilter;
use super::{Duration, Range, Season, entry};
use crate::IdRef;

/// What an input or output slot accepts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Match {
    /// A specific item (explicit or generated `form/material`).
    Item(IdRef),
    /// Any item of a form, optionally restricted by material.
    Form {
        form: IdRef,
        #[serde(default)]
        materials: Option<MaterialFilter>,
    },
    /// Bulk material by mass (clay, sand, water, meat).
    Material(IdRef),
    /// Any item carrying a tag.
    Tag(String),
    /// A garment (`hide_cape`), of the material in play.
    Garment(IdRef),
}

/// What a process is done to in the world: what the person looks at while doing it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Target {
    /// A block, by id or by what it is made of.
    Block(BlockMatch),
    /// A thing lying in the world (an anvil stone, a carcass); when it matches an input it is
    /// that input.
    Thing(Match),
    /// Water to hand: a pool, a stream, the sea's edge.
    Water,
    /// A fire burning in the world (a lit hearth, a natural fire).
    Fire,
    /// The open top of solid ground, to build or lay something on.
    Ground,
    /// A live animal (V2-12): one to catch, tether, milk or pluck.
    Animal(AnimalMatch),
}

/// Which live animals a target accepts (V2-12). Every condition given must hold.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AnimalMatch {
    /// Kept by people (true), wild (false), either (unset).
    #[serde(default)]
    pub kept: Option<bool>,
    /// Young (not yet grown: true), grown (false), either.
    #[serde(default)]
    pub young: Option<bool>,
    /// Female (true), male (false), either.
    #[serde(default)]
    pub female: Option<bool>,
    /// Of a kind people can keep (one with a domestic form).
    #[serde(default)]
    pub domesticable: bool,
}

impl AnimalMatch {
    /// Whether an animal of this sort is accepted.
    pub fn accepts(&self, kept: bool, young: bool, female: bool, domesticable: bool) -> bool {
        self.kept.is_none_or(|k| k == kept)
            && self.young.is_none_or(|y| y == young)
            && self.female.is_none_or(|f| f == female)
            && (!self.domesticable || domesticable)
    }
}

/// Which blocks a target accepts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BlockMatch {
    /// One block, any of its states (`oak_log`).
    Id(IdRef),
    /// Blocks made of a material the filter accepts (any clay, a birch log).
    Material(MaterialFilter),
    /// Blocks whose name ends in this (`_cobbles`: loose stones of any rock).
    Suffix(String),
    /// Blocks whose name (without its namespace) starts with this (`post/`: posts of any
    /// wood).
    Prefix(String),
    /// Blocks any of these accept (soils, and the turf over them).
    Any(Vec<BlockMatch>),
}

/// What doing a process does to its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Effect {
    /// It stays as it was (fruit picked, bark peeled, a stone struck).
    #[default]
    Keep,
    /// It is gone (a plant pulled up, deadwood broken off, a carcass butchered).
    Remove,
    /// Dug out: the block goes and its loose earth falls in a spoil pile beside the hole.
    Excavate,
    /// Taken a little at a time: the block goes once its whole mass has been taken.
    Deplete,
    /// A laid fire is lit.
    Ignite,
    /// What is used up feeds the fire.
    Feed,
    /// The fire is banked: covered so it smoulders for hours.
    Bank,
    /// The first input, kept, is mended: its edge, point or binding made good again.
    Mend,
    /// A standing tree is cut through and falls (its trunk lies along the ground after).
    Fell,
    /// A limb is cut off a tree with what grows from it.
    Lop,
    /// A lying trunk is cut through: a section comes off.
    Buck,
    /// The vegetation aimed at is set alight (it spreads as dry as it is).
    SetAlight,
    /// A construction piece ([`Process::places`]) is put up over what is aimed at, facing as the
    /// person faces, of the material of what it uses.
    Place,
    /// Soil is broken up for a field (V2-12): the block becomes tilled soil, a plot.
    Till,
    /// Seed (the input with a lot) is sown in the plot aimed at.
    Sow,
    /// A ripe crop is reaped: what the plot gives, its lot as the reaping leaves it.
    Reap,
    /// The weeds are pulled from the plot.
    Weed,
    /// Dung is spread on the plot: its soil the richer.
    Manure,
    /// The best of the seed is picked out to sow (its lot's grain the larger).
    Select,
    /// A wild young animal is caught to keep: raised by hand, it follows its keeper.
    Catch,
    /// A kept animal is tethered to a stake where it stands.
    Tether,
    /// A kept animal is led off on a halter: it follows its keeper.
    Lead,
    /// A kept mother in milk is milked: the milk is what is made.
    Milk,
    /// A kept animal's fleece is plucked as it moults: the wool is what is made.
    Pluck,
    /// A kept animal is killed for its meat: it lies dead, to be butchered.
    Slaughter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Input {
    pub item: Match,
    /// Count for items, kilograms for materials.
    pub amount: f32,
    /// False for things that are used but not used up (an anvil stone).
    #[serde(default = "yes")]
    pub consumed: bool,
}

fn yes() -> bool {
    true
}

/// A tool requirement: something in hand with a property at or above `min`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolReq {
    pub property: String,
    pub min: f32,
}

/// Environmental or workstation condition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Condition {
    /// A heat source at least this hot.
    HeatAtLeastC(f32),
    /// Standing water or a water container at hand.
    Water,
    Dry,
    Daylight,
    /// Air temperature below (e.g. freezing meat).
    ColdBelowC(f32),
    /// Inside a structure with a roof.
    Sheltered,
    /// Next to a named feature of the world ("river", "cliff", "clay bank").
    Near(String),
    /// Only in these seasons (fruit, nuts, fibre stems ripe).
    Season(Vec<Season>),
}

/// Quality of the output: base plus skill and tool influences, clamped 0–1.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Quality {
    #[serde(default)]
    pub base: f32,
    /// Added at skill 1.
    #[serde(default)]
    pub from_skill: f32,
    /// Added in proportion to the input material's knapping/workability.
    #[serde(default)]
    pub from_material: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Output {
    pub item: Match,
    /// Count range for items, kg range for materials.
    pub amount: Range,
    #[serde(default = "certain")]
    pub chance: f32,
    #[serde(default)]
    pub quality: Quality,
    /// Only in these seasons, when not empty (a deer's fat in autumn, antlers while it carries
    /// them).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seasons: Vec<Season>,
}

fn certain() -> f32 {
    1.0
}

/// A way the process can go wrong.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Failure {
    /// Chance at skill 0; skill reduces it towards `min_chance`.
    pub chance: f32,
    #[serde(default)]
    pub min_chance: f32,
    /// What happens, in words for the journal.
    pub outcome: String,
    /// Inputs lost on failure.
    #[serde(default = "yes")]
    pub loses_inputs: bool,
    /// Injury risk description, if any.
    #[serde(default)]
    pub injury: Option<String>,
}

entry! {
    /// A process.
    pub struct Process in "processes", schema 1, name name {
        pub name: String,
        /// Verb phrase offered as a contextual action ("strike off a flake").
        pub action: String,
        /// The kind of doing (`strike`, `cut`, `scrape`, `heat`, `twist`, `dig`): doing it emits
        /// the discovery trigger `verb:key` for each thing used and the target, for every key
        /// they answer to (their form, material, and the tags of both).
        #[serde(default)]
        pub verb: Option<String>,
        /// What is looked at while doing it (none: things in hand).
        #[serde(default)]
        pub target: Option<Target>,
        /// What it does to the target.
        #[serde(default)]
        pub effect: Effect,
        pub inputs: Vec<Input>,
        #[serde(default)]
        pub tools: Vec<ToolReq>,
        #[serde(default)]
        pub station: Option<IdRef>,
        #[serde(default)]
        pub conditions: Vec<Condition>,
        pub duration: Duration,
        /// Knowledge needed to attempt it.
        #[serde(default)]
        pub knowledge: Option<IdRef>,
        /// Skill track practised by it.
        #[serde(default)]
        pub skill: Option<String>,
        pub outputs: Vec<Output>,
        #[serde(default)]
        pub byproducts: Vec<Output>,
        #[serde(default)]
        pub failures: Vec<Failure>,
        /// Further discovery triggers doing it emits.
        #[serde(default)]
        pub teaches: Vec<String>,
        /// The person works at it throughout (holding the action); otherwise it is set up and
        /// left (drying, soaking), and the work waits where it was put until it is done.
        #[serde(default = "yes")]
        pub attended: bool,
        /// How hard the work is (METs: 1.5 sitting at handwork, 3 light work, 6 hard digging).
        #[serde(default = "light_work")]
        pub mets: f32,
        /// How much of the tool's condition one doing wears away (an edge dulled, a point
        /// blunted).
        #[serde(default)]
        pub wear: f32,
        /// How many times a year one block yields it (a tree's dead branches, a bush's
        /// berries); none: as often as wanted.
        #[serde(default)]
        pub harvests: Option<u8>,
        /// A treatment it gives the body's most recent injury that has not had it (a poultice
        /// of yarrow on a cut).
        #[serde(default)]
        pub treats: Option<String>,
        /// The construction piece it puts up (with the `Place` effect).
        #[serde(default)]
        pub places: Option<IdRef>,
        /// Firing (v2 §11.4): unattended work at a station's fire whose outcome is the heat it
        /// got — the hottest it was, and how long it was held at or above the firing range of
        /// what is fired (its material's `firing_c`).
        #[serde(default)]
        pub firing: Option<Firing>,
    }
}

/// How a firing is judged (v2 §11.4): hours at or above the bottom of the material's firing
/// range needed for it to be fired through.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Firing {
    pub hold_h: f32,
}

fn light_work() -> f32 {
    2.5
}
