//! Per-frame input state: which actions are held, which were pressed or released since the
//! last frame, toggle latches for crouching and running, and accumulated mouse motion and
//! scrolling.
//!
//! A modifier key (Shift, Ctrl, Alt, either side) and the debug key can be bound alone and
//! still be part of combinations (E §3.1). A *hold* action on such a key starts on the press and
//! stays held through the combinations; a *tap* action waits for the key to be let go, and
//! comes only if no other key was pressed while it was held.
//!
//! On a controller, crouching, running and crawling toggle with each press: the thumb that
//! presses them is the one that steers.

use crate::action::{ActionId, Contexts, Kind, builtin};
use crate::bindings::KeyBindings;
use crate::key::{Binding, InputKey, Modifiers};

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
    /// Latched by a controller's button.
    pad_latched: bool,
}

/// A key or button being held, and what it does.
#[derive(Debug, Clone)]
struct Held {
    key: InputKey,
    /// The actions it drives while held.
    actions: Vec<ActionId>,
    /// Tap actions bound to it alone, waiting for it to be let go.
    taps: Vec<ActionId>,
    /// Another key was pressed while this one was held: its waiting taps are dropped.
    combined: bool,
}

/// Tracks raw input and derives action states.
#[derive(Debug, Clone)]
pub struct InputState {
    held: Vec<Held>,
    slots: Vec<ActionSlot>,
    context: Contexts,
    debug_key_held: bool,
    mouse_delta: (f64, f64),
    scroll_delta: f64,
    options: InputOptions,
}

impl InputState {
    pub fn new(action_count: usize) -> Self {
        Self {
            held: Vec::with_capacity(16),
            slots: vec![ActionSlot::default(); action_count],
            context: Contexts::GAMEPLAY,
            debug_key_held: false,
            mouse_delta: (0.0, 0.0),
            scroll_delta: 0.0,
            options: InputOptions::default(),
        }
    }

