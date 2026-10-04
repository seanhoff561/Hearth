//! People in the world's save (V2.1 §2, H0): every person's record and every band under a format
//! version. A save from an older format is walked forward a step at a time, as `level.json` is;
//! a newer one is refused rather than misread.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::band::Band;
use crate::person::Person;
use crate::sim::People;

/// The format of the people's save. Each format after the first adds its step to [`migrate`]:
/// 2 (H1) gave persons their genomes and phenotypes, 3 (H2) their psyches, 4 their memories.
pub const FORMAT: u32 = 4;

/// The people as saved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PeopleSave {
    pub format: u32,
    pub next_id: u64,
    /// Seconds of play to the bodies' next step.
    #[serde(default)]
    pub body_s: f32,
    pub persons: Vec<Person>,
    pub bands: Vec<Band>,
}

impl People {
    /// The people as they are, to save.
    pub fn to_save(&self) -> PeopleSave {
        PeopleSave {
            format: FORMAT,
            next_id: self.next_id,
            body_s: self.body_s,
            persons: self.persons.clone(),
            bands: self.bands.clone(),
        }
    }

    /// The people of a save, in a world of `seed`.
    pub fn from_save(seed: u64, save: PeopleSave) -> Self {
        let mut people = People::new(seed);
        people.next_id = save.next_id;
        people.body_s = save.body_s;
        people.persons = save.persons;
        people.persons.sort_by_key(|p| p.id);
        people.bands = save.bands;
        people
    }
}

/// Upgrades a save's JSON in place to [`FORMAT`]: the format it was saved in.
pub fn migrate(v: &mut Value) -> Result<u32, String> {
    let format = v
        .get("format")
        .and_then(Value::as_u64)
        .ok_or_else(|| "the people's save has no format".to_owned())? as u32;
    if format > FORMAT {
        return Err(format!(
            "the people were saved by a newer version (format {format}; this one reads up to \
             {FORMAT})"
        ));
    }
    if format == 0 {
        return Err("the people's save has format 0, which never existed".to_owned());
    }
    // 1 → 2: no genomes yet; each band's people are given theirs from its pool when it is next
    // drawn out (`People::endow`), parents before children.
    // 2 → 3: no psyches yet; each is formed from its phenotype when its band is next drawn out
    // (`People::form`); a mind's fear moved into the psyche.
    // 3 → 4: no memories yet; each mental map starts from its band's places when next drawn out.
    Ok(format)
}

/// Reads a save's JSON, walked forward to the current format.
pub fn from_json(bytes: &[u8]) -> Result<PeopleSave, String> {
    let mut v: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    migrate(&mut v)?;
    if let Some(o) = v.as_object_mut() {
        o.insert("format".to_owned(), Value::from(FORMAT));
    }
    serde_json::from_value(v).map_err(|e| e.to_string())
}

/// A save as JSON.
pub fn to_json(save: &PeopleSave) -> Result<Vec<u8>, String> {
    serde_json::to_vec(save).map_err(|e| e.to_string())
}
