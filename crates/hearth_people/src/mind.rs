//! Minds: what a person needs, what it knows of the moment about it, and what it chooses to do —
//! each thing it could do weighed by what it needs and what it fears (a utility choice, the third
//! of V2.1 §6's layers, which H2 builds the others about), within what its species ever does.

use glam::DVec3;
use hearth_body::{Body, BodyConfig, Hunger, Thirst, Tiredness};
use hearth_content::schema::humans::Behavior;
use serde::{Deserialize, Serialize};

use crate::psyche::{Feeling, Psyche, Tendency, Value};
use crate::species::Species;
use crate::work::REACH_M;

/// Why a person goes somewhere.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Intent {
    /// To feed where food grows.
    Feed,
    /// To the water to drink.
    Drink,
    /// To do a process there (its recipe).
    Work(usize),
    /// Up a tree to make its nest and sleep.
    Nest,
    /// Back to the others.
    Rejoin,
    /// About its range.
    Roam,
    /// To a place its plan goes by.
    Step,
}

/// What a person is doing.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum Doing {
    /// Looking about, about to choose.
    #[default]
    Idle,
    /// On its way to a place.
    Going {
        to: DVec3,
        then: Intent,
    },
    /// Picking and eating what grows where it is.
    Feeding,
    Drinking,
    /// A process under way (its recipe).
    Working {
        recipe: usize,
    },
    /// Taking up a thing lying where it stands.
    Taking {
        id: u64,
    },
    Resting,
    /// Grooming another (or itself).
    Grooming {
        other: Option<u64>,
    },
    /// Bending branches into a nest in its tree.
    Nesting,
    Sleeping,
    /// Watching something it is wary or curious of.
    Watching {
        at: DVec3,
    },
    /// Calling out at a hunter it has seen.
    Alarm {
        at: DVec3,
    },
    /// Facing a threat with the others: shouting, brandishing sticks, throwing stones.
    Mobbing {
        at: DVec3,
    },
    /// Running for a tree, or away.
    Fleeing {
        to: DVec3,
    },
    /// An infant carried by another (its mother, or else one of its band), nursed when hungry.
    Carried {
        by: u64,
    },
    /// A child at play: chasing another of the young, or romping about on its own.
    Playing {
        to: DVec3,
        with: Option<u64>,
    },
    /// A child close by a grown one at its work, watching and trying it after them.
    Imitating {
        at: DVec3,
        whom: u64,
        recipe: usize,
    },
    /// Taking food it carries to one who is hungry (V2.1 §8.3).
    Sharing {
        to: u64,
        at: DVec3,
    },
    /// Staying by one of its own who is hurt, a comfort to them (V2.1 §8.3).
    Tending {
        who: u64,
        at: DVec3,
    },
    /// Mocking one it thinks ill of to their face: a sanction (V2.1 §8.4).
    Mocking {
        who: u64,
        at: DVec3,
    },
    /// Having it out with another (V2.1 §8.6): words, threats, a scuffle.
    Quarrelling {
        with: u64,
        rung: crate::conflict::Rung,
    },
    /// Stepping into a quarrel between two, to talk them round.
    Mediating {
        a: u64,
        b: u64,
    },
    /// Going to meet a stranger, and greeting it (V2.1 §8.7).
    Greeting {
        who: u64,
    },
    /// Warning a stranger off: up to them, shouting, the body made big.
    WarningOff {
        who: u64,
    },
}

/// How pressing a person's needs are, 0 not at all … 1 desperately.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Needs {
    pub hunger: f32,
    pub thirst: f32,
    pub tiredness: f32,
    pub fear: f32,
}

