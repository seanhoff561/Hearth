//! Block-state mapping between a save and the running content. Saves store state names; on
//! load they are matched against the registry. Names that no longer exist become placeholder
//! blocks with exactly the same name and properties, so they render as "unknown", keep their
//! data, and are written back unchanged (v2 §3.5).

use std::collections::BTreeMap;

use hearth_core::ResourceLocation;
use hearth_world::block::BlockDef;
use hearth_world::{BlockRegistry, BlockStateId};

use crate::SaveError;

/// Splits `ns:block[a=1,b=2]` into the block name and its property pairs (in order).
pub fn split_state(s: &str) -> Option<(ResourceLocation, Vec<(String, String)>)> {
    let s = s.trim();
    let (name, props) = match s.find('[') {
        Some(i) if s.ends_with(']') => (&s[..i], &s[i + 1..s.len() - 1]),
        Some(_) => return None,
        None => (s, ""),
    };
    let rl = ResourceLocation::parse(name).ok()?;
    let mut out = Vec::new();
    for kv in props.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (k, v) = kv.split_once('=')?;
        out.push((k.trim().to_owned(), v.trim().to_owned()));
    }
    Some((rl, out))
}

/// Placeholder definitions for the saved states the content doesn't know.
pub fn placeholders(
    known: &dyn Fn(&str) -> bool,
    saved: &[String],
) -> Result<Vec<(ResourceLocation, BlockDef)>, SaveError> {
    // block name → property name → values, in first-seen order.
    let mut blocks: BTreeMap<String, (ResourceLocation, Vec<(String, Vec<String>)>)> =
        BTreeMap::new();
    for s in saved {
        if known(s) {
            continue;
        }
        let (rl, props) = split_state(s)
            .ok_or_else(|| SaveError::Corrupt(format!("bad block state name `{s}`")))?;
        let entry = blocks
            .entry(rl.to_string())
            .or_insert_with(|| (rl.clone(), Vec::new()));
        for (k, v) in props {
            match entry.1.iter_mut().find(|(n, _)| *n == k) {
                Some((_, values)) => {
                    if !values.contains(&v) {
                        values.push(v);
                    }
                }
                None => entry.1.push((k, vec![v])),
            }
        }
    }
    Ok(blocks
        .into_values()
        .map(|(rl, props)| {
            let def = BlockDef {
                properties: props
                    .iter()
                    .map(|(k, vs)| format!("{k}:{}", vs.join(",")))
                    .collect(),
                defaults: props
                    .iter()
                    .map(|(k, vs)| (k.clone(), vs[0].clone()))
                    .collect(),
                ..BlockDef::unknown()
            };
            (rl, def)
        })
        .collect())
}

/// Saved id → registry id.
#[derive(Debug, Clone)]
pub struct Remap {
    table: Vec<BlockStateId>,
    /// Saved names that became placeholders.
    pub placeholders: Vec<String>,
}

impl Remap {
    /// Builds the table; every saved name must resolve (placeholders included).
    pub fn new(
        reg: &BlockRegistry,
        saved: &[String],
        placeholder_names: Vec<String>,
    ) -> Result<Self, SaveError> {
        let mut table = Vec::with_capacity(saved.len());
        for s in saved {
            let id = reg
                .parse_state(s)
                .map_err(|e| SaveError::Corrupt(format!("state `{s}` did not resolve: {e}")))?;
            table.push(id);
        }
        Ok(Self {
            table,
            placeholders: placeholder_names,
        })
    }

    pub fn map(&self, saved: u16) -> BlockStateId {
        self.table
            .get(saved as usize)
            .copied()
            .unwrap_or(BlockStateId::AIR)
    }
}

/// Builds the block registry for a world: content blocks plus placeholders for saved states the
/// content no longer has, and the remap from saved ids.
pub fn registry_for_save(
    content_defs: Vec<(ResourceLocation, BlockDef)>,
    saved: &[String],
) -> Result<(BlockRegistry, Remap), SaveError> {
    let base = BlockRegistry::build(content_defs.clone())
        .map_err(|e| SaveError::Content(e.to_string()))?;
    let known = |s: &str| base.parse_state(s).is_ok();
    let extra = placeholders(&known, saved)?;
    let placeholder_names: Vec<String> = saved.iter().filter(|s| !known(s)).cloned().collect();
    if !placeholder_names.is_empty() {
        log::warn!(
            "{} saved block states are no longer defined; keeping them as unknown placeholders",
            placeholder_names.len()
        );
    }
    let reg = if extra.is_empty() {
        base
    } else {
        let mut defs = content_defs;
        defs.extend(extra);
        BlockRegistry::build(defs).map_err(|e| SaveError::Content(e.to_string()))?
    };
    let remap = Remap::new(&reg, saved, placeholder_names)?;
    Ok((reg, remap))
}
