//! Logical input actions (move forward, attack, open inventory, …), their categories, the input
//! contexts they are active in, and their default bindings.

use crate::key::{Binding, Key, Modifiers, MouseButton, PadButton};

/// Dense index of a registered action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ActionId(pub u16);

/// Grouping used by the Controls screen, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    Movement,
    Gameplay,
    Inventory,
    Creative,
    Miscellaneous,
    Debug,
}

impl Category {
    pub const ALL: [Category; 6] = [
        Category::Movement,
        Category::Gameplay,
        Category::Inventory,
        Category::Creative,
        Category::Miscellaneous,
        Category::Debug,
    ];

    /// Translation key of the category heading.
    pub fn translation_key(self) -> &'static str {
        match self {
            Category::Movement => "key.categories.movement",
            Category::Gameplay => "key.categories.gameplay",
            Category::Inventory => "key.categories.inventory",
            Category::Creative => "key.categories.creative",
            Category::Miscellaneous => "key.categories.misc",
            Category::Debug => "key.categories.debug",
        }
    }
}

/// Bit set of input contexts. Two bindings only conflict if their contexts overlap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Contexts(pub u8);

impl Contexts {
    /// Playing in the world with no screen open.
    pub const GAMEPLAY: Contexts = Contexts(1);
    /// A container / inventory screen is open.
    pub const CONTAINER: Contexts = Contexts(2);
    /// The debug key (F3) is held: debug chords.
    pub const DEBUG_CHORD: Contexts = Contexts(4);
    /// Always active, even in menus.
    pub const GLOBAL: Contexts = Contexts(8);

    pub const fn union(self, o: Contexts) -> Contexts {
        Contexts(self.0 | o.0)
    }
    pub fn overlaps(self, o: Contexts) -> bool {
        // Global actions are active everywhere except inside debug chords, which only fire
        // while the debug key is held and suppress their plain meaning.
        let expand = |c: Contexts| {
            if c.0 & Self::GLOBAL.0 != 0 {
                Contexts(c.0 | Self::GAMEPLAY.0 | Self::CONTAINER.0)
            } else {
                c
            }
        };
        expand(self).0 & expand(o).0 != 0
    }
    pub fn contains(self, o: Contexts) -> bool {
        self.0 & o.0 == o.0
    }
}

/// How an action is worked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Active while its key is held (walking, the hands' work, dragging). On a modifier key it
    /// starts on the press and stays held through combinations made with it.
    Hold,
    /// Held, or latched by a press when its toggle option is on (crouching, running).
    Toggleable,
    /// Does one thing per press (the inventory, a quick slot, the journal). On a modifier key
    /// (or the debug key) alone it waits for the key to be let go, and comes only if no other
    /// key was pressed meanwhile, so the combinations made with that key still work.
    Tap,
}

/// Static description of an action.
#[derive(Debug, Clone)]
pub struct ActionDef {
    /// Stable id used in `options.toml` and as translation key, e.g. `key.forward`.
    pub id: String,
    pub category: Category,
    pub contexts: Contexts,
    pub default: Option<Binding>,
    /// The controller's button by default (a second binding, beside the key's).
    pub pad: Option<PadButton>,
    pub kind: Kind,
}