    pub fn set_options(&mut self, options: InputOptions) {
        self.options = options;
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
        for i in 0..self.held.len() {
            let actions = std::mem::take(&mut self.held[i].actions);
            let mut keep = Vec::with_capacity(actions.len());
            for a in actions {
                if self.action_active_in(a, bindings) {
                    keep.push(a);
                } else {
                    self.release_slot(a);
                }
            }
            self.held[i].actions = keep;
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
        Modifiers(
            self.held
                .iter()
                .fold(0, |acc, h| acc | h.key.modifier_bit()),
        )
    }

    /// The actions a press of `key` with `modifiers` held means now: those bound to it whose
    /// modifiers are held and that are active in the context, the most specific first.
    fn candidates(
        &self,
        key: InputKey,
        modifiers: Modifiers,
        bindings: &KeyBindings,
    ) -> Vec<ActionId> {
        let mut best = 0u32;
        let mut chosen: Vec<ActionId> = Vec::new();
        for id in bindings.actions_for_key(key) {
            let Some(b) = bindings.binding_for(id, key) else {
                continue;
            };
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
        chosen
    }

    /// Feeds a key or mouse button press. Returns the actions it activated now (a tap on a
    /// modifier key alone comes from [`InputState::release`] instead).
    pub fn press(&mut self, key: InputKey, bindings: &KeyBindings) -> &[ActionId] {
        if self.held.iter().any(|h| h.key == key) {
            // OS key repeat: not a new press.
            return &[];
        }
        // Whatever is held now takes part in a combination.
        for h in &mut self.held {
            h.combined = true;
        }
        let modifiers = self.modifiers();
        let debug_key = bindings.get(builtin::DEBUG).is_some_and(|b| b.key == key);
        let mut held = Held {
            key,
            actions: Vec::new(),
            taps: Vec::new(),
            combined: false,
        };
        // The system's own shortcuts (Alt+Tab, Alt+F4) are left to it.
        if !(Binding { key, modifiers }).reserved() {
            // A modifier or the debug key may yet be part of a combination: its taps wait.
            let prefix = key.modifier_bit() != 0 || debug_key;
            for id in self.candidates(key, modifiers, bindings) {
                let kind = bindings.registry().def(id).kind;
                if prefix && kind == Kind::Tap {
                    held.taps.push(id);
                    continue;
                }
                if key.is_pad() && kind == Kind::Toggleable {
                    let slot = &mut self.slots[id.0 as usize];
                    slot.pad_latched = !slot.pad_latched;
                    slot.presses = slot.presses.saturating_add(1);
                    continue;
                }
                let toggleable = kind == Kind::Toggleable && self.toggle_enabled(id);
                let slot = &mut self.slots[id.0 as usize];
                slot.holders = slot.holders.saturating_add(1);
                slot.presses = slot.presses.saturating_add(1);
                if toggleable {
                    slot.latched = !slot.latched;
                }
                held.actions.push(id);
            }
        }
        if debug_key {
            self.debug_key_held = true;
        }
        self.held.push(held);
        self.held.last().map_or(&[], |h| h.actions.as_slice())
    }

    fn toggle_enabled(&self, id: ActionId) -> bool {
        use crate::action::builtin::{SNEAK, SPRINT};
        (id == SNEAK && self.options.toggle_sneak) || (id == SPRINT && self.options.toggle_sprint)
    }

    /// Feeds a key or mouse button release. Returns the taps it fired: those bound to the key
    /// alone when no other key was pressed while it was held.
    pub fn release(&mut self, key: InputKey, bindings: &KeyBindings) -> Vec<ActionId> {
        let Some(pos) = self.held.iter().position(|h| h.key == key) else {
            return Vec::new();
        };
        let held = self.held.swap_remove(pos);
        for &a in &held.actions {
            self.release_slot(a);
        }
        let mut tapped = Vec::new();
        if !held.combined {
            for a in held.taps {
                if self.action_active_in(a, bindings) {
                    let slot = &mut self.slots[a.0 as usize];
                    slot.presses = slot.presses.saturating_add(1);
                    slot.released = true;
                    tapped.push(a);
                }
            }
        }
        if bindings.get(builtin::DEBUG).is_some_and(|b| b.key == key) {
            self.debug_key_held = false;
        }
        tapped
    }

    /// Releases everything (window focus lost); taps waiting on a key are dropped.
    pub fn release_all(&mut self) {
        for held in std::mem::take(&mut self.held) {
            for a in held.actions {
                self.release_slot(a);
            }
        }
        self.debug_key_held = false;
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

    /// For toggle-capable actions: the latched state in toggle mode, otherwise held state; or
    /// latched by a controller's button.
    pub fn is_active(&self, action: ActionId) -> bool {
        let slot = &self.slots[action.0 as usize];
        slot.pad_latched
            || if self.toggle_enabled(action) {
                slot.latched
            } else {
                self.is_down(action)
            }
    }

    /// Whether a controller's button latched the action.
    pub fn pad_latched(&self, action: ActionId) -> bool {
        self.slots[action.0 as usize].pad_latched
    }

    /// Lets go of what a controller's button latched (running, when the stick comes back).
    pub fn unlatch_pad(&mut self, action: ActionId) {
        self.slots[action.0 as usize].pad_latched = false;
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

    /// Clears per-frame edges and deltas. Call once per frame after the game consumed input.
    pub fn end_frame(&mut self) {
        for s in &mut self.slots {
            s.presses = 0;
            s.released = false;
        }
        self.mouse_delta = (0.0, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::builtin::*;
    use crate::key::{Binding, Key, MouseButton, PadButton};

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
        s.press(kb(Key::C), &b);
        assert!(s.is_down(SNEAK));
        s.press(kb(Key::LeftControl), &b);
        s.press(kb(Key::G), &b);
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
        s.press(kb(Key::C), &b);
        s.release(kb(Key::C), &b);
        assert!(s.is_active(SNEAK));
        assert!(!s.is_down(SNEAK));
        s.press(kb(Key::C), &b);
        s.release(kb(Key::C), &b);
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
        assert!(
            !s.was_pressed(DEBUG),
            "the overlay waits for F3 to be let go"
        );
        s.press(kb(Key::W), &b);
        assert!(s.was_pressed(DEBUG_TIME_WARP));
        s.release(kb(Key::W), &b);
        assert!(
            s.release(kb(Key::F3), &b).is_empty(),
            "chord suppresses the overlay toggle"
        );
        assert!(!s.was_pressed(DEBUG));
        s.end_frame();
        s.press(kb(Key::F3), &b);
        assert_eq!(s.release(kb(Key::F3), &b), vec![DEBUG]);
        assert!(s.was_pressed(DEBUG));
        s.end_frame();
        // Without F3, W is just forward.
        s.press(kb(Key::W), &b);
        assert!(s.is_down(FORWARD) && !s.was_pressed(DEBUG_TIME_WARP));
    }

    #[test]
    fn debug_chord_overrides_plain_meaning() {
        let (b, mut s) = setup();
        s.press(kb(Key::F3), &b);
        s.press(kb(Key::W), &b);
        assert!(s.was_pressed(DEBUG_TIME_WARP));
        assert!(!s.was_pressed(FORWARD) && !s.is_down(FORWARD));
    }

    #[test]
    fn context_switch_releases_gameplay_actions() {
        let (b, mut s) = setup();
        s.press(kb(Key::W), &b);
        s.press(kb(Key::Digit2), &b);
        s.set_context(Contexts::CONTAINER, &b);
        assert!(!s.is_down(FORWARD), "movement released when a screen opens");
        assert!(s.is_down(HOTBAR_2), "hotbar keys stay valid in containers");
        // In a container, W does nothing, G drops.
        s.release(kb(Key::W), &b);
        s.press(kb(Key::W), &b);
        assert!(!s.is_down(FORWARD));
        s.press(kb(Key::G), &b);
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

    /// E §3.1: a hold action on a lone modifier starts on the press and stays held through
    /// combinations made with it.
    #[test]
    fn a_held_lone_modifier_lasts_through_combinations() {
        let (mut b, mut s) = setup();
        b.set(SNEAK, Some(Binding::key(Key::LeftControl)));
        s.press(kb(Key::LeftControl), &b);
        assert!(s.is_active(SNEAK), "crouching from the press");
        s.press(kb(Key::G), &b);
        assert!(
            s.was_pressed(DROP_STACK) && !s.was_pressed(DROP),
            "Ctrl + G still works"
        );
        assert!(s.is_active(SNEAK), "still crouching");
        s.release(kb(Key::G), &b);
        assert!(s.release(kb(Key::LeftControl), &b).is_empty());
        assert!(!s.is_active(SNEAK) && s.was_released(SNEAK));
    }

    /// E §3.1: a tap action on a lone modifier comes when the key is let go, and only if no
    /// combination was made with it meanwhile.
    #[test]
    fn a_tapped_lone_modifier_waits_for_the_release() {
        let (mut b, mut s) = setup();
        b.set(JOURNAL, Some(Binding::key(Key::LeftAlt)));
        b.set(DROP_STACK, Some(Binding::with(Key::G, Modifiers::ALT)));
        assert!(s.press(kb(Key::LeftAlt), &b).is_empty());
        assert!(!s.was_pressed(JOURNAL), "not on the press");
        assert_eq!(s.release(kb(Key::LeftAlt), &b), vec![JOURNAL]);
        assert!(s.was_pressed(JOURNAL));
        s.end_frame();
        // Alt + G: the combination, and no journal.
        s.press(kb(Key::LeftAlt), &b);
        s.press(kb(Key::G), &b);
        assert!(s.was_pressed(DROP_STACK));
        s.release(kb(Key::G), &b);
        assert!(s.release(kb(Key::LeftAlt), &b).is_empty());
        assert!(!s.was_pressed(JOURNAL));
        s.end_frame();
        // Any other key or button pressed meanwhile counts as a combination.
        s.press(kb(Key::LeftAlt), &b);
        s.press(InputKey::Mouse(MouseButton::Left), &b);
        assert!(s.release(kb(Key::LeftAlt), &b).is_empty());
    }

    /// Left and right modifiers are keys of their own.
    #[test]
    fn each_side_is_its_own_key() {
        let (mut b, mut s) = setup();
        b.set(JOURNAL, Some(Binding::key(Key::RightControl)));
        b.set(INVENTORY, Some(Binding::key(Key::RightAlt)));
        s.press(kb(Key::LeftControl), &b);
        assert!(s.release(kb(Key::LeftControl), &b).is_empty());
        s.press(kb(Key::LeftAlt), &b);
        assert!(s.release(kb(Key::LeftAlt), &b).is_empty());
        s.press(kb(Key::RightControl), &b);
        assert_eq!(s.release(kb(Key::RightControl), &b), vec![JOURNAL]);
        s.press(kb(Key::RightAlt), &b);
        assert_eq!(s.release(kb(Key::RightAlt), &b), vec![INVENTORY]);
        // Either side still makes the combination.
        s.press(kb(Key::RightControl), &b);
        s.press(kb(Key::G), &b);
        assert!(s.was_pressed(DROP_STACK));
    }

    /// The system's shortcuts are left alone: Alt + F4 doesn't clear the view on its way out,
    /// and Alt + Tab doesn't open the inventory or fire a tap on Alt.
    #[test]
    fn the_systems_shortcuts_do_nothing_in_play() {
        let (mut b, mut s) = setup();
        b.set(JOURNAL, Some(Binding::key(Key::LeftAlt)));
        s.press(kb(Key::LeftAlt), &b);
        assert!(s.press(kb(Key::F4), &b).is_empty());
        assert!(!s.was_pressed(CLEAR_VIEW));
        assert!(s.press(kb(Key::Tab), &b).is_empty());
        assert!(!s.was_pressed(INVENTORY));
        s.release(kb(Key::Tab), &b);
        s.release(kb(Key::F4), &b);
        assert!(s.release(kb(Key::LeftAlt), &b).is_empty());
    }

    #[test]
    fn controller_buttons_work_actions() {
        let (b, mut s) = setup();
        let pad = |p| InputKey::Pad(p);
        s.press(pad(PadButton::South), &b);
        assert!(s.is_active(JUMP) && s.was_pressed(JUMP));
        s.release(pad(PadButton::South), &b);
        assert!(!s.is_active(JUMP));
        // Crouching toggles on the controller.
        s.press(pad(PadButton::East), &b);
        s.release(pad(PadButton::East), &b);
        assert!(s.is_active(SNEAK), "latched");
        s.press(pad(PadButton::East), &b);
        s.release(pad(PadButton::East), &b);
        assert!(!s.is_active(SNEAK), "and let go");
        // Running latches until the stick comes back.
        s.press(pad(PadButton::LeftStick), &b);
        s.release(pad(PadButton::LeftStick), &b);
        assert!(s.is_active(SPRINT));
        s.unlatch_pad(SPRINT);
        assert!(!s.is_active(SPRINT));
        // A held keyboard modifier doesn't change what a button does.
        s.press(kb(Key::LeftControl), &b);
        s.press(pad(PadButton::RightTrigger), &b);
        assert!(s.is_down(ATTACK));
    }

    #[test]
    fn focus_loss_drops_waiting_taps() {
        let (mut b, mut s) = setup();
        b.set(JOURNAL, Some(Binding::key(Key::LeftAlt)));
        s.press(kb(Key::LeftAlt), &b);
        s.release_all();
        assert!(s.release(kb(Key::LeftAlt), &b).is_empty());
        assert!(!s.was_pressed(JOURNAL));
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
