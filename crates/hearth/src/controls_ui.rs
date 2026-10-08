//! The Controls screen (E §3.1): the mouse and movement options, then every action's key and
//! controller button, each rebound by clicking it and pressing the new one; under the list, a
//! note on the one pointed at, telling how a lone modifier shares its key with combinations.

use hearth_input::{
    ActionId, Binding, CaptureResult, Contexts, InputKey, KeyBindings, Kind, RebindCapture, builtin,
};
use hearth_ui::widgets::theme;
use hearth_ui::{Column, Rect, Ui};

use crate::menus::{MenuAction, MenuContext, ROW, W};

/// The column of controller buttons.
const PAD_COLUMN: f32 = 76.0;

/// What the Controls screen keeps between frames.
#[derive(Debug, Clone, Default)]
pub struct ControlsScreen {
    /// The action waiting for a key or a button.
    pub capturing: Option<ActionId>,
    /// Waiting for a controller's button rather than a key.
    pub pad: bool,
    pub capture: RebindCapture,
    /// A system shortcut just pressed, which can't be bound.
    pub refused: Option<Binding>,
}

impl ControlsScreen {
    /// A key or button pressed or let go while capturing a binding: Ctrl, Alt or Shift let go
    /// with nothing else pressed binds that key alone; the system's own shortcuts are refused;
    /// a controller's button binds the controller's column. Whether the options changed.
    pub fn capture(&mut self, key: InputKey, pressed: bool, bindings: &mut KeyBindings) -> bool {
        let Some(action) = self.capturing else {
            return false;
        };
        if self.pad {
            // A button binds; Escape unbinds; other keys are not for this column.
            let button = match key {
                InputKey::Pad(b) if pressed => Some(b),
                InputKey::Keyboard(hearth_input::Key::Escape) if pressed => None,
                _ => return false,
            };
            bindings.set_pad(action, button);
            self.capturing = None;
            return true;
        }
        if key.is_pad() {
            return false;
        }
        let result = if pressed {
            self.capture.press(key)
        } else {
            self.capture.release(key)
        };
        match result {
            CaptureResult::Pending => false,
            CaptureResult::Done(Some(b)) if b.reserved() => {
                self.refused = Some(b);
                self.capture = RebindCapture::new();
                false
            }
            CaptureResult::Done(binding) => {
                bindings.set(action, binding);
                self.capturing = None;
                self.capture = RebindCapture::new();
                self.refused = None;
                true
            }
        }
    }

