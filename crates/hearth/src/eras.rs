//! A world's era (Amendment E §6.4): Wild Earth the one playable; the others are listed as
//! "Coming soon" until Phase F brings their people.

use hearth_content::Content;
use hearth_content::schema::era::Era;

/// Wild Earth: the one playable era, and the era of worlds made before eras.
pub const WILD_EARTH: &str = "hearth:wild_earth";

fn key(id: &str) -> &str {
    id.rsplit(':').next().unwrap_or(id)
}

/// An era by id (Wild Earth for an unknown one).
pub fn era_of<'a>(content: &'a Content, id: &str) -> Option<&'a Era> {
    content
        .eras
        .iter()
        .find(|e| key(&e.id) == key(id))
        .or_else(|| content.eras.iter().find(|e| key(&e.id) == key(WILD_EARTH)))
}
