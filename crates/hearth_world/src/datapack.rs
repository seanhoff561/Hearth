//! Loading block definitions from data packs.
//!
//! Layout: `<pack>/data/<namespace>/blocks/*.json`. Each file maps block names to definitions.
//! Files whose name starts with `_` hold *templates* (partial definitions); a block entry with
//! `"template": "<name>"` starts from that template (from the same namespace, or
//! `ns:name`) and overrides its keys. Later packs override earlier ones per block id.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use hearth_core::ResourceLocation;
use serde_json::{Map, Value};

use crate::block::BlockDef;

/// Errors while loading a data pack.
#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path}: invalid JSON: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("{path}: block {block}: {message}")]
    Block {
        path: PathBuf,
        block: String,
        message: String,
    },
}

/// Lists the namespace directories of a pack root (`<root>/data/<ns>` or `<root>/<ns>` if the
/// root already is a `data` folder).
pub fn namespaces(root: &Path) -> Vec<(String, PathBuf)> {
    let data = if root.join("data").is_dir() {
        root.join("data")
    } else {
        root.to_path_buf()
    };
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&data) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir()
                && let Some(name) = p.file_name().and_then(|n| n.to_str())
            {
                out.push((name.to_owned(), p));
            }
        }
    }
    out.sort();
    out
}

fn read_json(path: &Path) -> Result<Map<String, Value>, DataError> {
    let text = std::fs::read_to_string(path).map_err(|source| DataError::Io {
        path: path.to_owned(),
        source,
    })?;
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(m)) => Ok(m),
        Ok(_) => Err(DataError::Block {
            path: path.to_owned(),
            block: String::new(),
            message: "top level must be an object".into(),
        }),
        Err(source) => Err(DataError::Json {
            path: path.to_owned(),
            source,
        }),
    }
}

fn json_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "json"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

/// Block objects for the natural blocks the packs' content defines (one per rock type, see
/// `hearth_content::generate::natural_blocks`), in the same form as a pack's JSON so templates
/// apply and a pack can override any of them by name.
fn generated_blocks(packs: &[PathBuf]) -> Vec<(ResourceLocation, Map<String, Value>)> {
    let (Some(content), _) = hearth_content::Content::load(packs) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for b in hearth_content::generate::natural_blocks(&content) {
        let Ok(id) = ResourceLocation::parse(&b.id) else {
            continue;
        };
        let template = match b.kind {
            hearth_content::generate::NaturalKind::Rock => "hearth:rock",
            hearth_content::generate::NaturalKind::Soil => "hearth:soil",
            hearth_content::generate::NaturalKind::Loose => "hearth:falling_soil",
            hearth_content::generate::NaturalKind::Cobbles => "hearth:loose_stones",
        };
        let [r, g, bl] = b.map_color;
        let mut obj = Map::new();
        obj.insert("template".into(), template.into());
        obj.insert("hardness".into(), Value::from(b.hardness as f64));
        obj.insert("resistance".into(), Value::from(b.hardness as f64 * 4.0));
        obj.insert(
            "map_color".into(),
            format!("#{r:02x}{g:02x}{bl:02x}").into(),
        );
        obj.insert("material".into(), b.material.into());
        out.push((id, obj));
    }
    out
}

/// Loads all block definitions from the given pack roots, in priority order (later packs
/// override earlier ones), plus the natural blocks generated from the packs' content (which
/// any pack's JSON overrides). Returns definitions sorted by id for deterministic
/// registration.
pub fn load_block_defs(packs: &[PathBuf]) -> Result<Vec<(ResourceLocation, BlockDef)>, DataError> {
    // Templates are global across packs: "ns:name" → object.
    let mut templates: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    let mut raw: BTreeMap<ResourceLocation, (PathBuf, Map<String, Value>)> = BTreeMap::new();
    for (id, obj) in generated_blocks(packs) {
        raw.insert(id, (PathBuf::from("<generated from content>"), obj));
    }
    for pack in packs {
        for (ns, dir) in namespaces(pack) {
            let blocks_dir = dir.join("blocks");
            for file in json_files(&blocks_dir) {
                let is_template = file
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('_'));
                let map = read_json(&file)?;
                for (name, value) in map {
                    let Value::Object(obj) = value else {
                        return Err(DataError::Block {
                            path: file.clone(),
                            block: name,
                            message: "definition must be an object".into(),
                        });
                    };
                    if is_template {
                        templates.insert(format!("{ns}:{name}"), obj);
                    } else {
                        let id =
                            ResourceLocation::new(&ns, &name).map_err(|e| DataError::Block {
                                path: file.clone(),
                                block: name.clone(),
                                message: e.to_string(),
                            })?;
                        raw.insert(id, (file.clone(), obj));
                    }
                }
            }
        }
    }
    let mut out = Vec::with_capacity(raw.len());
    for (id, (path, obj)) in raw {
        let merged = resolve_template(&id, obj, &templates, &path, 0)?;
        let def: BlockDef =
            serde_json::from_value(Value::Object(merged)).map_err(|e| DataError::Block {
                path: path.clone(),
                block: id.to_string(),
                message: e.to_string(),
            })?;
        out.push((id, def));
    }
    Ok(out)
}

