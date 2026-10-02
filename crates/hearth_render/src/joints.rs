//! Joints between construction pieces (V2-9): a member that reaches the face of its block toward
//! a neighbouring piece is carried on into that piece's block until it meets the piece there — a
//! beam into the post it rests beside, a panel into the post it is lashed to, a wall into the
//! wall at a corner, a post up under the beam over it, a wall up under the roof, the edge of a
//! roof onto the beam or the gable it rests on. Pieces lie on their blocks' middle lines; these
//! carried-on ends are what make a frame of them one structure rather than pieces in a grid.
//! They are only drawn: what stands and what bears is reckoned by blocks.
//!
//! A member carries on as far as it must to meet what is there. Straight on along its own
//! length (a post up, a beam along, a wall along or up) that may be the whole block; anything
//! else (a roof's edge, a beam's top, a floor's edge) at most to the middle, so that nothing
//! grows across a block it only touches. It never carries on where nothing in the block lies
//! across its way.

use hearth_math::Direction;
use smallvec::SmallVec;

/// Boxes in block-local space (0..1): `[min x, min y, min z, max x, max y, max z]`.
pub type Box6 = [f32; 6];

/// A box a neighbour's member is carried on by into a block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Carried {
    pub b: Box6,
    /// Its far end lies against the block's own piece (that face is hidden).
    pub met: bool,
}

const EPS: f32 = 1e-4;

/// The axis across a direction's faces (0 x, 1 y, 2 z) and whether the neighbour lies on the
/// far (+) side.
fn axis(d: Direction) -> (usize, bool) {
    match d {
        Direction::West => (0, false),
        Direction::East => (0, true),
        Direction::Down => (1, false),
        Direction::Up => (1, true),
        Direction::North => (2, false),
        Direction::South => (2, true),
    }
}

