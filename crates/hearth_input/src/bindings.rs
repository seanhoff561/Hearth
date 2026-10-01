//! The player's current key bindings: rebinding, conflict detection, resets, persistence, and
//! the "press a key" capture used by the Controls screen.

use std::collections::BTreeMap;

use crate::action::{ActionId, ActionRegistry};
use crate::key::{Binding, InputKey, Modifiers};

/// Current binding of every registered action.
#[derive(Debug, Clone)]
pub struct KeyBindings {
    registry: ActionRegistry,
    bindings: Vec<Option<Binding>>,
}

impl KeyBindings {
    /// Bindings initialised to every action's default.
    pub fn new(registry: ActionRegistry) -> Self {
        let bindings = registry.ids().map(|id| registry.def(id).default).collect();
        Self { registry, bindings }
    }

    /// Default layout with only built-in actions.
    pub fn builtin_defaults() -> Self {
        Self::new(ActionRegistry::with_builtins())
    }

    pub fn registry(&self) -> &ActionRegistry {
        &self.registry
    }

    /// Registers a new action (e.g. from a mod) and binds it to its default.
    pub fn register(&mut self, def: crate::action::ActionDef) -> ActionId {
        let default = def.default;
        let id = self.registry.register(def);
        if id.0 as usize == self.bindings.len() {
            self.bindings.push(default);
        }
        id
    }

    pub fn get(&self, action: ActionId) -> Option<Binding> {
        self.bindings[action.0 as usize]
    }

    /// Rebinds an action; `None` unbinds it.
    pub fn set(&mut self, action: ActionId, binding: Option<Binding>) {
        self.bindings[action.0 as usize] = binding;
    }

    pub fn default_of(&self, action: ActionId) -> Option<Binding> {
        self.registry.def(action).default
    }

    pub fn is_default(&self, action: ActionId) -> bool {
        self.get(action) == self.default_of(action)
    }

    pub fn reset(&mut self, action: ActionId) {
        self.set(action, self.default_of(action));
    }

    pub fn reset_all(&mut self) {
        for id in self.registry.ids().collect::<Vec<_>>() {
            self.reset(id);
        }
    }

    /// True if every action has its default binding.
    pub fn all_default(&self) -> bool {
        self.registry.ids().all(|id| self.is_default(id))
    }

    /// Other actions whose binding is identical to `action`'s in an overlapping context.
    pub fn conflicts_of(&self, action: ActionId) -> Vec<ActionId> {
        let Some(b) = self.get(action) else {
            return Vec::new();
        };
        let ctx = self.registry.def(action).contexts;
        self.registry
            .ids()
            .filter(|&other| {
                other != action
                    && self.get(other) == Some(b)
                    && self.registry.def(other).contexts.overlaps(ctx)
            })
            .collect()
    }

    /// True if the action's binding collides with another action's (shown in red in the UI).
    pub fn is_conflicting(&self, action: ActionId) -> bool {
        !self.conflicts_of(action).is_empty()
    }

    /// All conflicting pairs `(a, b)` with `a < b`.
    pub fn all_conflicts(&self) -> Vec<(ActionId, ActionId)> {
        let mut out = Vec::new();
        for a in self.registry.ids() {
            for b in self.conflicts_of(a) {
                if a < b {
                    out.push((a, b));
                }
            }
        }
        out
    }

