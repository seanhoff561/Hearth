//! The player's blows on the server (E §3.2): begun on a click, struck at the end of their
//! wind-up along their path at whatever animal is there then (so one that moves off is
//! missed), and the things flung: thrown, or loosed from a bow. Other players aren't struck
//! yet: that waits for the PvP setting (Amendment R §3.6).

use glam::DVec3;
use hearth_body::BodyConfig;
use hearth_content::schema::item::Use;
use hearth_fauna::live::Live;
use hearth_fauna::species::Catalog;
use hearth_fauna::wound::Blow;
use hearth_items::{Carry, ItemKind, Items, Stack};
use hearth_player::Player;
use hearth_player::strike::{Attack, EYE_SHARE, Striking, Weapon};

use crate::workshop::Flight;

/// The thing in the hand a blow or a use is made with: the right's, else the left's (none:
/// the right hand, empty). Whether it is the right.
pub fn in_hand<'a>(carry: &'a Carry, items: &'a Items) -> (Option<&'a ItemKind>, bool) {
    match (&carry.right, &carry.left) {
        (Some(s), _) => (items.get(&s.id), true),
        (None, Some(s)) => (items.get(&s.id), false),
        _ => (None, true),
    }
}

/// The blow a click (or `kick`) makes with what is carried: what, with what, and whether with
/// the right hand (or foot).
pub fn chosen(carry: &Carry, items: &Items, kick: bool) -> (Attack, Weapon, bool) {
    if kick {
        return (Attack::Kick, Weapon::default(), true);
    }
    let (kind, right) = in_hand(carry, items);
    match kind.and_then(|k| Attack::of(k.primary).map(|a| (a, k))) {
        Some((a, k)) if a != Attack::Punch => (a, Weapon::of(k), right),
        // The fist, empty or holding a thing with no blow of its own.
        _ => (Attack::Punch, Weapon::default(), right),
    }
}

/// What a click at nothing does with the thing in hand (E §3.2).
#[derive(Debug, Clone, PartialEq)]
pub enum InHand {
    /// Food: a bite of it.
    Eat(hearth_items::Path),
    /// A water skin with water in it: a drink.
    Drink(hearth_items::Path),
    /// A thing a wound is treated with: the treatment (a process).
    Treat(String),
    /// A burning brand: held up, or lowered.
    HoldUp,
    /// A bow: drawn while the button is held (with an arrow carried).
    Draw,
    /// Anything else: a blow (with the fist, if nothing is held or what is held has no blow
    /// of its own).
    Blow,
}

/// What the thing in hand (the right's, else the left's) is for with nothing aimed at; `treats`
/// gives the treatment of a wound offered now that uses a kind of thing.
pub fn use_of(
    carry: &Carry,
    items: &Items,
    content: &hearth_content::Content,
    treats: &dyn Fn(&ItemKind) -> Option<String>,
) -> InHand {
    let (kind, right) = in_hand(carry, items);
    let hand = if right {
        hearth_items::Hand::Right
    } else {
        hearth_items::Hand::Left
    };
    let path = hearth_items::Path::at(hearth_items::Root::Hand(hand));
    let (Some(kind), Some(stack)) = (kind, carry.get(&path)) else {
        return InHand::Blow;
    };
    if hearth_craft::food::bite_of(content, kind, stack).is_some() {
        return InHand::Eat(path);
    }
    if kind.container.is_some_and(|c| c.liquid_l > 0.0) && stack.liquid_l >= 0.1 {
        return InHand::Drink(path);
    }
    if let Some(id) = treats(kind) {
        return InHand::Treat(id);
    }
    match kind.primary {
        Some(Use::HoldUp) => InHand::HoldUp,
        Some(Use::Draw) => InHand::Draw,
        _ => InHand::Blow,
    }
}