impl Needs {
    /// From its body (hungry, thirsty and tired as the player's body feels it) and its fear.
    pub fn of(body: &Body, cfg: &BodyConfig, fear: f32) -> Self {
        let s = body.status(cfg);
        let hunger = match s.hunger {
            Hunger::Stuffed | Hunger::Full => 0.0,
            Hunger::Satisfied => 0.1,
            Hunger::Peckish => 0.35,
            Hunger::Hungry => 0.6,
            Hunger::VeryHungry => 0.85,
            Hunger::Starving => 1.0,
        };
        let thirst = match s.thirst {
            Thirst::Sated => 0.0,
            Thirst::Fine => 0.15,
            Thirst::Thirsty => 0.5,
            Thirst::VeryThirsty => 0.8,
            Thirst::Parched | Thirst::Dying => 1.0,
        };
        let tiredness = match s.tiredness {
            Tiredness::Rested => 0.0,
            Tiredness::Awake => 0.2,
            Tiredness::Tired => 0.5,
            Tiredness::VeryTired => 0.8,
            Tiredness::Exhausted => 1.0,
        };
        Self {
            hunger,
            thirst,
            tiredness,
            fear: fear.clamp(0.0, 1.0),
        }
    }
}

/// A hunter or a person it has noticed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Threat {
    pub at: DVec3,
    /// How far (m).
    pub dist: f32,
    /// A hunter that takes its kind (not a person).
    pub hunter: bool,
}

/// A stranger near (V2.1 §8.7): one not of its band, nor its band's guest, whom it does not
/// know or trust.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stranger {
    pub id: u64,
    pub at: DVec3,
    /// How far (m).
    pub dist: f64,
    /// Grown (a child is never warned off).
    pub grown: bool,
    /// Not welcome here, to be warned off: the country crowded, a bad name, a grievance.
    pub unwelcome: bool,
    /// How near it comes before it is met (m).
    pub greet_m: f64,
    /// Another of its band is already meeting it, or warning it off.
    pub met: bool,
}

/// A process it could do here, offered by what lies about it: the recipe, where it is done, and
/// whether what it makes is food.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Offer {
    pub recipe: usize,
    pub at: DVec3,
    pub feeds: bool,
    /// How much the work is its own by its culture's division of labour: 1 for anyone's, more
    /// for its sex's work there, less for the other's (V2.1 §9.1; ground rule 6).
    pub labour: f32,
}

/// What a person knows of the moment about it: what its senses tell it and its group remembers.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Situation {
    /// Where its feet are.
    pub pos: DVec3,
    /// Local solar time, hours.
    pub hour: f32,
    pub threat: Option<Threat>,
    /// How near (m) a threat may come before it runs: its kind's flight distance, the less for a
    /// person its group has come to tolerate.
    pub flight_m: f32,
    pub in_tree: bool,
    pub in_nest: bool,
    /// The nearest tree it could climb into (its sleeping trees first).
    pub tree: Option<DVec3>,
    /// The nearest water it knows of.
    pub water: Option<DVec3>,
    /// Water where it stands.
    pub water_here: bool,
    /// The nearest place with food to pick (fruit underfoot, a fruiting tree).
    pub food: Option<DVec3>,
    /// Food where it stands, and how fast it can eat it (kg a minute).
    pub food_here: bool,
    pub food_rate: f32,
    pub offers: Vec<Offer>,
    /// Another of its group is calling the alarm.
    pub alarm_raised: bool,
    /// Grown ones of its group within a stone's throw.
    pub grown_near: u16,
    /// The middle of its group, and how far it is from it (m).
    pub group_at: Option<DVec3>,
    pub from_group_m: f32,
    /// It is grown (the young keep to their mothers).
    pub grown: bool,
    /// It is a child or a juvenile, who play; and one still learning by watching (to its
    /// adolescence).
    pub plays: bool,
    pub learns: bool,
    /// The nearest of the band's young to play with, and where.
    pub playmate: Option<(u64, DVec3)>,
    /// A grown one of the band at its work close by: who, where, and the work.
    pub work_near: Option<(u64, DVec3, usize)>,
    /// It carries food, and one near it is hungry whom it would feed: who, where, and how
    /// dearly (1 its own household's young … a band-mate it is not fond of).
    pub share_with: Option<(u64, DVec3, f32)>,
    /// One of its own is hurt near: who, where, and how fond of them it is.
    pub hurt_near: Option<(u64, DVec3, f32)>,
    /// One near it thinks ill of: who, where, how bad a name (−1 … 0), whether the name brings
    /// mockery and keeping away.
    pub scorn: Option<(u64, DVec3, f32, bool, bool)>,
    /// Where its band keeps camp.
    pub camp: Option<DVec3>,
    /// The nearest stranger it sees, if any (V2.1 §8.7).
    pub stranger: Option<Stranger>,
    /// One warning it off: where they stand.
    pub warned: Option<DVec3>,
    /// The next step of what it means to do (its plan's), if it has one.
    pub project: Option<Doing>,
}

