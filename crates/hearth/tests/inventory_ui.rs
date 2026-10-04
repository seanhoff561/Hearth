//! The inventory screen round trip: drawn offscreen (`bench-out/inventory.png`), a click lifts
//! the hand axe from the right hand and a click on the basket's first cell asks for it to move
//! there; the move, made, puts it in the basket. E on food asks to eat it where it is.

use hearth::interface::Interface;
use hearth::inventory_ui::InventoryView;
use hearth::menus::{MenuAction, MenuContext, Menus, Screen};
use hearth_content::Content;
use hearth_core::options::Options;
use hearth_input::{ActionRegistry, KeyBindings};
use hearth_items::{Carry, Hand, Items, Path, Root, Stack, Target};
use hearth_render::GpuContext;
use hearth_render::offscreen::{OFFSCREEN_FORMAT, OffscreenTarget, write_png};

fn id(items: &Items, suffix: &str) -> String {
    items
        .iter()
        .find(|k| k.id.ends_with(suffix))
        .unwrap_or_else(|| panic!("no {suffix}"))
        .id
        .clone()
}

#[test]
fn the_inventory_moves_things_where_they_are_put() {
    let Ok(ctx) = GpuContext::headless(false) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let content = Content::load_base();
    let items = Items::from_content(&content);
    let body_kg = 70.0;
    // A loincloth and a belt with a pouch of flakes, a basket of cobbles in the left hand, a
    // hand axe in the right.
    let loincloth = items
        .iter()
        .find(|k| {
            k.wear
                .as_ref()
                .is_some_and(|w| w.garment.ends_with("loincloth"))
        })
        .expect("loincloth")
        .id
        .clone();
    let belt = items
        .iter()
        .find(|k| k.wear.as_ref().is_some_and(|w| w.garment.ends_with("belt")))
        .expect("belt")
        .id
        .clone();
    let mut carry = Carry::dressed(&items, [Stack::one(&loincloth), Stack::one(&belt)]);
    let mut pouch = Stack::one(&id(&items, "pouch/rawhide"));
    let pouch_spec = pouch.kind(&items).and_then(|k| k.container).expect("pouch");
    pouch
        .contents_mut(&items)
        .expect("opens")
        .put_anywhere(
            &items,
            &pouch_spec,
            Stack::of(&id(&items, "flake/flint"), 11),
        )
        .expect("flakes");
    carry.hang(&items, pouch, 1, 0).expect("on the belt");
    let mut basket = Stack::one(&id(&items, "basket/reed"));
    let basket_spec = basket
        .kind(&items)
        .and_then(|k| k.container)
        .expect("basket");
    basket
        .contents_mut(&items)
        .expect("opens")
        .put(
            &items,
            &basket_spec,
            Stack::one(&id(&items, "cobble/granite")),
            2,
            2,
            false,
        )
        .expect("a cobble");
    carry
        .hold(&items, basket, Hand::Left, body_kg)
        .expect("basket");
    carry
        .hold(
            &items,
            Stack::one(&id(&items, "hand_axe/flint")),
            Hand::Right,
            body_kg,
        )
        .expect("axe");

    let (w, h) = (1280u32, 720u32);
    let target = OffscreenTarget::new(&ctx, w, h);
    let mut iface = Interface::new("en_us");
    let mut options = Options::default();
    let mut bindings = KeyBindings::from_map(
        ActionRegistry::with_builtins(),
        &options.controls.key_bindings,
    );
    let mut profiles = hearth::profiles::Profiles::default();
    let languages = vec!["en_us".to_owned()];
    let mut menus = Menus::none();
    menus.open(Screen::Inventory {
        lifted: None,
        turned: false,
    });
    let mut frame = |iface: &mut Interface, menus: &mut Menus, carry: &Carry| {
        let mut actions = Vec::new();
        let mut enc = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        iface.frame(
            &ctx,
            &mut enc,
            &target.color_view,
            OFFSCREEN_FORMAT,
            (w, h),
            0,
            |ui| {
                let mut cx = MenuContext {
                    options: &mut options,
                    bindings: &mut bindings,
                    saves: std::env::temp_dir(),
                    in_game: true,
                    languages: &languages,
                    audio_devices: &[],
                    profiles: &mut profiles,
                    death: None,
                    inventory: Some(InventoryView {
                        carry,
                        items: &items,
                        body_kg,
                        content: Some(&content),
                    }),
                    journal: None,
                    eras: Vec::new(),
                };
                actions = menus.ui(ui, &mut cx);
            },
        );
        ctx.queue.submit(Some(enc.finish()));
        actions
    };
    // The interface is twice the size at 720 lines: the right hand's slot at (88..192, 41),
    // the basket (the left hand's, the first grid) from (208, 37), cells 13 across.
    let s = 2.0;
    frame(&mut iface, &mut menus, &carry);
    let px = target.read_rgba(&ctx);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench-out");
    std::fs::create_dir_all(&out).ok();
    write_png(&out.join("inventory.png"), w, h, &px).expect("png");

    iface.pointer_moved(140.0 * s, 47.0 * s);
    iface.button(true);
    let none = frame(&mut iface, &mut menus, &carry);
    assert!(none.is_empty(), "lifting asks nothing yet: {none:?}");
    iface.button(false);
    frame(&mut iface, &mut menus, &carry);
    iface.pointer_moved((208.0 + 6.0) * s, (37.0 + 6.0) * s);
    iface.button(true);
    let actions = frame(&mut iface, &mut menus, &carry);
    iface.button(false);
    let shift = actions
        .iter()
        .find_map(|a| match a {
            MenuAction::Shift { from, count, to } => Some((from.clone(), *count, to.clone())),
            _ => None,
        })
        .expect("a move");
    assert_eq!(shift.0, Path::at(Root::Hand(Hand::Right)));
    assert_eq!(
        shift.2,
        Target::Cell {
            container: Path::at(Root::Hand(Hand::Left)),
            x: 0,
            y: 0,
            turned: false
        }
    );
    // Made, the move puts the axe in the basket and empties the right hand.
    assert!(carry.shift(&items, &shift.0, shift.1, &shift.2, body_kg));
    assert!(carry.right.is_none());
    let in_basket = carry
        .get(&Path::at(Root::Hand(Hand::Left)))
        .and_then(|b| b.contents())
        .map_or(0, |b| b.items.len());
    assert_eq!(in_basket, 2, "the cobble and the axe");

    // Meat in the right hand: E with the pointer on it asks to eat it; on the axe, nothing.
    carry
        .hold(
            &items,
            Stack::one(&id(&items, "cooked_meat")),
            Hand::Right,
            body_kg,
        )
        .expect("meat");
    iface.pointer_moved(140.0 * s, 47.0 * s);
    iface.typed("e");
    let actions = frame(&mut iface, &mut menus, &carry);
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, MenuAction::Eat(p) if *p == Path::at(Root::Hand(Hand::Right)))),
        "{actions:?}"
    );
    iface.pointer_moved((208.0 + 6.0) * s, (37.0 + 6.0) * s);
    iface.typed("e");
    let actions = frame(&mut iface, &mut menus, &carry);
    assert!(
        !actions.iter().any(|a| matches!(a, MenuAction::Eat(_))),
        "{actions:?}"
    );
}