fn resolve_template(
    id: &ResourceLocation,
    mut obj: Map<String, Value>,
    templates: &BTreeMap<String, Map<String, Value>>,
    path: &Path,
    depth: u32,
) -> Result<Map<String, Value>, DataError> {
    let Some(t) = obj.remove("template") else {
        return Ok(obj);
    };
    let err = |m: String| DataError::Block {
        path: path.to_owned(),
        block: id.to_string(),
        message: m,
    };
    if depth > 8 {
        return Err(err("template nesting too deep".into()));
    }
    let Value::String(name) = t else {
        return Err(err("template must be a string".into()));
    };
    let key = if name.contains(':') {
        name.clone()
    } else {
        format!("{}:{name}", id.namespace())
    };
    let base = templates
        .get(&key)
        .ok_or_else(|| err(format!("unknown template {key}")))?
        .clone();
    let mut base = resolve_template(id, base, templates, path, depth + 1)?;
    for (k, v) in obj {
        base.insert(k, v);
    }
    Ok(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, rel: &str, text: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    #[test]
    fn templates_and_overrides() {
        let root = std::env::temp_dir().join(format!("hearth-dp-{}", std::process::id()));
        let a = root.join("base");
        let b = root.join("mod");
        write(
            &a,
            "data/hearth/blocks/_templates.json",
            r#"{"rock": {"hardness": 1.5, "sound": "stone"}}"#,
        );
        write(
            &a,
            "data/hearth/blocks/terrain.json",
            r#"{"stone": {"template": "rock"}, "dirt": {"hardness": 0.5}}"#,
        );
        write(
            &b,
            "data/hearth/blocks/over.json",
            r#"{"dirt": {"hardness": 0.9}}"#,
        );
        write(
            &b,
            "data/mymod/blocks/ores.json",
            r#"{"ruby_ore": {"template": "hearth:rock", "hardness": 3.0}}"#,
        );
        let defs = load_block_defs(&[a, b]).unwrap();
        let get = |n: &str| {
            defs.iter()
                .find(|(id, _)| id.as_str() == n)
                .map(|(_, d)| d.clone())
                .unwrap()
        };
        assert_eq!(get("hearth:stone").hardness, 1.5);
        assert_eq!(get("hearth:stone").sound, "stone");
        assert_eq!(get("hearth:dirt").hardness, 0.9, "later pack overrides");
        assert_eq!(get("mymod:ruby_ore").hardness, 3.0);
        assert_eq!(
            get("mymod:ruby_ore").sound,
            "stone",
            "cross-namespace template"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn unknown_template_is_an_error() {
        let root = std::env::temp_dir().join(format!("hearth-dp2-{}", std::process::id()));
        write(
            &root,
            "data/hearth/blocks/x.json",
            r#"{"a": {"template": "missing"}}"#,
        );
        assert!(load_block_defs(std::slice::from_ref(&root)).is_err());
        std::fs::remove_dir_all(&root).ok();
    }
}

/// Path of the repository's base data pack (`<repo>/data`), for tests and tools.
pub fn builtin_pack_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

/// Loads the base game's blocks from the repository data pack and builds the registry.
pub fn load_builtin_registry() -> Result<crate::block::BlockRegistry, String> {
    let defs = load_block_defs(&[builtin_pack_dir()]).map_err(|e| e.to_string())?;
    crate::block::BlockRegistry::build(defs).map_err(|e| e.to_string())
}

/// A small pack of blocks that engine tests rely on (torches, stairs, glass, slabs, doors…),
/// kept apart from the game content so content changes can't break mechanism tests.
pub fn engine_test_pack_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata")
}

/// The base pack plus the engine test pack. For tests.
pub fn load_test_registry() -> Result<crate::block::BlockRegistry, String> {
    let defs = load_block_defs(&[builtin_pack_dir(), engine_test_pack_dir()])
        .map_err(|e| e.to_string())?;
    crate::block::BlockRegistry::build(defs).map_err(|e| e.to_string())
}

#[cfg(test)]
mod builtin_tests {
    use super::*;
    use crate::block::StateFlags;

    #[test]
    fn base_pack_loads_and_builds() {
        let reg = load_builtin_registry().expect("base data pack is valid");
        assert!(reg.block_count() > 50, "{} blocks", reg.block_count());
        assert!(reg.state_count() < 16_000, "{} states", reg.state_count());
        for name in [
            "granite",
            "chalk",
            "bituminous_coal",
            "grass_block[snowy=false]",
            "oak_leaves[distance=3,persistent=false,waterlogged=false]",
            "water[level=0]",
            "tall_grass[half=upper]",
            "seagrass",
        ] {
            reg.parse_state(name)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        for dropped in [
            "deepslate_diamond_ore",
            "coal_ore",
            "crafting_table",
            "oak_planks",
            "stone",
            "deepslate",
        ] {
            assert!(
                reg.parse_state(dropped).is_err(),
                "{dropped} was removed with v1 content"
            );
        }
        let seagrass = reg.default_state("seagrass");
        assert!(reg.has(seagrass, StateFlags::WATER));
        let ice = reg.default_state("ice");
        assert!(!reg.is_opaque(ice));
        // Rock blocks are generated from the content's rock types, with their materials.
        let granite = reg.default_state("granite");
        assert!(reg.is_opaque(granite));
        assert_eq!(
            reg.block_of(granite).def.material.as_deref(),
            Some("hearth:granite")
        );
        let chalk = reg.block_of(reg.default_state("chalk"));
        assert!(
            chalk.def.hardness < reg.block_of(granite).def.hardness,
            "chalk is soft"
        );
    }

    #[test]
    fn engine_test_pack_adds_mechanism_blocks() {
        let reg = load_test_registry().expect("test pack is valid");
        reg.parse_state("oak_stairs[facing=east,half=top,shape=straight,waterlogged=true]")
            .expect("stairs");
        let torch = reg.default_state("torch");
        assert_eq!(reg.light_emission(torch), 14);
        let lit = reg.parse_state("furnace[lit=true]").unwrap();
        assert_eq!(reg.light_emission(lit), 13);
    }
}
