//! Per-frame input state: which actions are held, which were pressed or released since the
//! last frame, toggle latches for sneak/sprint, and accumulated mouse motion and scrolling.

use crate::action::{ActionId, Contexts, ToggleKind};
use crate::bindings::KeyBindings;
use crate::key::{InputKey, Modifiers};

/// Options affecting how actions are interpreted.
#[derive(Debug, Clone, Copy, Default)]
pub struct InputOptions {
    pub toggle_sneak: bool,
    pub toggle_sprint: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct ActionSlot {
    /// Number of held keys currently driving this action.
    holders: u8,
    /// Presses since the last frame (for repeated actions such as hotbar scrolling).
    presses: u16,
    released: bool,
    /// Latched state for toggle-mode actions.
    latched: bool,
}

/// Tracks raw input and derives action states.
#[derive(Debug, Clone)]
pub struct InputState {
    held: Vec<InputKey>,
    /// Actions activated by each currently held key, parallel to `held`.
    held_actions: Vec<Vec<ActionId>>,
    slots: Vec<ActionSlot>,
    context: Contexts,
    debug_key_held: bool,
    /// A debug chord fired while the debug key was held (suppresses the overlay toggle).
    debug_chord_used: bool,
    mouse_delta: (f64, f64),
    scroll_delta: f64,
    options: InputOptions,
}

impl InputState {
    pub fn new(action_count: usize) -> Self {
        Self {
            held: Vec::with_capacity(16),
            held_actions: Vec::with_capacity(16),
            slots: vec![ActionSlot::default(); action_count],
            context: Contexts::GAMEPLAY,
            debug_key_held: false,
            debug_chord_used: false,
            mouse_delta: (0.0, 0.0),
            scroll_delta: 0.0,
            options: InputOptions::default(),
        }
    }

    pub fn set_options(&mut self, options: InputOptions) {
        self.options = options;
    }

    /// Makes sure the slot table covers all registered actions (after a mod registers more).
    pub fn sync_action_count(&mut self, count: usize) {
        if self.slots.len() < count {
            self.slots.resize(count, ActionSlot::default());
        }
    }

    /// Current context (gameplay, container screen, …).
    pub fn context(&self) -> Contexts {
        self.context
    }

    /// Switches input context. Actions not valid in the new context are released so that
    /// e.g. opening the inventory while walking stops the player.
    pub fn set_context(&mut self, context: Contexts, bindings: &KeyBindings) {
        if context == self.context {
            return;
        }
        self.context = context;
        for i in 0..self.held_actions.len() {
            let actions = std::mem::take(&mut self.held_actions[i]);
            let mut keep = Vec::with_capacity(actions.len());
            for a in actions {
                if self.action_active_in(a, bindings) {
                    keep.push(a);
                } else {
                    self.release_slot(a);
                }
            }
            self.held_actions[i] = keep;
        }
    }

    fn action_active_in(&self, action: ActionId, bindings: &KeyBindings) -> bool {
        let ctx = bindings.registry().def(action).contexts;
        if ctx.contains(Contexts::DEBUG_CHORD) {
            return self.debug_key_held && self.context.contains(Contexts::GAMEPLAY);
        }
        ctx.overlaps(self.context)
    }

    fn release_slot(&mut self, action: ActionId) {
        let slot = &mut self.slots[action.0 as usize];
        slot.holders = slot.holders.saturating_sub(1);
        if slot.holders == 0 {
            slot.released = true;
        }
    }

    /// Modifier bits currently held.
    pub fn modifiers(&self) -> Modifiers {
        Modifiers(self.held.iter().fold(0, |acc, k| acc | k.modifier_bit()))
    }

    pub fn is_key_held(&self, key: InputKey) -> bool {
        self.held.contains(&key)
    }

