//! Logical input actions (move forward, attack, open inventory, …), their categories, the input
//! contexts they are active in, and their default bindings.

use crate::key::{Binding, Key, Modifiers, MouseButton};

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
    Multiplayer,
    Miscellaneous,
    Debug,
}

impl Category {
    pub const ALL: [Category; 7] = [
        Category::Movement,
        Category::Gameplay,
        Category::Inventory,
        Category::Creative,
        Category::Multiplayer,
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
            Category::Multiplayer => "key.categories.multiplayer",
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

/// How a held action behaves when its "toggle" option is enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleKind {
    /// Always hold-to-activate.
    HoldOnly,
    /// Can be switched to toggle mode (sneak and sprint).
    Toggleable,
}

/// Static description of an action.
#[derive(Debug, Clone)]
pub struct ActionDef {
    /// Stable id used in `options.toml` and as translation key, e.g. `key.forward`.
    pub id: String,
    pub category: Category,
    pub contexts: Contexts,
    pub default: Option<Binding>,
    pub toggle: ToggleKind,
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
        ATTACK = 7; USE = 8; PICK_BLOCK = 9;
        INVENTORY = 10; DROP = 11; DROP_STACK = 12; SWAP_OFFHAND = 13;
        HOTBAR_1 = 14; HOTBAR_2 = 15; HOTBAR_3 = 16; HOTBAR_4 = 17; HOTBAR_5 = 18;
        HOTBAR_6 = 19; HOTBAR_7 = 20; HOTBAR_8 = 21; HOTBAR_9 = 22;
        CHAT = 23; COMMAND = 24; PLAYER_LIST = 25;
        HIDE_HUD = 26; SCREENSHOT = 27; DEBUG = 28; TOGGLE_PERSPECTIVE = 29;
        WORLD_MAP = 30; FULLSCREEN = 31; SMOOTH_CAMERA = 32; PAUSE = 33;
        DEBUG_RELOAD_CHUNKS = 34; DEBUG_HITBOXES = 35; DEBUG_CHUNK_BORDERS = 36;
        DEBUG_RELOAD_RESOURCES = 37; DEBUG_ADVANCED_TOOLTIPS = 38; DEBUG_PAUSE_NO_MENU = 39;
        DEBUG_FRAME_GRAPH = 40; DEBUG_PROFILER = 41;
        DEBUG_TIME_FORWARD = 42; DEBUG_TIME_BACK = 43; DEBUG_SEASON_FORWARD = 44;
        DEBUG_TIME_WARP = 45;
    }
    /// Number of built-in actions.
    pub const COUNT: usize = 46;

    /// Hotbar actions in slot order.
    pub const HOTBAR: [ActionId; 9] = [
        HOTBAR_1, HOTBAR_2, HOTBAR_3, HOTBAR_4, HOTBAR_5, HOTBAR_6, HOTBAR_7, HOTBAR_8, HOTBAR_9,
    ];
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
        let mut add = |id: &str, category, contexts, default, toggle| {
            r.defs.push(ActionDef {
                id: id.to_owned(),
                category,
                contexts,
                default,
                toggle,
            });
        };
        use ToggleKind::{HoldOnly as H, Toggleable as T};
        add("key.forward", C::Movement, g, k(Key::W), H);
        add("key.left", C::Movement, g, k(Key::A), H);
        add("key.back", C::Movement, g, k(Key::S), H);
        add("key.right", C::Movement, g, k(Key::D), H);
        add("key.jump", C::Movement, g, k(Key::Space), H);
        add("key.sneak", C::Movement, g, k(Key::LeftControl), T);
        add("key.sprint", C::Movement, g, k(Key::LeftShift), T);
        add("key.attack", C::Gameplay, g, m(MouseButton::Left), H);
        add("key.use", C::Gameplay, g, m(MouseButton::Right), H);
        add("key.pick_block", C::Gameplay, g, m(MouseButton::Middle), H);
        add("key.inventory", C::Inventory, gc, k(Key::LeftAlt), H);
        add("key.drop", C::Inventory, gc, k(Key::X), H);
        add(
            "key.drop_stack",
            C::Inventory,
            gc,
            Some(Binding::with(Key::X, Modifiers::CTRL)),
            H,
        );
        add(
            "key.swap_offhand",
            C::Inventory,
            gc,
            m(MouseButton::Forward),
            H,
        );
        let digits = [
            Key::Digit1,
            Key::Digit2,
            Key::Digit3,
            Key::Digit4,
            Key::Digit5,
            Key::Digit6,
            Key::Digit7,
            Key::Digit8,
            Key::Digit9,
        ];
        for (i, d) in digits.into_iter().enumerate() {
            add(&format!("key.hotbar.{}", i + 1), C::Inventory, gc, k(d), H);
        }
        add("key.chat", C::Multiplayer, g, k(Key::T), H);
        add("key.command", C::Multiplayer, g, k(Key::Slash), H);
        add("key.player_list", C::Multiplayer, g, k(Key::Tab), H);
        add("key.hide_hud", C::Miscellaneous, g, k(Key::F1), H);
        add("key.screenshot", C::Miscellaneous, glob, k(Key::F2), H);
        add("key.debug", C::Miscellaneous, g, k(Key::F3), H);
        add(
            "key.toggle_perspective",
            C::Miscellaneous,
            g,
            m(MouseButton::Back),
            H,
        );
        add("key.world_map", C::Miscellaneous, g, k(Key::M), H);
        add("key.fullscreen", C::Miscellaneous, glob, k(Key::F11), H);
        add("key.smooth_camera", C::Miscellaneous, g, None, H);
        add("key.pause", C::Miscellaneous, g, k(Key::Escape), H);
        add("key.debug.reload_chunks", C::Debug, dbg, k(Key::A), H);
        add("key.debug.hitboxes", C::Debug, dbg, k(Key::B), H);
        add("key.debug.chunk_borders", C::Debug, dbg, k(Key::G), H);
        add("key.debug.reload_resources", C::Debug, dbg, k(Key::T), H);
        add("key.debug.advanced_tooltips", C::Debug, dbg, k(Key::H), H);
        add("key.debug.pause_no_menu", C::Debug, dbg, k(Key::Escape), H);
        add("key.debug.frame_graph", C::Debug, dbg, k(Key::Digit2), H);
        add("key.debug.profiler", C::Debug, dbg, k(Key::Digit1), H);
        add("key.debug.time_forward", C::Debug, dbg, k(Key::Right), H);
        add("key.debug.time_back", C::Debug, dbg, k(Key::Left), H);
        add("key.debug.season_forward", C::Debug, dbg, k(Key::Up), H);
        add("key.debug.time_warp", C::Debug, dbg, k(Key::W), H);
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

    /// Actions in a category, in registration order.
    pub fn in_category(&self, cat: Category) -> impl Iterator<Item = ActionId> + '_ {
        self.ids().filter(move |id| self.def(*id).category == cat)
    }
}