    /// Draws the screen; whether Done was pressed.
    pub fn draw(
        &mut self,
        ui: &mut Ui<'_>,
        cx: &mut MenuContext<'_>,
        out: &mut Vec<MenuAction>,
    ) -> bool {
        let size = ui.size;
        ui.title(10.0, &ui.t("menu.options.controls"));
        let mut changed = false;
        let wide = W + 140.0;
        let left = ((size.0 - wide) / 2.0).round();
        let mut c = Column::new(left, 24.0, wide);
        c.gap = 3.0;
        let ctl = &mut cx.options.controls;
        let row = c.row(ROW);
        let (a, b) = row.split_left((wide - 4.0) / 2.0, 4.0);
        let mut sens = ctl.mouse_sensitivity;
        let text = format!("{}%", (sens * 200.0).round());
        if ui.slider(
            a,
            &ui.t("menu.controls.sensitivity"),
            &mut sens,
            0.0,
            1.0,
            &text,
        ) {
            ctl.mouse_sensitivity = sens;
            changed = true;
        }
        let mut inv = ctl.invert_y;
        if ui.toggle(b, &ui.t("menu.controls.invert_y"), &mut inv) {
            ctl.invert_y = inv;
            changed = true;
        }
        let row = c.row(ROW);
        let (a, b) = row.split_left((wide - 4.0) / 2.0, 4.0);
        let mut ts = ctl.toggle_sneak;
        if ui.toggle(a, &ui.t("menu.controls.toggle_sneak"), &mut ts) {
            ctl.toggle_sneak = ts;
            changed = true;
        }
        let mut tr = ctl.toggle_sprint;
        if ui.toggle(b, &ui.t("menu.controls.toggle_sprint"), &mut tr) {
            ctl.toggle_sprint = tr;
            changed = true;
        }
        // The hands (P §5.1–5.2): things put away come back, the name and the hands' hints by
        // the crosshair, and forgetting what the hands learned from the action menu.
        let row = c.row(ROW);
        let (a, b) = row.split_left((wide - 4.0) / 2.0, 4.0);
        changed |= ui.toggle(
            a,
            &ui.t("menu.controls.return_to_hand"),
            &mut ctl.return_to_hand,
        );
        changed |= ui.toggle(b, &ui.t("menu.controls.hand_hints"), &mut ctl.hand_hints);
        let row = c.row(ROW);
        let (a, b) = row.split_left((wide - 4.0) / 2.0, 4.0);
        use hearth_core::options::NameTags;
        let tags = [NameTags::Off, NameTags::Brief, NameTags::Always];
        let names: Vec<String> = ["off", "brief", "always"]
            .iter()
            .map(|k| ui.t(&format!("menu.controls.name_tags.{k}")))
            .collect();
        let mut i = tags.iter().position(|t| *t == ctl.name_tags).unwrap_or(1);
        if ui.cycle(a, &ui.t("menu.controls.name_tags"), &names, &mut i) {
            ctl.name_tags = tags[i];
            changed = true;
        }
        if ui.button_enabled(b, &ui.t("menu.controls.forget_hands"), cx.in_game) {
            out.push(MenuAction::ForgetHandUses);
        }
        // The key bindings, and under them a note on the one pointed at.
        let ids: Vec<ActionId> = cx.bindings.registry().ids().collect();
        let line = hearth_ui::font::LINE as f32;
        let list_h = (size.1 - c.y - 34.0 - 2.0 * line).max(40.0);
        let list_r = Rect::new(left, c.y, wide, list_h);
        let bindings = &*cx.bindings;
        let waiting = self.capturing;
        let pad_column = self.pad;
        let press_key = ui.t("menu.controls.press_key");
        let press_button = ui.t("menu.controls.press_button");
        let unbound = ui.t("menu.controls.unbound");
        let mut pointed = None;
        let clicked = ui.list(
            list_r,
            "bindings",
            ids.len(),
            12.0,
            waiting.and_then(|w| ids.iter().position(|i| *i == w)),
            |ui, r, i, _| {
                let id = ids[i];
                if ui.input.pointer.is_some_and(|p| r.contains(p)) {
                    pointed = Some(id);
                }
                let def = bindings.registry().def(id);
                ui.label(r.x + 4.0, r.y + 2.0, ui.lang.get(&def.id), theme::TEXT);
                let here = waiting == Some(id);
                let b = if here && !pad_column {
                    press_key.clone()
                } else {
                    binding_words(bindings, id).unwrap_or_else(|| unbound.clone())
                };
                let color = |conflict: bool| if conflict { theme::WARN } else { theme::DIM };
                let bw = ui.font.width(&b) as f32;
                ui.label(
                    r.x + r.w - PAD_COLUMN - bw - 8.0,
                    r.y + 2.0,
                    &b,
                    color(bindings.is_conflicting(id)),
                );
                // The controller's button, in its own column.
                let p = if here && pad_column {
                    press_button.clone()
                } else {
                    bindings
                        .pad(id)
                        .map_or("–", |b| b.display_name())
                        .to_owned()
                };
                let pw = ui.font.width(&p) as f32;
                ui.label(
                    r.x + r.w - pw - 8.0,
                    r.y + 2.0,
                    &p,
                    color(!bindings.pad_conflicts_of(id).is_empty()),
                );
            },
        );
        if let Some(i) = clicked {
            self.capturing = Some(ids[i]);
            self.pad = ui
                .input
                .pointer
                .is_some_and(|(x, _)| x > list_r.x + list_r.w - PAD_COLUMN - 4.0);
            self.refused = None;
        }
        let note = match (self.refused, waiting.or(pointed)) {
            (Some(b), _) if waiting.is_some() => ui
                .lang
                .format("menu.controls.reserved", &[("binding", &b.display_name())]),
            (_, Some(id)) => {
                overlap_note(ui, bindings, id).unwrap_or_else(|| ui.t("menu.controls.hint"))
            }
            _ => ui.t("menu.controls.hint"),
        };
        let mut y = list_r.y + list_r.h + 2.0;
        for text in ui.font.wrap(&note, wide as u32).into_iter().take(2) {
            ui.label(left, y, &text, theme::DIM);
            y += line;
        }
        let mut c2 = Column::new(left, list_r.y + list_r.h + 2.0 * line + 6.0, wide);
        let row = c2.row(ROW);
        let (a, b) = row.split_left((wide - 4.0) / 2.0, 4.0);
        if ui.button(a, &ui.t("menu.controls.reset_all")) {
            cx.bindings.reset_all();
            changed = true;
        }
        let done = ui.button(b, &ui.t("menu.done"));
        if changed {
            out.push(MenuAction::OptionsChanged);
        }
        done
    }
}