    /// Feeds a key or mouse button press. Returns the actions it activated.
    pub fn press(&mut self, key: InputKey, bindings: &KeyBindings) -> &[ActionId] {
        if self.held.contains(&key) {
            // OS key repeat: not a new press.
            return &[];
        }
        let debug_binding = bindings.get(crate::action::builtin::DEBUG);
        let modifiers = self.modifiers();
        // Candidates: bound to this key, modifiers satisfied, active in the current context.
        let mut best = 0u32;
        let mut chosen: Vec<ActionId> = Vec::new();
        for id in bindings.actions_for_key(key) {
            let Some(b) = bindings.get(id) else { continue };
            if !modifiers.contains(b.modifiers) || !self.action_active_in(id, bindings) {
                continue;
            }
            let specificity = b.modifiers.count()
                + if bindings.registry().def(id).contexts == Contexts::DEBUG_CHORD {
                    // Debug chords win over the key's normal meaning while F3 is held.
                    8
                } else {
                    0
                };
            if specificity > best {
                best = specificity;
                chosen.clear();
            }
            if specificity == best {
                chosen.push(id);
            }
        }
        for &id in &chosen {
            let def = bindings.registry().def(id);
            if def.contexts == Contexts::DEBUG_CHORD {
                self.debug_chord_used = true;
            }
            let toggleable = def.toggle == ToggleKind::Toggleable && self.toggle_enabled(id);
            let slot = &mut self.slots[id.0 as usize];
            slot.holders = slot.holders.saturating_add(1);
            slot.presses = slot.presses.saturating_add(1);
            if toggleable {
                slot.latched = !slot.latched;
            }
        }
        if debug_binding.is_some_and(|b| b.key == key) {
            self.debug_key_held = true;
            self.debug_chord_used = false;
        }
        self.held.push(key);
        self.held_actions.push(chosen);
        self.held_actions.last().map(Vec::as_slice).unwrap_or(&[])
    }

    fn toggle_enabled(&self, id: ActionId) -> bool {
        use crate::action::builtin::{SNEAK, SPRINT};
        (id == SNEAK && self.options.toggle_sneak) || (id == SPRINT && self.options.toggle_sprint)
    }

    /// Feeds a key or mouse button release.
    pub fn release(&mut self, key: InputKey, bindings: &KeyBindings) {
        let Some(pos) = self.held.iter().position(|k| *k == key) else {
            return;
        };
        self.held.swap_remove(pos);
        let actions = self.held_actions.swap_remove(pos);
        for a in actions {
            self.release_slot(a);
        }
        if bindings
            .get(crate::action::builtin::DEBUG)
            .is_some_and(|b| b.key == key)
        {
            self.debug_key_held = false;
        }
    }

    /// Releases everything (window focus lost).
    pub fn release_all(&mut self) {
        for actions in self.held_actions.drain(..) {
            for a in actions {
                let slot = &mut self.slots[a.0 as usize];
                slot.holders = slot.holders.saturating_sub(1);
                if slot.holders == 0 {
                    slot.released = true;
                }
            }
        }
        self.held.clear();
        self.debug_key_held = false;
    }

    /// Clears toggle latches (e.g. on death or when entering a vehicle).
    pub fn clear_toggles(&mut self) {
        for s in &mut self.slots {
            s.latched = false;
        }
    }

    pub fn add_mouse_motion(&mut self, dx: f64, dy: f64) {
        self.mouse_delta.0 += dx;
        self.mouse_delta.1 += dy;
    }

    pub fn add_scroll(&mut self, lines: f64) {
        self.scroll_delta += lines;
    }

    /// Mouse motion accumulated since the last frame.
    pub fn mouse_delta(&self) -> (f64, f64) {
        self.mouse_delta
    }

    /// Scroll accumulated since the last frame, in lines (positive = away from the user).
    pub fn scroll_delta(&self) -> f64 {
        self.scroll_delta
    }