/// What a person keeps in mind between moments (the layered mind of H2 replaces it).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Mind {
    pub doing: Doing,
    /// Seconds of play before it looks about and chooses again (danger interrupts at once).
    pub timer: f32,
    /// What its body bears (0–1: hunger, thirst, weariness, pain), as last felt.
    pub strain: f32,
    /// Where and when (seconds of play) it last saw hunters.
    pub seen: Vec<(DVec3, f64)>,
    /// What it means to have, if anything: kept until it has it or gives it up.
    pub goal: Option<crate::plan::Want>,
    /// How it means to get it, step by step (made again when the world has moved on).
    #[serde(skip)]
    pub plan: Option<crate::plan::Plan>,
    /// Plans for the goal that came to nothing; at three it is given up.
    pub failures: u8,
    /// Times running it chose not to feed one hungry near while it carried food, and the one
    /// last kept from (seen at three: a breach of the sharing norm).
    #[serde(skip)]
    pub withheld: u8,
    #[serde(skip)]
    pub stingy_to: Option<u64>,
    /// The one of its band it is apprenticed to, while young (V2.1 §11.2).
    pub master: Option<u64>,
}

/// How far a person strays from its group's middle before it goes back to them (m).
const STRAY_M: f32 = 30.0;
/// How far from its camp a person goes about its day (m).
const CAMP_M: f32 = 400.0;
/// Those who keep camp sleep close about its fire, a few steps out (m): its warmth reaches no
/// further on a cold night.
const CAMP_SLEEP_M: f64 = 3.0;
const CAMP_SLEEP_RING_M: f64 = 1.5;

