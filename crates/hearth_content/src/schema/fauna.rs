//! Animal species (`fauna/`), v2 §7.

use serde::{Deserialize, Serialize};

use super::{Color, Range, Season, entry};
use crate::IdRef;

/// Shared skeleton families (v2 §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodyPlan {
    Ungulate,
    Carnivore,
    Rodent,
    Lagomorph,
    Bear,
    Elephant,
    Giraffe,
    Hippo,
    Primate,
    BirdPerching,
    BirdGround,
    Waterfowl,
    Raptor,
    Seabird,
    Penguin,
    FishFusiform,
    FishFlat,
    Eel,
    Seal,
    Cetacean,
    Lizard,
    Crocodilian,
    Turtle,
    Snake,
    Amphibian,
    Crab,
    Insect,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Speeds {
    pub walk_m_s: f32,
    #[serde(default)]
    pub run_m_s: Option<f32>,
    #[serde(default)]
    pub swim_m_s: Option<f32>,
    #[serde(default)]
    pub fly_m_s: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Senses {
    /// Distance at which it notices a moving human in daylight.
    pub vision_m: f32,
    pub hearing_m: f32,
    /// Downwind scent detection distance.
    pub smell_m: f32,
    /// 0–1 relative night vision.
    #[serde(default)]
    pub night_vision: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DietKind {
    Grazer,
    Browser,
    MixedFeeder,
    Frugivore,
    Granivore,
    Carnivore,
    Omnivore,
    Insectivore,
    Piscivore,
    Scavenger,
    FilterFeeder,
}

/// Something eaten: a plant species, an animal species or a material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Food {
    pub food: IdRef,
    /// Relative preference 0–1.
    #[serde(default = "one")]
    pub preference: f32,
}

fn one() -> f32 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diet {
    pub kind: DietKind,
    pub foods: Vec<Food>,
    /// Food needed per day (dry matter for herbivores, meat for carnivores).
    #[serde(default)]
    pub daily_food_kg: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Social {
    Solitary,
    Pair,
    Family,
    Herd { size: Range },
    Pack { size: Range },
    Pride { size: Range },
    Flock { size: Range },
    School { size: Range },
    Colony { size: Range },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Activity {
    Diurnal,
    Nocturnal,
    Crepuscular,
    Cathemeral,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SeasonalBehavior {
    Migration { seasons: Vec<Season> },
    Hibernation,
    Rut { season: Season },
    Birthing { season: Season },
    WinterCoat,
    Spawning { season: Season },
}

/// How and why it can be dangerous to people.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Danger {
    /// Will hunt a human under the right conditions.
    #[serde(default)]
    pub predatory: bool,
    /// Defends itself, young or food with force.
    #[serde(default)]
    pub defensive: bool,
    #[serde(default)]
    pub venomous: bool,
    /// 0–1 baseline aggression (scaled by the Predator Behavior setting).
    #[serde(default)]
    pub aggression: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Domestication {
    /// Name of the domestic form (dog, sheep, cattle...).
    pub domestic_form: String,
    /// Knowledge era in which it becomes possible.
    pub era: u8,
    /// 0 = easy, 1 = only tameable, never domesticated.
    pub difficulty: f32,
}

/// How it breeds, grows up and dies (V2-7).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LifeHistory {
    /// Age at first breeding, years.
    pub maturity_years: f32,
    /// The greatest age commonly reached in the wild, years.
    pub lifespan_years: f32,
    /// Young at a birth.
    pub litter: Range,
    /// Births a year; below one, a birth every so many years (a bear's 0.4: every two and a
    /// half).
    #[serde(default = "one")]
    pub births_per_year: f32,
    /// The day of the year most young are born (in the north; the south is half a year on).
    pub birth_day: u16,
    /// Yearly survival of adults from what is not simulated (disease, accident, old age).
    pub adult_survival: f32,
    /// First-year survival of the young from such causes.
    pub young_survival: f32,
    /// A newborn's mass, kg.
    pub birth_mass_kg: f32,
}

/// Which young leave home to settle elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Dispersers {
    #[default]
    Males,
    Females,
    Both,
}

/// How the population is simulated: large animals as groups that keep their members, small
/// ones as numbers per ecological cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PopulationModel {
    Groups,
    Density,
}

/// How it uses the land (V2-7).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Ranging {
    /// The area one group (or one solitary adult) lives in, km².
    pub home_range_km2: f32,
    /// Keeps others of its kind out of its range.
    #[serde(default)]
    pub territorial: bool,
    /// How far the young go to settle, km.
    pub dispersal_km: f32,
    #[serde(default)]
    pub dispersers: Dispersers,
    /// How much it keeps to cover, 0 (open country) to 1 (thickets only).
    #[serde(default)]
    pub cover: f32,
    /// Groups or density; by default groups from 5 kg up.
    #[serde(default)]
    pub model: Option<PopulationModel>,
}