    /// Actions bound to `key` (any modifiers).
    pub fn actions_for_key(&self, key: InputKey) -> impl Iterator<Item = ActionId> + '_ {
        self.registry
            .ids()
            .filter(move |&id| self.get(id).is_some_and(|b| b.key == key))
    }

    /// Serializes the non-default bindings to the `options.toml` representation. Unbound
    /// actions are written as `"none"`.
    pub fn to_map(&self) -> BTreeMap<String, String> {
        let mut map = BTreeMap::new();
        for id in self.registry.ids() {
            if self.is_default(id) {
                continue;
            }
            let value = match self.get(id) {
                Some(b) => b.to_string(),
                None => "none".to_owned(),
            };
            map.insert(self.registry.def(id).id.clone(), value);
        }
        map
    }

    /// Applies bindings from the `options.toml` representation on top of the defaults.
    /// Unknown actions are kept aside (a mod may register them later) and unparsable values
    /// fall back to the default with a warning.
    pub fn apply_map(&mut self, map: &BTreeMap<String, String>) {
        for (action_id, value) in map {
            let Some(id) = self.registry.find(action_id) else {
                log::debug!("ignoring binding for unknown action {action_id}");
                continue;
            };
            if value.eq_ignore_ascii_case("none") {
                self.set(id, None);
                continue;
            }
            match value.parse::<Binding>() {
                Ok(b) => self.set(id, Some(b)),
                Err(e) => log::warn!("{e}; keeping default for {action_id}"),
            }
        }
    }

    /// Builds bindings from defaults plus a stored map.
    pub fn from_map(registry: ActionRegistry, map: &BTreeMap<String, String>) -> Self {
        let mut b = Self::new(registry);
        b.apply_map(map);
        b
    }
}

/// Result of feeding an input event to a [`RebindCapture`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureResult {
    /// Still waiting for the final key.
    Pending,
    /// The capture finished with this binding (`None` = unbind, from Escape).
    Done(Option<Binding>),
}

/// State machine for "press a key to bind" on the Controls screen.
///
/// Modifier keys are remembered while held: pressing Ctrl then X yields `ctrl+x`; pressing and
/// releasing Ctrl alone binds the Ctrl key itself. Escape unbinds; mouse buttons bind
/// immediately (with any held modifiers).
#[derive(Debug, Clone, Default)]
pub struct RebindCapture {
    held_modifiers: Vec<InputKey>,
}

impl RebindCapture {
    pub fn new() -> Self {
        Self::default()
    }

    fn modifiers(&self) -> Modifiers {
        Modifiers(
            self.held_modifiers
                .iter()
                .fold(0, |acc, k| acc | k.modifier_bit()),
        )
    }

    pub fn press(&mut self, key: InputKey) -> CaptureResult {
        if key == InputKey::Keyboard(crate::key::Key::Escape) && self.held_modifiers.is_empty() {
            return CaptureResult::Done(None);
        }
        if key.modifier_bit() != 0 {
            if !self.held_modifiers.contains(&key) {
                self.held_modifiers.push(key);
            }
            return CaptureResult::Pending;
        }
        CaptureResult::Done(Some(Binding {
            key,
            modifiers: self.modifiers(),
        }))
    }