/// Built-in actions. Their ids are the indices they are registered at by
/// [`ActionRegistry::with_builtins`].
pub mod builtin {
    use super::ActionId;
    macro_rules! ids {
        ($($name:ident = $n:expr;)*) => { $(pub const $name: ActionId = ActionId($n);)* };
    }
    ids! {
        FORWARD = 0; LEFT = 1; BACK = 2; RIGHT = 3; JUMP = 4; SNEAK = 5; SPRINT = 6;
        ATTACK = 7; PICK_BLOCK = 8;
        INVENTORY = 9; DROP = 10; DROP_STACK = 11;
        HOTBAR_1 = 12; HOTBAR_2 = 13; HOTBAR_3 = 14; HOTBAR_4 = 15; HOTBAR_5 = 16; HOTBAR_6 = 17;
        DEBUG = 18; TOGGLE_PERSPECTIVE = 19; WORLD_MAP = 20; FULLSCREEN = 21; PAUSE = 22;
        DEBUG_RELOAD_RESOURCES = 23;
        DEBUG_TIME_FORWARD = 24; DEBUG_TIME_BACK = 25; DEBUG_SEASON_FORWARD = 26;
        DEBUG_TIME_WARP = 27; CRAWL = 28; DEBUG_FREE_CAMERA = 29;
        SLEEP = 30; BODY_PANEL = 31; INTERACT = 32; DRAG = 33; RADIAL = 34;
        THROW = 35; JOURNAL = 36; SHOUT = 37; BUILDER_VIEW = 38;
        WATCH_FASTER = 39; WATCH_SLOWER = 40;
        CLEAR_VIEW = 41; SPECTATE = 42; CREATIVE_REMOVE = 43; NO_CLIP = 44;
        KICK = 45;
    }
    /// Number of built-in actions.
    pub const COUNT: usize = 46;

    /// The quick slots' actions in slot order.
    pub const HOTBAR: [ActionId; 6] = [HOTBAR_1, HOTBAR_2, HOTBAR_3, HOTBAR_4, HOTBAR_5, HOTBAR_6];
}

/// Registry of all actions (built-in first, then any added by mods).
#[derive(Debug, Clone)]
pub struct ActionRegistry {
    defs: Vec<ActionDef>,
}