/// Begins a blow along `dir` (`kick`, or with what is in the hand), if the body can act and
/// isn't striking already; it spends its stamina. Whether it began.
pub fn begin(player: &mut Player, items: &Items, cfg: &BodyConfig, dir: DVec3, kick: bool) -> bool {
    if player.striking.is_some() || !player.can_act(cfg) {
        return false;
    }
    let (attack, weapon, right) = chosen(&player.carry, items, kick);
    let all_out = (cfg.params.stamina.all_out_s as f64).max(1.0);
    let striking = Striking::new(attack, weapon, right, dir, player.body.stamina);
    player.body.stamina = (player.body.stamina - attack.effort_s(&weapon) / all_out).max(0.0);
    player.striking = Some(striking);
    true
}

/// What the blow under way meets as it strikes: the animal along its path, wounded and shoved;
/// in words (none for a miss).
pub fn land(
    player: &Player,
    items: &Items,
    cfg: &BodyConfig,
    live: &mut Live,
    cat: &Catalog,
    year_frac: f32,
) -> Option<String> {
    let s = player.striking?;
    let eye = player.mover.pos + DVec3::Y * (EYE_SHARE * cfg.height_m);
    let path = s
        .attack
        .path(&s.weapon, cfg.height_m as f32, eye, s.dir, s.right);
    let hit = live.hit_along(cat, &path, year_frac)?;
    // Moving as the path moves where it struck.
    let along = path[(hit.segment + 1).min(path.len() - 1)] - path[hit.segment];
    let l = s.landed();
    let blow = Blow {
        energy_j: l.energy_j,
        piercing: l.piercing,
        cutting: l.cutting,
        push: along.normalize_or(s.dir) * l.push.length(),
    };
    let what = match s.attack {
        Attack::Kick => "kick".to_owned(),
        Attack::Punch => "fist".to_owned(),
        _ => in_hand(&player.carry, items)
            .0
            .map_or_else(|| "blow".to_owned(), |k| k.name.clone()),
    };
    live.strike(cat, &hit, &blow, &what, player.mover.pos, year_frac)
        .map(|s| s.words)
}

/// What a thing flung strikes on its way: the animal it meets first, wounded by the energy it
/// carries there (and shoved by its momentum); in words, with where the thing comes to rest
/// (where it struck, or where its flight ends).
pub fn fly(
    f: &Flight,
    items: &Items,
    live: &mut Live,
    cat: &Catalog,
    from: DVec3,
    year_frac: f32,
) -> (Option<String>, DVec3) {
    let end = f.path.last().copied().unwrap_or(from);
    let Some(hit) = live.hit_along(cat, &f.path, year_frac) else {
        return (None, end);
    };
    let kind = items.get(&f.stack.id);
    let v = f.speed_at(hit.segment);
    let along = f.path[(hit.segment + 1).min(f.path.len() - 1)] - f.path[hit.segment];
    let blow = Blow {
        energy_j: (0.5 * f.mass * v * v) as f32,
        piercing: kind.and_then(|k| k.property("piercing")).unwrap_or(0.0),
        cutting: 0.0,
        push: along.normalize_or_zero() * f.mass * v,
    };
    let what = kind.map_or("thing".to_owned(), |k| k.name.clone());
    let words = live
        .strike(cat, &hit, &blow, &what, from, year_frac)
        .map(|s| s.words);
    (words, hit.at)
}

/// Whether a stack is an arrow.
pub fn is_arrow(items: &Items, s: &Stack) -> bool {
    items
        .get(&s.id)
        .is_some_and(|k| k.form.as_deref() == Some("hearth:arrow"))
}

/// The speed a bow of `draw_kg` drawn `drawn_s` seconds sends an arrow of `arrow_kg` (m/s):
/// half the work of the draw (a straight-limbed self bow drawn 0.7 m, stored as a triangle
/// under its draw-force line, and about half of it given to the arrow), a full draw taking a
/// second. A 25 kg self bow sends a 30 g arrow at some 50 m/s.
pub fn bow_speed(draw_kg: f32, drawn_s: f32, arrow_kg: f64) -> f64 {
    let drawn = (drawn_s as f64).clamp(0.0, 1.0);
    let stored_j = 0.5 * draw_kg as f64 * 9.81 * 0.7 * drawn;
    let given_j = 0.5 * stored_j;
    (2.0 * given_j / arrow_kg.max(0.005)).sqrt().min(70.0)
}