/// Chooses what to do now, from what it needs and what it knows of the moment, as its psyche
/// tilts it: danger first (to face a hunter with the others, to flee up a tree or away, to call
/// the alarm, to watch), then its nest at night, then the others if it has strayed, then the most
/// pressing of its needs — drinking, feeding, a work that feeds it — and otherwise rest and
/// company. `roll` (0–1) varies the idle choices.
pub fn choose(
    species: &Species,
    needs: &Needs,
    s: &Situation,
    psyche: &Psyche,
    roll: f32,
) -> Doing {
    use hearth_content::schema::mind::Routinely;
    // Night by the routine of its kind; its other hours pull toward what they are for.
    let night = species.asleep_at(s.hour);
    let pull = |doing: Routinely| species.routinely(doing, s.hour);
    if let Some(t) = s.threat {
        if t.dist < s.flight_m {
            // A hunter that enough grown ones face is mobbed; a nearer one, or a lone one, is
            // fled — up a tree if there is one.
            let mob = t.hunter
                && species.does(Behavior::MobThreat)
                && s.grown
                && s.grown_near >= 4
                && t.dist > 6.0
                && needs.fear < 0.6 + 0.4 * psyche.value(Value::Courage);
            if mob {
                return Doing::Mobbing { at: t.at };
            }
            if s.in_tree {
                return Doing::Watching { at: t.at };
            }
            if species.does(Behavior::FleeToTrees)
                && let Some(tree) = s.tree
            {
                return Doing::Fleeing { to: tree };
            }
            let away = (s.pos - t.at).normalize_or(DVec3::X);
            return Doing::Fleeing {
                to: s.pos + away * 40.0,
            };
        }
        if t.hunter && s.grown && species.does(Behavior::AlarmCall) && !s.alarm_raised {
            return Doing::Alarm { at: t.at };
        }
        return Doing::Watching { at: t.at };
    }
    // Warned off by those whose country it is in: away.
    if let Some(at) = s.warned {
        let away = (s.pos - at).normalize_or(DVec3::X);
        return Doing::Going {
            to: s.pos + away * 60.0,
            then: Intent::Roam,
        };
    }
    if night {
        if s.in_nest {
            return Doing::Sleeping;
        }
        // Those who keep camp sleep there, about its fire, on the side they come from.
        if let Some(camp) = s.camp
            && !species.does(Behavior::TreeNest)
            && (camp - s.pos).length() > CAMP_SLEEP_M
        {
            let from = DVec3::new(s.pos.x - camp.x, 0.0, s.pos.z - camp.z).normalize_or(DVec3::X);
            return Doing::Going {
                to: camp + from * CAMP_SLEEP_RING_M,
                then: Intent::Rejoin,
            };
        }
        if s.in_tree && species.does(Behavior::TreeNest) {
            return Doing::Nesting;
        }
        // Those who nest in trees go up one; the others sleep where they are (camps and
        // shelters come with H3).
        if species.does(Behavior::TreeNest)
            && let Some(tree) = s.tree
        {
            return Doing::Going {
                to: tree,
                then: Intent::Nest,
            };
        }
        return Doing::Sleeping;
    }
    // A stranger near: the young to their mothers.
    if !s.grown
        && s.stranger.is_some()
        && s.from_group_m > 6.0
        && let Some(at) = s.group_at
    {
        return Doing::Going {
            to: at,
            then: Intent::Rejoin,
        };
    }
    // Far from camp, back toward it.
    if let Some(camp) = s.camp
        && (camp - s.pos).length() as f32 > CAMP_M
        && needs.hunger < 0.6
        && needs.thirst < 0.6
    {
        return Doing::Going {
            to: camp,
            then: Intent::Rejoin,
        };
    }
    // The conforming keep closer to the others.
    let stray = STRAY_M * (1.5 - psyche.tendency(Tendency::Conformity));
    if s.from_group_m > stray
        && let Some(at) = s.group_at
    {
        return Doing::Going {
            to: at,
            then: Intent::Rejoin,
        };
    }
    let near = |p: DVec3| (p - s.pos).length() <= REACH_M;
    // Rest weighs more when weary, low or grieving.
    let low = (-psyche.mood).max(0.0) * 0.15 + psyche.feeling(Feeling::Grief) * 0.3;
    let mut best = (
        Doing::Resting,
        0.2 + 0.5 * needs.tiredness + low + pull(Routinely::Rest),
    );
    let mut consider = |d: Doing, score: f32| {
        if score > best.1 {
            best = (d, score);
        }
    };
    // A stranger near (V2.1 §8.7): watched, the more by the wary; met and greeted when it
    // comes near, by the trusting and the sociable the more readily — or, unwelcome, warned off
    // by the bolder.
    if s.grown
        && let Some(st) = s.stranger
    {
        let trusting = psyche.tendency(Tendency::TrustStrangers);
        consider(Doing::Watching { at: st.at }, 0.45 + 0.3 * (1.0 - trusting));
        if !st.met {
            if st.unwelcome {
                let bold = psyche.tendency(Tendency::Dominance);
                // A trespasser stirs more than a passer-by: the bolder half of a band of
                // ordinary temper warns it off rather than watching it (D190).
                consider(
                    Doing::WarningOff { who: st.id },
                    0.55 + 0.5 * bold - 0.3 * trusting + 0.1 * roll,
                );
            } else if st.dist <= st.greet_m {
                let sociable = psyche.tendency(Tendency::Sociability);
                consider(
                    Doing::Greeting { who: st.id },
                    0.5 + 0.4 * trusting + 0.2 * sociable + 0.1 * roll,
                );
            }
        }
    }
    if needs.thirst > 0.25 {
        if s.water_here {
            consider(Doing::Drinking, 0.5 + 3.0 * needs.thirst);
        } else if let Some(w) = s.water {
            let go = Doing::Going {
                to: w,
                then: Intent::Drink,
            };
            consider(go, 0.4 + 3.0 * needs.thirst);
        }
    }
    if needs.hunger > 0.1 {
        if s.food_here {
            // As worth it as it is rich: a thin scatter of grubs less than a fruiting tree.
            let rich = (s.food_rate / 0.1).clamp(0.15, 1.0);
            let score = (0.3 + 2.5 * needs.hunger) * rich + pull(Routinely::Forage);
            consider(Doing::Feeding, score);
        }
        if let Some(f) = s.food {
            let go = Doing::Going {
                to: f,
                then: Intent::Feed,
            };
            consider(go, 0.2 + 2.4 * needs.hunger + pull(Routinely::Forage));
        }
    }
    for o in &s.offers {
        // A work that feeds is worth a little more than picking (the kernels are rich); one
        // that does not (a flake struck) is done now and then.
        let score = if o.feeds {
            if needs.hunger > 0.1 {
                0.4 + 2.6 * needs.hunger
            } else {
                0.0
            }
        } else {
            (0.25 * (0.5 + psyche.tendency(Tendency::Diligence)) + 0.35 * roll) * o.labour
                + pull(Routinely::Work)
        };
        let d = if near(o.at) {
            Doing::Working { recipe: o.recipe }
        } else {
            Doing::Going {
                to: o.at,
                then: Intent::Work(o.recipe),
            }
        };
        consider(d, score);
    }
    if s.grown_near > 0 {
        let social = 0.2 * (0.5 + psyche.tendency(Tendency::Sociability));
        let score = social + 0.4 * roll + pull(Routinely::Socialize);
        consider(Doing::Grooming { other: None }, score);
    }
    // The young watch the grown at their work and try it after them, the curious the more; and
    // they play — chasing one another, or romping about on their own.
    let curious = psyche.tendency(Tendency::Curiosity);
    if let Some((whom, at, recipe)) = s.work_near {
        // The grown watch too, a work they do not know, the curious the more.
        let base = if s.learns { 0.3 } else { 0.1 };
        let roll_w = if s.learns { 0.2 } else { 0.15 };
        consider(
            Doing::Imitating { at, whom, recipe },
            base + 0.4 * curious + roll_w * roll,
        );
    }
    if s.plays {
        let (to, with) = match s.playmate {
            Some((id, at)) => (at, Some(id)),
            None => {
                let a = roll as f64 * std::f64::consts::TAU;
                (s.pos + DVec3::new(a.cos(), 0.0, a.sin()) * 5.0, None)
            }
        };
        let company = if with.is_some() { 0.15 } else { 0.0 };
        let score = 0.25 + 0.2 * psyche.tendency(Tendency::Sociability) + 0.3 * roll + company;
        consider(Doing::Playing { to, with }, score);
    }
    // Food it carries to one who is hungry: its household's first, kin and those it is fond of,
    // the generous the more readily (V2.1 §8.3); not while hungry itself.
    if let Some((to, at, dear)) = s.share_with
        && needs.hunger < 0.3
    {
        let score = 0.2 + 0.6 * dear + 0.3 * psyche.value(Value::Generosity) + 0.1 * roll;
        consider(Doing::Sharing { to, at }, score);
    }
    // One it thinks ill of near: mocked to its face while the indignation lasts, kept away
    // from when the name is worse (V2.1 §8.4).
    if let Some((who, at, bad, ridicule, avoid)) = s.scorn {
        if avoid && (at - s.pos).length() < 4.0 {
            let away = (s.pos - at).normalize_or(DVec3::X);
            consider(
                Doing::Going {
                    to: s.pos + away * 12.0,
                    then: Intent::Roam,
                },
                0.45 - bad,
            );
        }
        if ridicule {
            let score = 0.1 + 0.6 * psyche.feeling(Feeling::Indignation) - 0.3 * bad + 0.1 * roll;
            consider(Doing::Mocking { who, at }, score);
        }
    }
    // One of its own hurt: staying by them (a child by its mother too), the fonder the more
    // readily.
    if let Some((who, at, fond)) = s.hurt_near {
        let score = 0.15 + 0.6 * fond + 0.1 * psyche.tendency(Tendency::Cooperativeness);
        consider(Doing::Tending { who, at }, score);
    }
    // What it means to do, the diligent the more readily.
    if let Some(d) = &s.project {
        let score =
            0.45 + 0.3 * psyche.tendency(Tendency::Diligence) + 0.1 * roll + pull(Routinely::Work);
        consider(d.clone(), score);
    }
    best.0
}