impl ActionRegistry {
    /// A registry containing exactly the built-in actions with the default layout.
    pub fn with_builtins() -> Self {
        use Category as C;
        let g = Contexts::GAMEPLAY;
        let gc = Contexts::GAMEPLAY.union(Contexts::CONTAINER);
        let glob = Contexts::GLOBAL;
        let dbg = Contexts::DEBUG_CHORD;
        let k = |key| Some(Binding::key(key));
        let m = |b| Some(Binding::mouse(b));
        let mut r = Self { defs: Vec::new() };
        let mut add = |id: &str, category, contexts, default, kind| {
            r.defs.push(ActionDef {
                id: id.to_owned(),
                category,
                contexts,
                default,
                pad: None,
                kind,
            });
        };
        use Kind::{Hold as H, Tap, Toggleable as T};
        add("key.forward", C::Movement, g, k(Key::W), H);
        add("key.left", C::Movement, g, k(Key::A), H);
        add("key.back", C::Movement, g, k(Key::S), H);
        add("key.right", C::Movement, g, k(Key::D), H);
        add("key.jump", C::Movement, g, k(Key::Space), H);
        add("key.sneak", C::Movement, g, k(Key::C), T);
        add("key.sprint", C::Movement, g, k(Key::LeftShift), T);
        add("key.attack", C::Gameplay, g, m(MouseButton::Left), H);
        add(
            "key.pick_block",
            C::Gameplay,
            g,
            m(MouseButton::Middle),
            Tap,
        );
        add("key.inventory", C::Inventory, gc, k(Key::Tab), Tap);
        add("key.drop", C::Inventory, gc, k(Key::G), Tap);
        add(
            "key.drop_stack",
            C::Inventory,
            gc,
            Some(Binding::with(Key::G, Modifiers::CTRL)),
            Tap,
        );
        let digits = [
            Key::Digit1,
            Key::Digit2,
            Key::Digit3,
            Key::Digit4,
            Key::Digit5,
            Key::Digit6,
        ];
        for (i, d) in digits.into_iter().enumerate() {
            add(
                &format!("key.hotbar.{}", i + 1),
                C::Inventory,
                gc,
                k(d),
                Tap,
            );
        }
        add("key.debug", C::Miscellaneous, g, k(Key::F3), Tap);
        add(
            "key.toggle_perspective",
            C::Miscellaneous,
            g,
            m(MouseButton::Back),
            Tap,
        );
        add("key.world_map", C::Miscellaneous, g, k(Key::M), Tap);
        add("key.fullscreen", C::Miscellaneous, glob, k(Key::F11), Tap);
        add("key.pause", C::Miscellaneous, g, k(Key::Escape), Tap);
        add("key.debug.reload_resources", C::Debug, dbg, k(Key::T), Tap);
        add("key.debug.time_forward", C::Debug, dbg, k(Key::Right), Tap);
        add("key.debug.time_back", C::Debug, dbg, k(Key::Left), Tap);
        add("key.debug.season_forward", C::Debug, dbg, k(Key::Up), Tap);
        add("key.debug.time_warp", C::Debug, dbg, k(Key::W), Tap);
        add("key.crawl", C::Movement, g, k(Key::Z), T);
        add("key.debug.free_camera", C::Debug, dbg, k(Key::N), Tap);
        add("key.sleep", C::Gameplay, g, k(Key::X), Tap);
        add("key.body_panel", C::Gameplay, g, k(Key::B), Tap);
        add("key.interact", C::Gameplay, g, k(Key::E), Tap);
        add("key.drag", C::Gameplay, g, k(Key::F), H);
        add("key.radial", C::Inventory, g, k(Key::Q), H);
        add("key.throw", C::Gameplay, g, k(Key::R), H);
        add("key.journal", C::Gameplay, g, k(Key::J), Tap);
        add("key.shout", C::Gameplay, g, k(Key::H), Tap);
        add("key.builder_view", C::Gameplay, g, k(Key::V), Tap);
        add(
            "key.watch.faster",
            C::Gameplay,
            g,
            k(Key::RightBracket),
            Tap,
        );
        add("key.watch.slower", C::Gameplay, g, k(Key::LeftBracket), Tap);
        // Creative's (Amendment P §3, §13): keys no earlier action holds.
        add("key.creative.clear_view", C::Creative, g, k(Key::F4), Tap);
        add("key.creative.spectate", C::Creative, g, k(Key::F6), Tap);
        add("key.creative.remove", C::Creative, g, k(Key::Delete), Tap);
        add("key.creative.no_clip", C::Creative, g, k(Key::F7), Tap);
        // A kick (E §3.2): T lies by the movement keys and holds nothing else in play (F3 + T
        // is a debug chord, apart).
        add("key.kick", C::Gameplay, g, k(Key::T), H);
        // The controller (v1 M11, V2-3): the sticks walk and look; every button can be rebound.
        use PadButton as P;
        for (id, b) in [
            (builtin::JUMP, P::South),
            (builtin::SNEAK, P::East),
            (builtin::CRAWL, P::West),
            (builtin::INVENTORY, P::North),
            (builtin::SPRINT, P::LeftStick),
            (builtin::TOGGLE_PERSPECTIVE, P::DPadRight),
            (builtin::ATTACK, P::RightTrigger),
            (builtin::INTERACT, P::LeftTrigger),
            (builtin::THROW, P::RightBumper),
            (builtin::RADIAL, P::LeftBumper),
            (builtin::PAUSE, P::Start),
            (builtin::WORLD_MAP, P::Select),
            (builtin::JOURNAL, P::DPadUp),
            (builtin::DROP, P::DPadDown),
            (builtin::SLEEP, P::DPadLeft),
            (builtin::KICK, P::RightStick),
        ] {
            r.defs[id.0 as usize].pad = Some(b);
        }
        debug_assert_eq!(r.defs.len(), builtin::COUNT);
        r
    }

    /// Registers an additional action (used by mods). Returns its id.
    pub fn register(&mut self, def: ActionDef) -> ActionId {
        if let Some(existing) = self.find(&def.id) {
            log::warn!("action {} registered twice; keeping the first", def.id);
            return existing;
        }
        let id = ActionId(self.defs.len() as u16);
        self.defs.push(def);
        id
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    pub fn def(&self, id: ActionId) -> &ActionDef {
        &self.defs[id.0 as usize]
    }

    pub fn find(&self, id: &str) -> Option<ActionId> {
        self.defs
            .iter()
            .position(|d| d.id == id)
            .map(|i| ActionId(i as u16))
    }

    pub fn ids(&self) -> impl Iterator<Item = ActionId> + '_ {
        (0..self.defs.len()).map(|i| ActionId(i as u16))
    }
}