/// A coat's pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CoatPattern {
    #[default]
    Plain,
    /// Pale spots on the back and flanks (fawns, lynx).
    Spotted,
    /// Pale stripes along the body (piglets).
    Striped,
    /// Dark and pale hairs mixed (wolves, hares).
    Grizzled,
    /// Black and white stripes on the face (badgers).
    Masked,
    /// Speckled feathers.
    Speckled,
    /// Black and white in bold patches (woodpeckers).
    Pied,
    /// Overlapping scales with a zig-zag down the back (adders).
    ZigZag,
    /// Dark bands across the body (rattlesnakes).
    Banded,
}

/// Colours and patterns of its coat, feathers or scales (V2-7): the recipe for its texture.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Coat {
    /// Back and flanks (a bird's back, wings and tail).
    pub base: Color,
    /// Belly, throat and inner legs.
    pub belly: Color,
    /// Muzzle, ear rims, lower legs, tail tip; a bird's beak and legs.
    #[serde(default)]
    pub points: Option<Color>,
    /// The rump patch (deer), and the tail's underside.
    #[serde(default)]
    pub rump: Option<Color>,
    #[serde(default)]
    pub pattern: CoatPattern,
    /// The pattern's colour (spots, stripes, the mask); with no pattern, a bird's breast.
    #[serde(default)]
    pub marking: Option<Color>,
    /// The face or head where it differs: a badger's white face, a bird's bare head or the
    /// disc of an owl's face.
    #[serde(default)]
    pub face: Option<Color>,
    /// The lower legs where they differ from the back (a fox's black stockings).
    #[serde(default)]
    pub legs: Option<Color>,
    /// The tail's tip where it differs from the points (a fox's white tip); all of a short
    /// tail (a cottontail's).
    #[serde(default)]
    pub tail_tip: Option<Color>,
    /// The winter coat's back, where it differs.
    #[serde(default)]
    pub winter: Option<Color>,
    /// The male's back, where it differs (aurochs bulls, cock birds).
    #[serde(default)]
    pub male: Option<Color>,
    /// The young's pattern, where it differs.
    #[serde(default)]
    pub young: Option<CoatPattern>,
}

/// How the ears stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EarShape {
    #[default]
    Pointed,
    Round,
    /// A hare's.
    Long,
    /// Tipped with a tuft (lynx, red squirrel).
    Tufted,
}

/// What grows on the head.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum HeadGear {
    /// Branched antlers of bone, cast and grown again every year by the males (in the
    /// seasons of their `antler` yield): each beam's length (m) and its tines.
    Antlers {
        length_m: f32,
        tines: u8,
        #[serde(default)]
        palmate: bool,
    },
    /// Horns of keratin on a bony core, kept for life: their length along the curve (m) and
    /// how far they curve (0 straight, 1 a half circle).
    Horns {
        length_m: f32,
        curve: f32,
        #[serde(default)]
        both_sexes: bool,
    },
    /// Tusks: the length showing (m), the males'.
    Tusks { length_m: f32 },
}

/// The proportions of its body past its length and height (V2-7 bodies), each a fraction; its
/// body plan has the usual ones.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Shape {
    /// The neck's length, of the body's.
    #[serde(default)]
    pub neck: Option<f32>,
    /// The head's length, of the body's.
    #[serde(default)]
    pub head: Option<f32>,
    /// The snout's (a bird's beak's) length, of the head's.
    #[serde(default)]
    pub snout: Option<f32>,
    /// The tail's length, of the body's.
    #[serde(default)]
    pub tail: Option<f32>,
    /// The tail's thickness, of the body's width.
    #[serde(default)]
    pub tail_width: Option<f32>,
    /// The ears' length, of the head's.
    #[serde(default)]
    pub ears: Option<f32>,
    #[serde(default)]
    pub ear_shape: Option<EarShape>,
    /// The legs' thickness against the body plan's.
    #[serde(default)]
    pub legs: Option<f32>,
    /// A hump over the shoulders, of the shoulder height (bison, bears, boar).
    #[serde(default)]
    pub hump: Option<f32>,
    #[serde(default)]
    pub head_gear: Option<HeadGear>,
}

/// How an animal spends its time when nothing troubles it (V2-7 minds): weights about 1 on
/// what its utility AI chooses among; its kind has the usual ones.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Habits {
    /// Looking up from feeding to watch.
    #[serde(default)]
    pub vigilance: Option<f32>,
    #[serde(default)]
    pub grooming: Option<f32>,
    /// Going about rather than staying put.
    #[serde(default)]
    pub roaming: Option<f32>,
    /// Keeping near its group.
    #[serde(default)]
    pub sociability: Option<f32>,
}

/// One more thing butchering yields: antlers, tusks, a pelt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtraYield {
    /// The material.
    pub material: IdRef,
    /// Mass from a grown animal, kg.
    pub mass_kg: Range,
    /// Only from males (antlers, tusks) when set.
    #[serde(default)]
    pub males_only: bool,
    /// Only in these seasons (antlers are cast in late winter), when not empty.
    #[serde(default)]
    pub seasons: Vec<Season>,
}

