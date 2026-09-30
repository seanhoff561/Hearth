//! Seasonal cover on loaded terrain follows the calendar and is fully reversible: winter lays
//! snow on a summer landscape, and the next summer gives back exactly the blocks it started
//! with (buried plants included).

use glam::DVec3;
use hearth::scene::LocalWorld;
use hearth::season_cover::{self, column_cover};
use hearth_math::{BlockPos, ColumnPos, CubePos, PlanetSize};

const SUMMER: f64 = 0.4;
const WINTER: f64 = 0.9;

/// A land column with deep winter snow and none in summer (northern hemisphere).
fn snowy_place(lw: &LocalWorld) -> Option<ColumnPos> {
    let planet = *lw.map.planet();
    let terrain = lw.terrain();
    let c = planet.circumference();
    for zi in 1..64 {
        let z = -(planet.pole_edge_z() * zi as f64 / 64.0) as i32;
        for xi in 0..32 {
            let x = c * xi / 32;
            let s = terrain.sample(x, z);
            if s.is_underwater() || s.ocean || s.lake {
                continue;
            }
            let col = ColumnPos::new(x >> 4, z >> 4);
            let (winter, _, _) = column_cover(&lw.generator, col, WINTER);
            let (summer, _, _) = column_cover(&lw.generator, col, SUMMER);
            if winter > 0.4 && summer == 0.0 {
                return Some(col);
            }
        }
    }
    None
}

fn snapshot(lw: &LocalWorld, cubes: &[CubePos]) -> Vec<(BlockPos, hearth_world::BlockStateId)> {
    let mut out = Vec::new();
    for &c in cubes {
        let min = c.min_block();
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let p = BlockPos::new(min.x + x, min.y + y, min.z + z);
                    if let Some(b) = lw.map.block(p) {
                        out.push((p, b));
                    }
                }
            }
        }
    }
    out
}

#[test]
fn winter_comes_and_goes() {
    let mut lw = LocalWorld::create(11, PlanetSize::Tiny, 256, None).expect("world");
    let col = snowy_place(&lw).expect("a place with snowy winters");
    let (x0, z0) = col.min_block_xz();
    let (x, z) = (x0 as f64 + 8.0, z0 as f64 + 8.0);
    let centre = DVec3::new(x, lw.surface_y(x, z), z);
    let cubes = lw.load_area(centre, 1, 1, Some(SUMMER));
    let cols: Vec<ColumnPos> = {
        let mut v: Vec<ColumnPos> = cubes.iter().map(|c| c.column()).collect();
        v.sort_unstable_by_key(|c| (c.x, c.z));
        v.dedup();
        v
    };
    let summer = snapshot(&lw, &cubes);
    let snow = |lw: &LocalWorld| {
        snapshot(lw, &cubes)
            .iter()
            .filter(|(_, b)| lw.cover.snow_layers.contains(b))
            .count()
    };
    assert_eq!(snow(&lw), 0, "no snow in summer");

    let changed = season_cover::refresh(
        &mut lw.map,
        &lw.reg,
        &lw.cover,
        &lw.generator,
        &mut lw.buried,
        &cols,
        WINTER,
    );
    let winter_snow = snow(&lw);
    assert!(winter_snow > 100, "winter snow on {winter_snow} blocks");
    assert!(changed.len() >= winter_snow);

    let changed_back = season_cover::refresh(
        &mut lw.map,
        &lw.reg,
        &lw.cover,
        &lw.generator,
        &mut lw.buried,
        &cols,
        SUMMER,
    );
    assert_eq!(
        changed_back.len(),
        changed.len(),
        "the thaw undoes every change"
    );
    assert_eq!(
        snapshot(&lw, &cubes),
        summer,
        "the summer landscape is back, block for block"
    );
}
