//! V2-12 (v2 §11.4): pottery in a running world. A pot is coiled of clay and set to dry; dried
//! pots are fired in a campfire kept fed for hours — earthenware clay comes out as pottery, a pot
//! of kaolin, which needs far more heat than an open fire gives, comes out still clay, under-fired;
//! a fired pot holds water; unfired clay left in the rain slumps back to clay.

mod common;

use common::{World, temp};
use hearth_env::weather::WeatherHold;
use hearth_math::BlockPos;
use hearth_protocol::{AimAt, ToServer};

fn weather(w: &mut World, rain_mm_h: f64) {
    w.server.send(ToServer::HoldWeather(Some(WeatherHold {
        temperature_c: Some(20.0),
        humidity: Some(0.5),
        precip_mm_h: Some(rain_mm_h),
        wind_speed_m_s: Some(1.0),
        ..WeatherHold::default()
    })));
    w.run(2);
}

/// Open ground beside the player to build on (the top solid block of a nearby column).
fn site(w: &World) -> BlockPos {
    let ground = w.ground();
    [(2, 0), (-2, 0), (0, 2), (0, -2), (2, 2), (-2, -2)]
        .iter()
        .find_map(|(dx, dz)| {
            let mut p = BlockPos::new(ground.x + dx, ground.y + 2, ground.z + dz);
            for _ in 0..5 {
                let solid = w
                    .mirror
                    .block(p)
                    .is_some_and(|s| !w.reg.collision_shape(s).is_empty());
                if solid {
                    return Some(p);
                }
                p = p.down();
            }
            None
        })
        .expect("ground to build on")
}

/// The things lying within a few metres of a place: their ids.
fn lying_near(w: &World, at: BlockPos) -> Vec<String> {
    w.lying
        .iter()
        .filter(|i| {
            let d = glam::DVec3::from_array(i.pos)
                - glam::DVec3::new(at.x as f64 + 0.5, at.y as f64, at.z as f64 + 0.5);
            d.length() < 3.0
        })
        .map(|i| i.stack.id.clone())
        .collect()
}

#[test]
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn pots_fire_in_their_range_and_under_fired_clay_stays_clay() {
    let dir = temp("pottery");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 11);
    weather(&mut w, 0.0);

    // A pot coiled of clay (some slump in a novice's hands), set to dry for two days.
    let mut shaped = false;
    for _ in 0..12 {
        w.give("hearth:lump/earthenware_clay", 5);
        let (done, words) = w.act("shape_pot", AimAt::Nothing);
        println!("shaping: {words}");
        if done {
            shaped = true;
            break;
        }
    }
    assert!(shaped, "a pot shaped");
    assert_eq!(w.has("hearth:wet_pot/earthenware_clay"), 1);
    let drying = site(&w);
    let (done, words) = w.act(
        "dry_pot",
        AimAt::Block {
            pos: drying,
            top: true,
        },
    );
    assert!(done, "{words}");
    w.server.send(ToServer::SkipHours(50.0));
    w.run(40);
    assert!(
        w.until(30.0, |w| {
            lying_near(w, drying)
                .iter()
                .any(|id| id == "hearth:greenware_pot/earthenware_clay")
                || w.acted.iter().any(|(p, d, _)| p == "hearth:dry_pot" && !*d)
        }),
        "dried (or cracked): {:?}",
        lying_near(&w, drying)
    );

    // A campfire, lit and kept fed for three hours, two dried pots in it: one of earthenware
    // clay, one of kaolin.
    let at = site(&w);
    w.give("hearth:stick/oak_wood", 4);
    w.give("hearth:handful/dry_grass", 2);
    let (done, words) = w.act("build_campfire", AimAt::Block { pos: at, top: true });
    assert!(done, "laid: {words}");
    let hearth = at.up();
    assert!(w.until(10.0, |w| w.block(hearth).as_deref() == Some("campfire")));
    loop {
        w.give("hearth:ember", 1);
        let (done, words) = w.act(
            "light_fire",
            AimAt::Block {
                pos: hearth,
                top: false,
            },
        );
        if done {
            break;
        }
        assert!(words.contains("ember dies"), "{words}");
    }
    let aim = AimAt::Block {
        pos: hearth,
        top: false,
    };
    for _ in 0..4 {
        w.give("hearth:stick/oak_wood", 1);
        w.act("feed_fire", aim);
    }
    for clay in ["earthenware_clay", "kaolin"] {
        w.give(&format!("hearth:greenware_pot/{clay}"), 1);
        let (done, words) = w.act("fire_pot_open", aim);
        assert!(done, "set in the fire: {words}");
    }
    let n = w.acted.len();
    let fired = |w: &World| {
        w.acted[n..]
            .iter()
            .filter(|(p, _, _)| p == "hearth:fire_pot_open")
            .count()
    };
    for _ in 0..16 {
        w.wait_hours(0.25);
        for _ in 0..2 {
            w.give("hearth:stick/oak_wood", 1);
            w.act("feed_fire", aim);
        }
        if fired(&w) >= 1
            && lying_near(&w, hearth)
                .iter()
                .any(|id| id.contains("pot/earthenware"))
        {
            break;
        }
    }
    w.wait_hours(0.5);
    let near = lying_near(&w, hearth);
    let told: Vec<String> = w.acted[n..]
        .iter()
        .filter(|(p, _, _)| p == "hearth:fire_pot_open")
        .map(|(_, _, words)| words.clone())
        .collect();
    println!("by the fire: {near:?}; told: {told:?}");
    // The kaolin never got hot enough: still clay, said so.
    assert!(
        near.iter().any(|id| id == "hearth:greenware_pot/kaolin"),
        "the kaolin pot is still clay: {near:?}"
    );
    assert!(
        told.iter().any(|t| t.contains("never got hot enough")),
        "{told:?}"
    );
    // The earthenware fired — or cracked in the uneven heat, as open-fired pots do; fired
    // again until one comes out.
    let mut pot = near.iter().any(|id| id == "hearth:pot/earthenware");
    for _ in 0..4 {
        if pot {
            break;
        }
        w.give("hearth:greenware_pot/earthenware_clay", 1);
        let (done, words) = w.act("fire_pot_open", aim);
        assert!(done, "{words}");
        for _ in 0..12 {
            w.wait_hours(0.25);
            for _ in 0..2 {
                w.give("hearth:stick/oak_wood", 1);
                w.act("feed_fire", aim);
            }
        }
        pot = lying_near(&w, hearth)
            .iter()
            .any(|id| id == "hearth:pot/earthenware");
    }
    assert!(
        pot,
        "an earthenware pot fired: {:?}",
        lying_near(&w, hearth)
    );

    // Unfired clay left in the rain slumps back to clay.
    w.give("hearth:greenware_pot/earthenware_clay", 1);
    w.put_down_all();
    weather(&mut w, 4.0);
    w.wait_hours(1.0);
    weather(&mut w, 0.0);
    let feet = w.ground();
    assert!(
        w.acted
            .iter()
            .any(|(_, _, t)| t.contains("slumps back to clay")),
        "rain undid the unfired pot: {:?}",
        lying_near(&w, feet)
    );
}