/// What butchering yields, as fractions of the live mass (V2-7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Yields {
    /// Lean meat.
    pub meat: f32,
    /// Fat (more in autumn, less in late winter).
    pub fat: f32,
    /// Edible organs: liver, heart, kidneys, tongue.
    pub organs: f32,
    pub bone: f32,
    /// The hide or pelt.
    pub hide: f32,
    pub sinew: f32,
    /// The hide's material (`rawhide` by default, or a fur).
    #[serde(default)]
    pub hide_material: Option<IdRef>,
    /// The flesh's material (`meat` by default; `raw_fish`).
    #[serde(default)]
    pub meat_material: Option<IdRef>,
    #[serde(default)]
    pub extras: Vec<ExtraYield>,
}

/// The foot that makes its tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Foot {
    /// Two toes with a gap between (deer, boar, aurochs).
    ClovenHoof,
    /// Four toes and claw marks (dogs, foxes, wolves).
    PawClawed,
    /// Four toes, claws drawn in (cats).
    PawRetracted,
    /// A whole sole with five toes (bears, badgers, hedgehogs).
    Plantigrade,
    /// Long hind feet ahead of the fore (hares, rabbits, squirrels).
    Hopping,
    /// Three toes forward (birds walking).
    BirdToes,
    /// A trail of a body (snakes).
    Slither,
}

/// The tracks it leaves (V2-7).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub foot: Foot,
    /// A print's length, cm.
    pub length_cm: f32,
    /// A walking stride, m.
    pub stride_m: f32,
}

/// A kind of call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallKind {
    Roar,
    Bark,
    Grunt,
    Squeal,
    Howl,
    Growl,
    Hiss,
    Hoot,
    Song,
    Caw,
    Drum,
    Croak,
    Scream,
    Bellow,
    Gobble,
    Chatter,
    Rattle,
    Huff,
    Buzz,
}

/// When a call is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallWhen {
    /// In the rut or the breeding season.
    Rut,
    /// On sensing danger.
    Alarm,
    /// To keep the group together.
    Contact,
    /// Holding a territory.
    Territory,
    /// Hurt or caught.
    Distress,
    /// Warning off a threat.
    Threat,
    /// At dawn (song).
    Dawn,
    /// At night.
    Night,
}

/// A call (V2-7): what kind, when, how loud and how high.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Call {
    pub kind: CallKind,
    pub when: CallWhen,
    /// Sound level at 1 m, dB.
    pub loudness_db: f32,
    /// Fundamental frequency, Hz.
    pub pitch_hz: Range,
    /// Length of one call, s.
    #[serde(default = "one")]
    pub seconds: f32,
}

/// How it behaves toward people (V2-7).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Temperament {
    /// The distance at which an approaching person makes it flee, m.
    pub flight_m: f32,
    /// 0 shy to 1 bold: how slowly fear rises and how soon it comes back.
    #[serde(default)]
    pub boldness: f32,
    /// Freezes (and hides) before fleeing (fawns, hares, grouse).
    #[serde(default)]
    pub freezes: bool,
}

entry! {
    /// An animal species.
    pub struct Animal in "fauna", schema 1, name name {
        pub name: String,
        #[serde(default)]
        pub scientific: Option<String>,
        pub body_plan: BodyPlan,
        /// Adult mass range.
        pub mass_kg: Range,
        pub length_m: f32,
        #[serde(default)]
        pub shoulder_height_m: Option<f32>,
        pub speed: Speeds,
        pub senses: Senses,
        pub diet: Diet,
        pub social: Social,
        pub activity: Activity,
        /// Ecosystems it lives in.
        pub habitat: Vec<IdRef>,
        #[serde(default)]
        pub seasonal: Vec<SeasonalBehavior>,
        #[serde(default)]
        pub danger: Danger,
        #[serde(default)]
        pub domestication: Option<Domestication>,
        /// Yearly population growth rate at low density (r).
        #[serde(default)]
        pub growth_rate: Option<f32>,
        /// Individuals per km² in good habitat.
        #[serde(default)]
        pub density_per_km2: Option<f32>,
        #[serde(default)]
        pub realms: Vec<String>,
        #[serde(default)]
        pub first_appearance_ya: Option<f64>,
        #[serde(default)]
        pub extinction_ya: Option<f64>,
        #[serde(default)]
        pub life: Option<LifeHistory>,
        #[serde(default)]
        pub ranging: Option<Ranging>,
        #[serde(default)]
        pub coat: Option<Coat>,
        #[serde(default)]
        pub shape: Option<Shape>,
        /// Climbs trees (to flee, to feed, to rest).
        #[serde(default)]
        pub climbs: bool,
        #[serde(default)]
        pub habits: Option<Habits>,
        #[serde(default)]
        pub yields: Option<Yields>,
        #[serde(default)]
        pub track: Option<Track>,
        #[serde(default)]
        pub calls: Vec<Call>,
        #[serde(default)]
        pub temperament: Option<Temperament>,
    }
}