    /// Takes the whole-step part of the accumulated scroll, keeping the remainder so that
    /// smooth trackpads still produce discrete hotbar steps.
    pub fn take_scroll_steps(&mut self, sensitivity: f64, discrete: bool) -> i32 {
        if discrete {
            // Note: f64::signum(0.0) is 1.0, so zero needs its own case.
            let steps = if self.scroll_delta > 0.0 {
                1
            } else if self.scroll_delta < 0.0 {
                -1
            } else {
                0
            };
            self.scroll_delta = 0.0;
            return steps;
        }
        let total = self.scroll_delta * sensitivity;
        let steps = total.trunc();
        self.scroll_delta = (total - steps) / sensitivity.max(1e-6);
        steps as i32
    }

    /// True while the action's key is held (ignores toggle mode).
    pub fn is_down(&self, action: ActionId) -> bool {
        self.slots[action.0 as usize].holders > 0
    }

    /// For toggle-capable actions: the latched state in toggle mode, otherwise held state.
    pub fn is_active(&self, action: ActionId) -> bool {
        if self.toggle_enabled(action) {
            self.slots[action.0 as usize].latched
        } else {
            self.is_down(action)
        }
    }

    /// True if the action was pressed at least once since the last frame.
    pub fn was_pressed(&self, action: ActionId) -> bool {
        self.slots[action.0 as usize].presses > 0
    }

    /// Number of presses since the last frame.
    pub fn press_count(&self, action: ActionId) -> u16 {
        self.slots[action.0 as usize].presses
    }

    /// True if the action was released since the last frame.
    pub fn was_released(&self, action: ActionId) -> bool {
        self.slots[action.0 as usize].released
    }

    pub fn debug_key_held(&self) -> bool {
        self.debug_key_held
    }

    /// True if the debug key was released since last frame without any chord being used, i.e.
    /// the debug overlay should toggle.
    pub fn debug_overlay_toggled(&self) -> bool {
        self.was_released(crate::action::builtin::DEBUG) && !self.debug_chord_used
    }