/// The boxes by which the members of the piece toward `d` (`nb`, in its own block's space) are
/// carried on into this block, to meet this block's piece (`own`).
pub fn carried(own: &[Box6], nb: &[Box6], d: Direction) -> SmallVec<[Carried; 8]> {
    let (n, far) = axis(d);
    let (u, v) = match n {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };
    // How far into this block an own box lies from the shared face.
    let depth_of = |o: &Box6| if far { 1.0 - o[n + 3] } else { o[n] };
    let mut out = SmallVec::new();
    for b in nb {
        // A member reaching the face it shares with this block.
        let touches = if far {
            b[n] <= EPS
        } else {
            b[n + 3] >= 1.0 - EPS
        };
        if !touches {
            continue;
        }
        let (u0, u1, v0, v1) = (b[u], b[u + 3], b[v], b[v + 3]);
        // Straight on along its length, as far as it must; anything else, to the middle.
        let straight = b[n] <= EPS && b[n + 3] >= 1.0 - EPS;
        let most = if straight { 1.0 } else { 0.5 };
        // This block's own boxes across its way.
        let across: SmallVec<[&Box6; 8]> = own
            .iter()
            .filter(|o| {
                o[u] < u1 - EPS && o[u + 3] > u0 + EPS && o[v] < v1 - EPS && o[v + 3] > v0 + EPS
            })
            .collect();
        if across.is_empty() {
            continue;
        }
        let nearest = across
            .iter()
            .map(|o| depth_of(o))
            .fold(f32::INFINITY, f32::min);
        // Its section cut where the boxes across its way begin and end, each part carried on
        // to the nearest of them behind it (to the nearest of all where none is).
        let mut us: SmallVec<[f32; 8]> = SmallVec::from_slice(&[u0, u1]);
        let mut vs: SmallVec<[f32; 8]> = SmallVec::from_slice(&[v0, v1]);
        for o in &across {
            for (cuts, lo, hi, c0, c1) in [
                (&mut us, u0, u1, o[u], o[u + 3]),
                (&mut vs, v0, v1, o[v], o[v + 3]),
            ] {
                for c in [c0, c1] {
                    if c > lo + EPS && c < hi - EPS {
                        cuts.push(c);
                    }
                }
            }
        }
        us.sort_by(f32::total_cmp);
        us.dedup_by(|a, b| (*a - *b).abs() < EPS);
        vs.sort_by(f32::total_cmp);
        vs.dedup_by(|a, b| (*a - *b).abs() < EPS);
        for i in 0..us.len() - 1 {
            for j in 0..vs.len() - 1 {
                let (cu, cv) = ((us[i] + us[i + 1]) / 2.0, (vs[j] + vs[j + 1]) / 2.0);
                let behind = across
                    .iter()
                    .filter(|o| o[u] <= cu && cu <= o[u + 3] && o[v] <= cv && cv <= o[v + 3])
                    .map(|o| depth_of(o))
                    .fold(f32::INFINITY, f32::min);
                let met = behind.is_finite();
                let depth = if met { behind } else { nearest };
                if depth <= EPS || depth > most + EPS {
                    continue;
                }
                let mut c = [0.0f32; 6];
                c[u] = us[i];
                c[u + 3] = us[i + 1];
                c[v] = vs[j];
                c[v + 3] = vs[j + 1];
                if far {
                    c[n] = 1.0 - depth;
                    c[n + 3] = 1.0;
                } else {
                    c[n] = 0.0;
                    c[n + 3] = depth;
                }
                out.push(Carried { b: c, met });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: f32 = 1.0 / 16.0;

    fn post(t: f32) -> Box6 {
        let (a, b) = (0.5 - t * P / 2.0, 0.5 + t * P / 2.0);
        [a, 0.0, a, b, 1.0, b]
    }

    /// A beam along x at the top of its block.
    fn beam_x(t: f32) -> Box6 {
        let (a, b) = (0.5 - t * P / 2.0, 0.5 + t * P / 2.0);
        [0.0, 1.0 - t * P, a, 1.0, 1.0, b]
    }

    /// A panel across x (its faces north and south).
    fn panel_x(t: f32) -> Box6 {
        let (a, b) = (0.5 - t * P / 2.0, 0.5 + t * P / 2.0);
        [0.0, 0.0, a, 1.0, 1.0, b]
    }

    /// A roof rising toward the south in eight steps.
    fn roof_south(t: f32) -> Vec<Box6> {
        (0..8)
            .map(|k| {
                let k = k as f32;
                let z0 = 2.0 * k * P;
                let y0 = 2.0 * k * P;
                [
                    0.0,
                    y0,
                    z0,
                    1.0,
                    (y0 + t.max(2.0) * P).min(1.0),
                    z0 + 2.0 * P,
                ]
            })
            .collect()
    }

    #[test]
    fn a_beam_beside_a_post_runs_into_it() {
        // The beam to the east of the post: carried west into the post's block to its face.
        let c = carried(&[post(4.0)], &[beam_x(4.0)], Direction::East);
        assert!(!c.is_empty());
        for k in &c {
            assert!((k.b[0] - (0.5 + 2.0 * P)).abs() < 1e-5, "{:?}", k.b);
            assert!((k.b[3] - 1.0).abs() < 1e-5);
            assert!(k.b[1] >= 1.0 - 4.0 * P - 1e-5, "at the top: {:?}", k.b);
        }
        // Thinner than the beam, the post stops all of it at its face.
        let c = carried(&[post(2.0)], &[beam_x(4.0)], Direction::East);
        assert!(c.iter().all(|k| (k.b[0] - (0.5 + P)).abs() < 1e-5), "{c:?}");
        // The post reaches no face of its block beside: nothing goes into the beam's.
        assert!(carried(&[beam_x(4.0)], &[post(4.0)], Direction::West).is_empty());
    }

    #[test]
    fn a_post_under_a_beam_rises_to_it() {
        // The post below: carried up into the beam's block to the beam's underside.
        let c = carried(&[beam_x(2.0)], &[post(2.0)], Direction::Down);
        assert_eq!(c.len(), 1, "{c:?}");
        assert!((c[0].b[4] - (1.0 - 2.0 * P)).abs() < 1e-5, "{:?}", c[0].b);
        assert!(c[0].met);
    }

    #[test]
    fn walls_meet_at_a_corner_and_panels_at_posts() {
        // A wall running north-south south of a wall running east-west: carried north to its
        // south face.
        let ns = [0.5 - 2.5 * P, 0.0, 0.0, 0.5 + 2.5 * P, 1.0, 1.0];
        let c = carried(&[panel_x(5.0)], &[ns], Direction::South);
        assert!(!c.is_empty());
        assert!(
            c.iter().all(|k| (k.b[2] - (0.5 + 2.5 * P)).abs() < 1e-5),
            "{c:?}"
        );
        // A panel beside a post, the post thinner: the panel is carried to the post's face.
        let c = carried(&[post(2.0)], &[panel_x(5.0)], Direction::West);
        assert!(c.iter().all(|k| (k.b[3] - (0.5 - P)).abs() < 1e-5), "{c:?}");
    }

    #[test]
    fn a_roof_rests_on_its_ridge_beam_and_a_wall_rises_under_it() {
        // The roof north of the beam, rising toward it: its top edge carried south onto the
        // beam, no further than the middle.
        let c = carried(&[beam_x(2.0)], &roof_south(2.0), Direction::North);
        assert!(!c.is_empty(), "the roof's edge reaches the beam");
        assert!(c.iter().all(|k| k.b[5] <= 0.5 + 1e-5), "{c:?}");
        // A wall under a roof: carried up to the roof's underside, step by step.
        let c = carried(&roof_south(2.0), &[panel_x(2.0)], Direction::Down);
        assert!(!c.is_empty());
        assert!(c.iter().all(|k| k.met && k.b[4] <= 0.5 + 1e-5), "{c:?}");
    }

    #[test]
    fn nothing_grows_across_a_block_it_only_touches() {
        // Two roofs side by side down a slope: the lower edge of one is not carried across the
        // other's block.
        let c = carried(&roof_south(2.0), &roof_south(2.0), Direction::North);
        assert!(c.is_empty(), "{c:?}");
        // A whole block under a post: nothing grows about the post's foot.
        let c = carried(
            &[post(2.0)],
            &[[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]],
            Direction::Down,
        );
        assert!(c.is_empty(), "{c:?}");
    }
}