    pub fn release(&mut self, key: InputKey) -> CaptureResult {
        if key.modifier_bit() == 0 {
            return CaptureResult::Pending;
        }
        // Releasing a modifier with no other key pressed binds the modifier itself, with the
        // other still-held modifiers (e.g. hold Shift, tap Ctrl → shift+left_control).
        if let Some(pos) = self.held_modifiers.iter().position(|k| *k == key) {
            self.held_modifiers.remove(pos);
            let others = self.modifiers();
            return CaptureResult::Done(Some(Binding {
                key,
                modifiers: Modifiers(others.0 & !key.modifier_bit()),
            }));
        }
        CaptureResult::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{ActionDef, Category, Contexts, ToggleKind, builtin};
    use crate::key::{Key, MouseButton};

    #[test]
    fn defaults_have_no_conflicts() {
        let b = KeyBindings::builtin_defaults();
        assert!(b.all_conflicts().is_empty(), "{:?}", b.all_conflicts());
        assert!(b.all_default());
    }

    #[test]
    fn default_layout_matches_spec() {
        let b = KeyBindings::builtin_defaults();
        assert_eq!(b.get(builtin::FORWARD), Some(Binding::key(Key::W)));
        assert_eq!(b.get(builtin::SPRINT), Some(Binding::key(Key::LeftShift)));
        // v2 §10.6: crouch C, prone Z, inventory Tab, drop G, interact E, drag F, radial Q,
        // the Body panel B.
        assert_eq!(b.get(builtin::SNEAK), Some(Binding::key(Key::C)));
        assert_eq!(b.get(builtin::CRAWL), Some(Binding::key(Key::Z)));
        assert_eq!(b.get(builtin::INVENTORY), Some(Binding::key(Key::Tab)));
        assert_eq!(b.get(builtin::DROP), Some(Binding::key(Key::G)));
        assert_eq!(
            b.get(builtin::DROP_STACK),
            Some(Binding::with(Key::G, Modifiers::CTRL))
        );
        assert_eq!(b.get(builtin::INTERACT), Some(Binding::key(Key::E)));
        assert_eq!(b.get(builtin::DRAG), Some(Binding::key(Key::F)));
        assert_eq!(b.get(builtin::RADIAL), Some(Binding::key(Key::Q)));
        assert_eq!(b.get(builtin::BODY_PANEL), Some(Binding::key(Key::B)));
        assert_eq!(
            b.get(builtin::SWAP_OFFHAND),
            Some(Binding::mouse(MouseButton::Forward))
        );
        assert_eq!(
            b.get(builtin::TOGGLE_PERSPECTIVE),
            Some(Binding::mouse(MouseButton::Back))
        );
        assert_eq!(b.get(builtin::WORLD_MAP), Some(Binding::key(Key::M)));
        assert_eq!(b.get(builtin::FULLSCREEN), Some(Binding::key(Key::F11)));
        assert_eq!(b.get(builtin::HOTBAR_9), Some(Binding::key(Key::Digit9)));
    }

    #[test]
    fn rebinding_creates_and_resolves_conflicts() {
        let mut b = KeyBindings::builtin_defaults();
        b.set(builtin::JUMP, Some(Binding::key(Key::W)));
        assert!(b.is_conflicting(builtin::JUMP));
        assert!(b.is_conflicting(builtin::FORWARD));
        assert_eq!(b.conflicts_of(builtin::FORWARD), vec![builtin::JUMP]);
        assert_eq!(b.all_conflicts(), vec![(builtin::FORWARD, builtin::JUMP)]);
        b.reset(builtin::JUMP);
        assert!(!b.is_conflicting(builtin::FORWARD));
    }

    #[test]
    fn different_modifiers_do_not_conflict() {
        let b = KeyBindings::builtin_defaults();
        // X and Ctrl+X are both bound by default.
        assert!(!b.is_conflicting(builtin::DROP));
        assert!(!b.is_conflicting(builtin::DROP_STACK));
    }

    #[test]
    fn debug_chords_do_not_conflict_with_gameplay() {
        let b = KeyBindings::builtin_defaults();
        // F3+A vs A (strafe left), F3+T vs T (chat).
        assert!(!b.is_conflicting(builtin::DEBUG_RELOAD_CHUNKS));
        assert!(!b.is_conflicting(builtin::DEBUG_RELOAD_RESOURCES));
    }

    #[test]
    fn global_actions_conflict_with_gameplay() {
        let mut b = KeyBindings::builtin_defaults();
        b.set(builtin::FULLSCREEN, Some(Binding::key(Key::W)));
        assert!(b.is_conflicting(builtin::FORWARD));
    }

    #[test]
    fn unbinding_and_reset_all() {
        let mut b = KeyBindings::builtin_defaults();
        b.set(builtin::CHAT, None);
        b.set(builtin::ATTACK, Some(Binding::key(Key::F)));
        assert!(!b.all_default());
        assert!(!b.is_conflicting(builtin::CHAT));
        b.reset_all();
        assert!(b.all_default());
    }

    #[test]
    fn map_round_trip_only_stores_changes() {
        let mut b = KeyBindings::builtin_defaults();
        b.set(builtin::FORWARD, Some(Binding::key(Key::Up)));
        b.set(builtin::CHAT, None);
        b.set(
            builtin::SCREENSHOT,
            Some(Binding::with(
                Key::P,
                Modifiers::CTRL.union(Modifiers::SHIFT),
            )),
        );
        let map = b.to_map();
        assert_eq!(map.len(), 3);
        assert_eq!(map["key.forward"], "up");
        assert_eq!(map["key.chat"], "none");
        assert_eq!(map["key.screenshot"], "ctrl+shift+p");
        let back = KeyBindings::from_map(ActionRegistry::with_builtins(), &map);
        for id in b.registry().ids() {
            assert_eq!(back.get(id), b.get(id));
        }
    }

    #[test]
    fn bad_entries_fall_back_to_default() {
        let mut map = BTreeMap::new();
        map.insert("key.forward".to_owned(), "not_a_key".to_owned());
        map.insert("key.from_unloaded_mod".to_owned(), "k".to_owned());
        let b = KeyBindings::from_map(ActionRegistry::with_builtins(), &map);
        assert_eq!(b.get(builtin::FORWARD), Some(Binding::key(Key::W)));
    }

    #[test]
    fn mod_actions_can_be_registered() {
        let mut b = KeyBindings::builtin_defaults();
        let id = b.register(ActionDef {
            id: "key.mymod.zoom".into(),
            category: Category::Miscellaneous,
            contexts: Contexts::GAMEPLAY,
            default: Some(Binding::key(Key::C)),
            toggle: ToggleKind::HoldOnly,
        });
        assert_eq!(b.get(id), Some(Binding::key(Key::C)));
        let mut map = BTreeMap::new();
        map.insert("key.mymod.zoom".into(), "z".into());
        b.apply_map(&map);
        assert_eq!(b.get(id), Some(Binding::key(Key::Z)));
    }

    #[test]
    fn capture_plain_key() {
        let mut c = RebindCapture::new();
        assert_eq!(
            c.press(InputKey::Keyboard(Key::K)),
            CaptureResult::Done(Some(Binding::key(Key::K)))
        );
    }

    #[test]
    fn capture_modifier_combo() {
        let mut c = RebindCapture::new();
        assert_eq!(
            c.press(InputKey::Keyboard(Key::LeftControl)),
            CaptureResult::Pending
        );
        assert_eq!(
            c.press(InputKey::Keyboard(Key::X)),
            CaptureResult::Done(Some(Binding::with(Key::X, Modifiers::CTRL)))
        );
    }

    #[test]
    fn capture_modifier_alone() {
        let mut c = RebindCapture::new();
        assert_eq!(
            c.press(InputKey::Keyboard(Key::LeftShift)),
            CaptureResult::Pending
        );
        assert_eq!(
            c.release(InputKey::Keyboard(Key::LeftShift)),
            CaptureResult::Done(Some(Binding::key(Key::LeftShift)))
        );
    }

    #[test]
    fn capture_modifier_with_other_modifier_held() {
        let mut c = RebindCapture::new();
        c.press(InputKey::Keyboard(Key::LeftShift));
        c.press(InputKey::Keyboard(Key::LeftControl));
        assert_eq!(
            c.release(InputKey::Keyboard(Key::LeftControl)),
            CaptureResult::Done(Some(Binding::with(Key::LeftControl, Modifiers::SHIFT)))
        );
    }

    #[test]
    fn capture_escape_unbinds_and_mouse_binds() {
        let mut c = RebindCapture::new();
        assert_eq!(
            c.press(InputKey::Keyboard(Key::Escape)),
            CaptureResult::Done(None)
        );
        let mut c = RebindCapture::new();
        c.press(InputKey::Keyboard(Key::LeftAlt));
        assert_eq!(
            c.press(InputKey::Mouse(MouseButton::Back)),
            CaptureResult::Done(Some(Binding {
                key: InputKey::Mouse(MouseButton::Back),
                modifiers: Modifiers::ALT
            }))
        );
    }
}