    /// Clears per-frame edges and deltas. Call once per frame after the game consumed input.
    pub fn end_frame(&mut self) {
        for s in &mut self.slots {
            s.presses = 0;
            s.released = false;
        }
        self.mouse_delta = (0.0, 0.0);
        if !self.debug_key_held {
            self.debug_chord_used = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::builtin::*;
    use crate::key::{Binding, Key, MouseButton};

    fn kb(k: Key) -> InputKey {
        InputKey::Keyboard(k)
    }

    fn setup() -> (KeyBindings, InputState) {
        let b = KeyBindings::builtin_defaults();
        let s = InputState::new(b.registry().len());
        (b, s)
    }

    #[test]
    fn press_hold_release() {
        let (b, mut s) = setup();
        s.press(kb(Key::W), &b);
        assert!(s.is_down(FORWARD) && s.was_pressed(FORWARD));
        s.end_frame();
        assert!(s.is_down(FORWARD) && !s.was_pressed(FORWARD));
        s.release(kb(Key::W), &b);
        assert!(!s.is_down(FORWARD) && s.was_released(FORWARD));
    }

    #[test]
    fn key_repeat_is_ignored() {
        let (b, mut s) = setup();
        s.press(kb(Key::Digit3), &b);
        s.press(kb(Key::Digit3), &b);
        assert_eq!(s.press_count(HOTBAR_3), 1);
    }

    #[test]
    fn modifier_combo_prefers_most_specific() {
        let (b, mut s) = setup();
        s.press(kb(Key::LeftControl), &b);
        assert!(s.is_down(SNEAK));
        s.press(kb(Key::X), &b);
        assert!(s.was_pressed(DROP_STACK));
        assert!(!s.was_pressed(DROP));
        assert!(s.is_down(SNEAK), "sneak stays held");
    }

    #[test]
    fn plain_binding_works_with_extra_modifiers() {
        let (b, mut s) = setup();
        s.press(kb(Key::LeftShift), &b); // sprint
        s.press(kb(Key::W), &b);
        assert!(s.is_down(FORWARD) && s.is_down(SPRINT));
    }

    #[test]
    fn toggle_sneak() {
        let (b, mut s) = setup();
        s.set_options(InputOptions {
            toggle_sneak: true,
            toggle_sprint: false,
        });
        s.press(kb(Key::LeftControl), &b);
        s.release(kb(Key::LeftControl), &b);
        assert!(s.is_active(SNEAK));
        assert!(!s.is_down(SNEAK));
        s.press(kb(Key::LeftControl), &b);
        s.release(kb(Key::LeftControl), &b);
        assert!(!s.is_active(SNEAK));
        // Sprint stays hold-mode.
        s.press(kb(Key::LeftShift), &b);
        assert!(s.is_active(SPRINT));
        s.release(kb(Key::LeftShift), &b);
        assert!(!s.is_active(SPRINT));
    }

    #[test]
    fn debug_chords() {
        let (b, mut s) = setup();
        s.press(kb(Key::F3), &b);
        s.press(kb(Key::G), &b);
        assert!(s.was_pressed(DEBUG_CHUNK_BORDERS));
        s.release(kb(Key::G), &b);
        s.release(kb(Key::F3), &b);
        assert!(
            !s.debug_overlay_toggled(),
            "chord suppresses the overlay toggle"
        );
        s.end_frame();
        s.press(kb(Key::F3), &b);
        s.release(kb(Key::F3), &b);
        assert!(s.debug_overlay_toggled());
        s.end_frame();
        // Without F3, A is just strafe-left.
        s.press(kb(Key::A), &b);
        assert!(s.is_down(LEFT) && !s.was_pressed(DEBUG_RELOAD_CHUNKS));
    }

    #[test]
    fn debug_chord_overrides_plain_meaning() {
        let (b, mut s) = setup();
        s.press(kb(Key::F3), &b);
        s.press(kb(Key::T), &b);
        assert!(s.was_pressed(DEBUG_RELOAD_RESOURCES));
        assert!(!s.was_pressed(CHAT));
    }

    #[test]
    fn context_switch_releases_gameplay_actions() {
        let (b, mut s) = setup();
        s.press(kb(Key::W), &b);
        s.press(kb(Key::Digit2), &b);
        s.set_context(Contexts::CONTAINER, &b);
        assert!(!s.is_down(FORWARD), "movement released when a screen opens");
        assert!(s.is_down(HOTBAR_2), "hotbar keys stay valid in containers");
        // In a container, W does nothing, X drops.
        s.release(kb(Key::W), &b);
        s.press(kb(Key::W), &b);
        assert!(!s.is_down(FORWARD));
        s.press(kb(Key::X), &b);
        assert!(s.was_pressed(DROP));
    }

    #[test]
    fn rebound_action_follows_new_key() {
        let (mut b, mut s) = setup();
        b.set(ATTACK, Some(Binding::key(Key::F)));
        s.press(InputKey::Mouse(MouseButton::Left), &b);
        assert!(!s.is_down(ATTACK));
        s.press(kb(Key::F), &b);
        assert!(s.is_down(ATTACK));
    }

    #[test]
    fn release_all_on_focus_loss() {
        let (b, mut s) = setup();
        s.press(kb(Key::W), &b);
        s.press(kb(Key::Space), &b);
        s.release_all();
        assert!(!s.is_down(FORWARD) && !s.is_down(JUMP));
        assert!(s.was_released(JUMP));
    }

    #[test]
    fn scroll_steps() {
        let (_, mut s) = setup();
        s.add_scroll(0.4);
        assert_eq!(s.take_scroll_steps(1.0, false), 0);
        s.add_scroll(0.7);
        assert_eq!(s.take_scroll_steps(1.0, false), 1);
        s.add_scroll(-0.2);
        assert_eq!(s.take_scroll_steps(1.0, true), -1);
        assert_eq!(s.take_scroll_steps(1.0, true), 0, "no scroll, no step");
    }
}