/// Whether the brand held up burns: a thing whose use is to be held up, in either hand.
pub fn brand_up(player: &Player, items: &Items) -> bool {
    player.held_up
        && [&player.carry.right, &player.carry.left]
            .into_iter()
            .flatten()
            .any(|s| {
                items
                    .get(&s.id)
                    .is_some_and(|k| k.primary == Some(Use::HoldUp))
            })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hearth_fauna::live::Stage;
    use hearth_fauna::rig::Rig;

    /// The eye of a person 1.75 m tall standing at the origin.
    const EYE: DVec3 = DVec3::new(0.0, EYE_SHARE * 1.75, 0.0);

    fn catalog() -> Catalog {
        Catalog::new(&hearth_content::Content::load_base())
    }

    /// A grown hind with the nearest of her body `near_m` before the person (along `toward`,
    /// which is level): side-on, or end-on with her back to the person. Where her middle is.
    fn hind(
        live: &mut Live,
        cat: &Catalog,
        toward: DVec3,
        near_m: f64,
        side_on: bool,
    ) -> (u64, DVec3) {
        let s = cat.index("red_deer").expect("red deer") as u16;
        let rig = Rig::of(&cat.species[s as usize], false);
        let away = toward.x.atan2(toward.z) as f32;
        let (half, yaw) = if side_on {
            (rig.torso.x, away + std::f32::consts::FRAC_PI_2)
        } else {
            (rig.torso.z, away)
        };
        let at = toward * (near_m + half as f64 * 0.5);
        let id = live.place(s, Stage::Adult, true, at, yaw);
        (id, at + DVec3::Y * rig.torso_y as f64)
    }

    /// Whether a blow looking along `look` (from aiming at her middle, turned `turn_deg` to the
    /// left) meets a hind whose nearest is `near_m` away along `toward` (side-on when not
    /// turned, else end-on).
    fn meets(
        cat: &Catalog,
        attack: Attack,
        w: &Weapon,
        right: bool,
        toward: DVec3,
        near_m: f64,
        turn_deg: f64,
    ) -> bool {
        let mut live = Live::new(3);
        let (_, middle) = hind(&mut live, cat, toward, near_m, turn_deg == 0.0);
        let aim = glam::DQuat::from_rotation_y(turn_deg.to_radians()) * (middle - EYE);
        let path = attack.path(w, 1.75, EYE, aim, right);
        live.hit_along(cat, &path, 0.5).is_some()
    }

    /// E §11: punches, swings, thrusts and kicks hit along their arcs and miss when they
    /// should.
    #[test]
    fn blows_meet_what_lies_along_their_arcs_and_miss_the_rest() {
        let cat = catalog();
        let fist = Weapon::default();
        let stick = Weapon {
            mass_kg: 0.4,
            length_m: 0.8,
            ..Weapon::default()
        };
        let spear = Weapon {
            mass_kg: 1.4,
            length_m: 2.2,
            piercing: 0.85,
            cutting: 0.0,
        };
        let ahead = DVec3::Z;
        // In reach, aimed at her: the fist, the foot, the stick and the spear all land.
        assert!(meets(&cat, Attack::Punch, &fist, true, ahead, 0.3, 0.0));
        assert!(meets(&cat, Attack::Kick, &fist, true, ahead, 0.4, 0.0));
        assert!(meets(&cat, Attack::Swing, &stick, true, ahead, 0.8, 0.0));
        assert!(meets(&cat, Attack::Strike, &stick, true, ahead, 0.6, 0.0));
        assert!(meets(&cat, Attack::Thrust, &spear, true, ahead, 2.0, 0.0));
        // Out of reach: two metres off, the fist and the foot fall short where the spear lands.
        assert!(!meets(&cat, Attack::Punch, &fist, true, ahead, 2.0, 0.0));
        assert!(!meets(&cat, Attack::Kick, &fist, true, ahead, 2.0, 0.0));
        assert!(!meets(&cat, Attack::Swing, &stick, true, ahead, 2.0, 0.0));
        // Behind the person: nothing reaches her.
        for (a, w) in [
            (Attack::Punch, fist),
            (Attack::Kick, fist),
            (Attack::Swing, stick),
            (Attack::Thrust, spear),
        ] {
            assert!(
                !meets(&cat, a, &w, true, -ahead, 0.4, 180.0),
                "{a:?} behind"
            );
        }
        // Off to the right of the look (turned 45° left of her, her back to the person): a
        // right-handed swing sweeps through her on its way across; a thrust along the look,
        // and a left-handed swing that ends before it comes round so far, miss her.
        assert!(meets(&cat, Attack::Swing, &stick, true, ahead, 0.6, 45.0));
        assert!(!meets(&cat, Attack::Thrust, &spear, true, ahead, 0.6, 45.0));
        assert!(!meets(&cat, Attack::Swing, &stick, false, ahead, 0.6, 70.0));
    }

    /// E §11: each primary use, and the blow each thing makes.
    #[test]
    fn each_thing_in_hand_has_its_use() {
        let content = hearth_content::Content::load_base();
        let items = Items::from_content(&content);
        let none = |_: &ItemKind| None;
        let holding = |id: &str| {
            let mut carry = Carry::default();
            let mut s = Stack::one(id);
            if id.contains("water_skin") {
                s.liquid_l = 1.0;
            }
            carry.right = Some(s);
            carry
        };
        let food = items
            .iter()
            .find(|k| {
                hearth_craft::food::bite_of(&content, k, &Stack::one(&k.id)).is_some()
                    && k.container.is_none()
            })
            .expect("a food")
            .id
            .clone();
        assert!(matches!(
            use_of(&holding(&food), &items, &content, &none),
            InHand::Eat(_)
        ));
        let skin = items
            .iter()
            .find(|k| k.id.starts_with("hearth:water_skin/"))
            .expect("a water skin")
            .id
            .clone();
        assert!(matches!(
            use_of(&holding(&skin), &items, &content, &none),
            InHand::Drink(_)
        ));
        // Empty, it is no drink: a blow with the fist that holds it.
        let mut dry = holding(&skin);
        if let Some(s) = dry.right.as_mut() {
            s.liquid_l = 0.0;
        }
        assert_eq!(use_of(&dry, &items, &content, &none), InHand::Blow);
        let yarrow = |k: &ItemKind| {
            (k.material.as_deref() == Some("hearth:yarrow"))
                .then(|| "hearth:poultice_wound".to_owned())
        };
        let handful = items
            .iter()
            .find(|k| k.material.as_deref() == Some("hearth:yarrow"))
            .expect("yarrow")
            .id
            .clone();
        assert_eq!(
            use_of(&holding(&handful), &items, &content, &yarrow),
            InHand::Treat("hearth:poultice_wound".into())
        );
        assert_eq!(
            use_of(&holding("hearth:firebrand"), &items, &content, &none),
            InHand::HoldUp
        );
        let bow = items
            .iter()
            .find(|k| k.form.as_deref() == Some("hearth:bow"))
            .expect("a bow")
            .id
            .clone();
        assert_eq!(
            use_of(&holding(&bow), &items, &content, &none),
            InHand::Draw
        );
        assert_eq!(
            use_of(&Carry::default(), &items, &content, &none),
            InHand::Blow
        );
        // The blows things make.
        let blow = |form: &str| {
            let id = items
                .iter()
                .find(|k| k.form.as_deref() == Some(form))
                .unwrap_or_else(|| panic!("{form}"))
                .id
                .clone();
            chosen(&holding(&id), &items, false).0
        };
        assert_eq!(blow("hearth:stone_tipped_spear"), Attack::Thrust);
        assert_eq!(blow("hearth:stick"), Attack::Swing);
        assert_eq!(blow("hearth:ground_axe"), Attack::Swing);
        assert_eq!(blow("hearth:flake"), Attack::Slash);
        assert_eq!(blow("hearth:awl"), Attack::Stab);
        assert_eq!(blow("hearth:cobble"), Attack::Strike);
        assert_eq!(
            blow("hearth:basket"),
            Attack::Punch,
            "no blow of its own: the fist"
        );
        assert_eq!(chosen(&Carry::default(), &items, false).0, Attack::Punch);
        assert_eq!(chosen(&holding(&bow), &items, true).0, Attack::Kick);
        // In the left hand when the right is empty.
        let stick = items
            .iter()
            .find(|k| k.form.as_deref() == Some("hearth:stick"))
            .expect("stick")
            .id
            .clone();
        let left = Carry {
            left: Some(Stack::one(&stick)),
            ..Carry::default()
        };
        let (a, _, right) = chosen(&left, &items, false);
        assert!(a == Attack::Swing && !right);
    }

    /// A blow wounds as its kind does and shoves as its momentum does: a kick sends a hare
    /// tumbling and barely moves a hind.
    #[test]
    fn blows_wound_and_shove_by_their_kind() {
        let cat = catalog();
        let mut live = Live::new(5);
        let hare = cat.index("brown_hare").expect("hare") as u16;
        let deer = cat.index("red_deer").expect("deer") as u16;
        let h = live.place(hare, Stage::Adult, true, DVec3::new(0.0, 0.0, 0.5), 1.57);
        let d = live.place(deer, Stage::Adult, true, DVec3::new(5.0, 0.0, 0.9), 1.57);
        let kick = |live: &mut Live, at: DVec3, id: u64| {
            // Looking at the middle of its body.
            let target = live.animals.iter().find(|a| a.id == id).map_or(at, |a| {
                let rig = Rig::of(&cat.species[a.species as usize], !a.female);
                a.pos + DVec3::Y * rig.torso_y as f64
            });
            let path = Attack::Kick.path(
                &Weapon::default(),
                1.75,
                at + EYE,
                target - (at + EYE),
                true,
            );
            let hit = live.hit_along(&cat, &path, 0.5).expect("the kick lands");
            assert_eq!(hit.animal, id);
            let l = Attack::Kick.blow(&Weapon::default(), 1.0, DVec3::Z);
            let blow = Blow {
                energy_j: l.energy_j,
                piercing: l.piercing,
                cutting: l.cutting,
                push: l.push,
            };
            live.strike(&cat, &hit, &blow, "kick", at, 0.5)
                .expect("struck")
        };
        let struck = kick(&mut live, DVec3::ZERO, h);
        let knocked = |live: &Live, id: u64| {
            live.animals
                .iter()
                .find(|a| a.id == id)
                .map_or(0.0, |a| a.knock.length())
        };
        assert!(
            struck.killed || knocked(&live, h) > 3.0,
            "a hare sent tumbling: {}",
            struck.words
        );
        kick(&mut live, DVec3::new(5.0, 0.0, 0.0), d);
        assert!(knocked(&live, d) < 0.6, "a hind barely moved");
    }

    #[test]
    fn a_self_bow_sends_an_arrow_at_its_real_speed() {
        let full = bow_speed(25.0, 1.2, 0.03);
        assert!((40.0..60.0).contains(&full), "{full} m/s");
        let half = bow_speed(25.0, 0.5, 0.03);
        assert!(half < full * 0.75, "a half draw sends it slower: {half}");
        assert_eq!(bow_speed(25.0, 0.0, 0.03), 0.0);
    }
}