/// An action's binding in words; a debug chord with the debug key ("F3 + W").
pub(crate) fn binding_words(bindings: &KeyBindings, id: ActionId) -> Option<String> {
    let b = bindings.get(id)?;
    let chord = bindings
        .registry()
        .def(id)
        .contexts
        .contains(Contexts::DEBUG_CHORD);
    Some(match bindings.get(builtin::DEBUG) {
        Some(d) if chord => format!("{} + {}", d.display_name(), b.display_name()),
        _ => b.display_name(),
    })
}

/// How an action's key is shared (E §3.1), in words: a modifier (or the debug key) bound alone
/// is held through the combinations made with it, or, tapped, comes when let go with no other
/// key pressed; a combination works alongside it.
fn overlap_note(ui: &Ui<'_>, bindings: &KeyBindings, id: ActionId) -> Option<String> {
    let others = bindings.overlaps_of(id);
    let first = *others.first()?;
    let b = bindings.get(id)?;
    let o = bindings.get(first)?;
    let name = |a: ActionId| ui.lang.get(&bindings.registry().def(a).id).to_owned();
    let words = |a: ActionId| binding_words(bindings, a).unwrap_or_default();
    let tap = |a: ActionId| bindings.registry().def(a).kind == Kind::Tap;
    // This action is the key alone when the other's binding is made with its key.
    let alone = o.key != b.key
        && (o.modifiers.0 & b.key.modifier_bit() != 0
            || bindings.get(builtin::DEBUG).is_some_and(|d| d.key == b.key));
    if alone {
        let mut with: Vec<String> = others
            .iter()
            .take(2)
            .map(|&a| format!("{} ({})", words(a), name(a)))
            .collect();
        if others.len() > 2 {
            with.push("…".into());
        }
        let key = if tap(id) {
            "menu.controls.overlap.tap"
        } else {
            "menu.controls.overlap.hold"
        };
        Some(ui.lang.format(
            key,
            &[
                ("key", &words(id)),
                ("action", &name(id)),
                ("with", &with.join(", ")),
            ],
        ))
    } else {
        let key = if tap(first) {
            "menu.controls.overlap.combo_tap"
        } else {
            "menu.controls.overlap.combo_hold"
        };
        Some(ui.lang.format(
            key,
            &[
                ("binding", &words(id)),
                ("key", &words(first)),
                ("action", &name(first)),
            ],
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menus::{Menus, Screen};
    use hearth_core::options::Options;
    use hearth_input::{ActionRegistry, Key, Modifiers};
    use hearth_ui::{DrawList, Font, UiInput, UiState};

    /// Draws the menus for a frame of 640×360 interface pixels with `input`, on the CPU.
    fn frame(
        font: &Font,
        menus: &mut Menus,
        bindings: &mut KeyBindings,
        state: &mut UiState,
        input: &UiInput,
    ) {
        let lang = crate::interface::lang();
        let mut options = Options::default();
        let mut profiles = crate::profiles::Profiles::default();
        let languages = vec!["en_us".to_owned()];
        let mut draw = DrawList::new(1);
        let mut ui = Ui::begin(font, &lang, &mut draw, input, state, (640.0, 360.0));
        let mut cx = MenuContext {
            options: &mut options,
            bindings,
            saves: std::env::temp_dir().join("hearth-controls-test-saves"),
            in_game: true,
            languages: &languages,
            audio_devices: &[],
            profiles: &mut profiles,
            death: None,
            inventory: None,
            journal: None,
            eras: Vec::new(),
            modes: Vec::new(),
            globe: None,
            time_words: None,
            waiting: None,
            may_watch: true,
            creative: false,
            catalog: &[],
            instant: false,
            clear_view: Default::default(),
        };
        menus.ui(&mut ui, &mut cx);
    }

    /// E §3.1 and §11: Left Ctrl, Right Ctrl, Left Alt and Right Alt each bound alone through
    /// the Controls screen: the action's row clicked, the key pressed and let go.
    #[test]
    fn ctrl_and_alt_bind_alone_through_the_controls_screen() {
        let font = Font::new();
        let mut bindings = KeyBindings::new(ActionRegistry::with_builtins());
        let mut menus = Menus::none();
        menus.open(Screen::controls());
        let mut state = UiState::recording();
        frame(
            &font,
            &mut menus,
            &mut bindings,
            &mut state,
            &UiInput::default(),
        );
        // The list of bindings: the tall unlabelled area.
        let list = state
            .layout
            .as_ref()
            .and_then(|l| l.iter().find(|p| p.text.is_empty() && p.rect.h > 100.0))
            .map(|p| p.rect)
            .expect("the bindings list");
        // Crouching's row.
        let at = Some((list.x + 20.0, list.y + 12.0 * builtin::SNEAK.0 as f32 + 6.0));
        for key in [
            Key::LeftControl,
            Key::RightControl,
            Key::LeftAlt,
            Key::RightAlt,
        ] {
            let press = UiInput {
                pointer: at,
                pressed: true,
                down: true,
                ..Default::default()
            };
            frame(&font, &mut menus, &mut bindings, &mut state, &press);
            let release = UiInput {
                pointer: at,
                released: true,
                ..Default::default()
            };
            frame(&font, &mut menus, &mut bindings, &mut state, &release);
            assert!(menus.capturing(), "waiting for a key after the click");
            let k = InputKey::Keyboard(key);
            assert!(
                !menus.capture(k, true, &mut bindings),
                "{key:?} pressed: still waiting"
            );
            assert!(
                menus.capture(k, false, &mut bindings),
                "{key:?} let go: bound"
            );
            assert!(!menus.capturing());
            assert_eq!(bindings.get(builtin::SNEAK), Some(Binding::key(key)));
        }
        // A combination still binds as one.
        frame(
            &font,
            &mut menus,
            &mut bindings,
            &mut state,
            &UiInput {
                pointer: at,
                pressed: true,
                down: true,
                ..Default::default()
            },
        );
        frame(
            &font,
            &mut menus,
            &mut bindings,
            &mut state,
            &UiInput {
                pointer: at,
                released: true,
                ..Default::default()
            },
        );
        let ctrl = InputKey::Keyboard(Key::RightControl);
        assert!(!menus.capture(ctrl, true, &mut bindings));
        assert!(menus.capture(InputKey::Keyboard(Key::K), true, &mut bindings));
        assert_eq!(
            bindings.get(builtin::SNEAK),
            Some(Binding::with(Key::K, Modifiers::CTRL))
        );
        // The release that follows the combination does nothing more.
        assert!(!menus.capture(ctrl, false, &mut bindings));
    }

    /// Every controller button can be bound, in the controller's column.
    #[test]
    fn controller_buttons_bind_in_their_column() {
        let font = Font::new();
        let mut bindings = KeyBindings::new(ActionRegistry::with_builtins());
        let mut menus = Menus::none();
        menus.open(Screen::controls());
        let mut state = UiState::recording();
        frame(
            &font,
            &mut menus,
            &mut bindings,
            &mut state,
            &UiInput::default(),
        );
        let list = state
            .layout
            .as_ref()
            .and_then(|l| l.iter().find(|p| p.text.is_empty() && p.rect.h > 100.0))
            .map(|p| p.rect)
            .expect("the bindings list");
        let row_y = list.y + 12.0 * builtin::SNEAK.0 as f32 + 6.0;
        let click = |menus: &mut Menus, bindings: &mut KeyBindings, state: &mut UiState, x| {
            for input in [
                UiInput {
                    pointer: Some((x, row_y)),
                    pressed: true,
                    down: true,
                    ..Default::default()
                },
                UiInput {
                    pointer: Some((x, row_y)),
                    released: true,
                    ..Default::default()
                },
            ] {
                frame(&font, menus, bindings, state, &input);
            }
        };
        for b in hearth_input::PadButton::ALL {
            click(
                &mut menus,
                &mut bindings,
                &mut state,
                list.x + list.w - 20.0,
            );
            assert!(menus.capturing());
            assert!(
                !menus.capture(InputKey::Keyboard(Key::K), true, &mut bindings),
                "a key doesn't bind the controller's column"
            );
            assert!(menus.capture(InputKey::Pad(b), true, &mut bindings));
            assert_eq!(bindings.pad(builtin::SNEAK), Some(b));
            assert_eq!(
                bindings.get(builtin::SNEAK),
                Some(Binding::key(Key::C)),
                "key kept"
            );
        }
        // Escape unbinds the button.
        click(
            &mut menus,
            &mut bindings,
            &mut state,
            list.x + list.w - 20.0,
        );
        assert!(menus.capture(InputKey::Keyboard(Key::Escape), true, &mut bindings));
        assert_eq!(bindings.pad(builtin::SNEAK), None);
        // In the key's column a button waits for a key.
        click(&mut menus, &mut bindings, &mut state, list.x + 20.0);
        let south = InputKey::Pad(hearth_input::PadButton::South);
        assert!(!menus.capture(south, true, &mut bindings));
        assert!(menus.capture(InputKey::Keyboard(Key::K), true, &mut bindings));
        assert_eq!(bindings.get(builtin::SNEAK), Some(Binding::key(Key::K)));
        // Mouse side buttons bind like keys.
        click(&mut menus, &mut bindings, &mut state, list.x + 20.0);
        let side = InputKey::Mouse(hearth_input::MouseButton::Forward);
        assert!(menus.capture(side, true, &mut bindings));
        assert_eq!(
            bindings.get(builtin::SNEAK),
            Some(Binding::mouse(hearth_input::MouseButton::Forward))
        );
    }

    /// The system's shortcuts are refused and the screen keeps waiting.
    #[test]
    fn system_shortcuts_are_refused() {
        let mut bindings = KeyBindings::new(ActionRegistry::with_builtins());
        let mut menus = Menus::none();
        menus.open(Screen::Controls(ControlsScreen {
            capturing: Some(builtin::JOURNAL),
            ..Default::default()
        }));
        let alt = InputKey::Keyboard(Key::LeftAlt);
        assert!(!menus.capture(alt, true, &mut bindings));
        assert!(!menus.capture(InputKey::Keyboard(Key::Tab), true, &mut bindings));
        assert!(menus.capturing(), "Alt + Tab refused; still waiting");
        assert!(matches!(
            menus.top_mut(),
            Some(Screen::Controls(ControlsScreen {
                refused: Some(_),
                ..
            }))
        ));
        // Alt let go after the refusal binds nothing.
        assert!(!menus.capture(alt, false, &mut bindings));
        assert_eq!(bindings.get(builtin::JOURNAL), Some(Binding::key(Key::J)));
        // Focus lost with a modifier held: it is forgotten.
        assert!(!menus.capture(alt, true, &mut bindings));
        menus.capture_reset();
        assert!(!menus.capture(alt, false, &mut bindings));
        assert!(menus.capturing());
    }

    /// Overlaps are explained, not flagged.
    #[test]
    fn overlaps_are_explained() {
        let font = Font::new();
        let lang = crate::interface::lang();
        let mut bindings = KeyBindings::new(ActionRegistry::with_builtins());
        bindings.set(builtin::SNEAK, Some(Binding::key(Key::LeftControl)));
        bindings.set(builtin::JOURNAL, Some(Binding::key(Key::RightAlt)));
        bindings.set(
            builtin::BODY_PANEL,
            Some(Binding::with(Key::B, Modifiers::ALT)),
        );
        let (mut state, input, mut draw) =
            (UiState::default(), UiInput::default(), DrawList::new(1));
        let ui = Ui::begin(&font, &lang, &mut draw, &input, &mut state, (640.0, 360.0));
        let note = |id| overlap_note(&ui, &bindings, id).unwrap_or_default();
        assert!(!bindings.is_conflicting(builtin::SNEAK));
        assert_eq!(
            note(builtin::SNEAK),
            "Left Control alone: Crouch, dive from the press, held through Ctrl + G (Drop all), \
             Ctrl + Middle Button (Pick (Creative))."
        );
        assert_eq!(
            note(builtin::DROP_STACK),
            "Ctrl + G works while Left Control holds Crouch, dive."
        );
        assert!(
            note(builtin::JOURNAL)
                .starts_with("Right Alt alone: Journal when let go with no other key"),
            "{}",
            note(builtin::JOURNAL)
        );
        assert!(note(builtin::FORWARD).is_empty(), "W overlaps nothing");
        assert_eq!(
            binding_words(&bindings, builtin::DEBUG_TIME_WARP).as_deref(),
            Some("F3 + W")
        );
    }
}
