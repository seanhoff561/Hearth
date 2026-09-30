//! Loading saves written by earlier formats (v2 §3.5, V2-0 acceptance).
//!
//! `fixtures/format2_world` is a world in save format 2 (the first v2 format, before the
//! deliberate format change to 3). Its cubes use `hearth:coal_ore`, which V2-0 removed from
//! the content: it must load as an "unknown" placeholder and be written back unchanged.
//! `fixtures/format1_world` is a v1 world, which must be refused with a clear message.

use std::path::{Path, PathBuf};

use hearth_math::{CubePos, LocalPos};
use hearth_save::states::registry_for_save;
use hearth_save::{FORMAT, SaveError, WorldDir};
use hearth_world::{BlockStateId, Cube};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let p = e.path();
        let dest = to.join(e.file_name());
        if p.is_dir() {
            copy_dir(&p, &dest);
        } else {
            std::fs::copy(&p, &dest).unwrap();
        }
    }
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("hearth_save_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

const STONE: LocalPos = LocalPos::new(0, 0, 0);
const ORE: LocalPos = LocalPos::new(1, 2, 3);
const GRASS: LocalPos = LocalPos::new(4, 5, 6);
const LOG: LocalPos = LocalPos::new(7, 8, 9);

/// Regenerates the format-2 fixture's region file (the level.json is hand-written in the
/// format-2 layout). Run with `cargo test -p hearth_save -- --ignored write_format2_fixture`.
#[test]
#[ignore]
fn write_format2_fixture() {
    let dir = fixtures().join("format2_world");
    let mut store = hearth_save::RegionStore::new(dir.join("region"));
    // Saved ids index level.json's block_states: 0 air, 1 stone, 2 coal_ore, 3 grass, 4 log.
    let mut cube = Cube::filled(BlockStateId(1));
    cube.set(ORE, BlockStateId(2));
    cube.set(GRASS, BlockStateId(3));
    cube.set(LOG, BlockStateId(4));
    store.write_cube(CubePos::new(0, 0, 0), &cube).unwrap();
    store
        .write_cube(CubePos::new(0, -1, 0), &Cube::filled(BlockStateId(1)))
        .unwrap();
    store.flush().unwrap();
}

#[test]
fn v2_0_world_loads_after_the_format_change() {
    let root = scratch("fmt2");
    copy_dir(&fixtures().join("format2_world"), &root);
    let (dir, meta, report) = WorldDir::open(&root).expect("format 2 opens");
    assert_eq!((report.from, report.to), (2, FORMAT), "migrated");
    assert_eq!(meta.format, FORMAT);
    assert_eq!(meta.settings.planet.seed, 42);
    assert_eq!(meta.clock.ticks, 123_456);
    assert_eq!(meta.settings.era, "hearth:wild_earth");

    let defs =
        hearth_world::datapack::load_block_defs(&[hearth_world::datapack::builtin_pack_dir()])
            .unwrap();
    let (reg, remap) = registry_for_save(defs.clone(), &meta.block_states).unwrap();
    // Both blocks were removed from the base content since (coal ore in V2-0, the generic
    // stone when real rock types replaced it in V2-2): they come back as named placeholders.
    let mut placeholders = remap.placeholders.clone();
    placeholders.sort();
    assert_eq!(
        placeholders,
        vec!["hearth:coal_ore".to_string(), "hearth:stone".to_string()]
    );
    let mut store = dir.regions();
    let cube = store
        .read_cube(CubePos::new(0, 0, 0), &|i| remap.map(i))
        .unwrap()
        .expect("cube saved");
    assert_eq!(reg.state_string(cube.get(STONE)), "hearth:stone");
    assert_eq!(
        reg.state_string(cube.get(ORE)),
        "hearth:coal_ore",
        "unknown content keeps its name"
    );
    assert_eq!(
        reg.state_string(cube.get(GRASS)),
        "hearth:grass_block[snowy=false]"
    );
    assert_eq!(reg.state_string(cube.get(LOG)), "hearth:oak_log[axis=x]");
    assert!(
        reg.is_opaque(cube.get(ORE)),
        "placeholders are solid, visible blocks"
    );

    // Save in the current format and reopen: nothing is lost.
    let mut meta = meta;
    meta.block_states = reg.state_names();
    dir.save_meta(&meta).unwrap();
    store.write_cube(CubePos::new(0, 0, 0), &cube).unwrap();
    store.flush().unwrap();
    let (dir, meta, report) = WorldDir::open(&root).unwrap();
    assert!(!report.migrated(), "already current");
    let (reg, remap) = registry_for_save(defs, &meta.block_states).unwrap();
    let cube = dir
        .regions()
        .read_cube(CubePos::new(0, 0, 0), &|i| remap.map(i))
        .unwrap()
        .unwrap();
    assert_eq!(reg.state_string(cube.get(ORE)), "hearth:coal_ore");
    assert_eq!(reg.state_string(cube.get(LOG)), "hearth:oak_log[axis=x]");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn v1_world_is_refused_with_a_clear_message() {
    let err = WorldDir::open(&fixtures().join("format1_world")).unwrap_err();
    match err {
        SaveError::Incompatible(msg) => {
            assert!(msg.contains("earlier, incompatible version"), "{msg}");
            assert!(msg.contains("format 1"), "{msg}");
        }
        other => panic!("expected an incompatibility error, got {other}"),
    }
}
