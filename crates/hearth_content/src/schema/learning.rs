//! Learning (`humans/learning/`), V2.1 §11; H6: how a people's knowledge passes between them —
//! how readily the young learn a technique from those who know it, the harder ones the more
//! slowly, how much less the grown learn, how far neighbours' knowers count, how often one tries
//! something new, how much teaching speeds learning, how often the old tell stories.

use super::entry;
use crate::IdRef;

entry! {
    /// How a people's knowledge passes between them (V2.1 §11.2–11.4).
    pub struct Transmission in "humans/learning/transmission", schema 1, name name {
        pub name: String,
        pub species: IdRef,
        /// A learner's chance a year to take a technique from each one who knows it, for one
        /// resting on nothing; each step deeper in what it rests on divides it by
        /// e^`depth_factor`.
        pub learn_year: f32,
        pub depth_factor: f32,
        /// The ages that learn as the young do; how much less the grown learn.
        pub learn_ages: (f32, f32),
        pub grown_factor: f32,
        /// How much a knower of a neighbouring band counts, at its nearest.
        pub contact_weight: f32,
        /// A grown one's chance a month to try something new (times twice its curiosity), and
        /// the insight a try gives.
        pub innovate_month: f32,
        pub innovate_insight: f32,
        /// How many times faster one learns taught than only watching (1 for a people that does
        /// not teach).
        pub teach: f32,
        /// A grown one's chance a second, in company in the evening, to tell a story.
        pub story_second: f32,
    }
}
