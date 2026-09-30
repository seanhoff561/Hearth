//! Physical input identifiers (keyboard keys and mouse buttons), modifier sets, and bindings,
//! with stable string names used in `options.toml`.

use std::fmt;
use std::str::FromStr;

macro_rules! keys {
    ($( $variant:ident => $winit:ident, $name:literal, $display:literal; )*) => {
        /// A physical keyboard key (layout independent, named after the US layout).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Key {
            $( $variant, )*
        }

        impl Key {
            /// All keys, in declaration order.
            pub const ALL: &'static [Key] = &[ $( Key::$variant, )* ];

            /// Stable config name, e.g. `left_shift`.
            pub fn name(self) -> &'static str {
                match self { $( Key::$variant => $name, )* }
            }

            /// Short label for UI display, e.g. `Left Shift`.
            pub fn display_name(self) -> &'static str {
                match self { $( Key::$variant => $display, )* }
            }

            /// Parses a config name.
            pub fn from_name(name: &str) -> Option<Key> {
                match name { $( $name => Some(Key::$variant), )* _ => None }
            }

            /// Converts from winit's physical key code.
            pub fn from_winit(code: winit::keyboard::KeyCode) -> Option<Key> {
                use winit::keyboard::KeyCode as K;
                match code { $( K::$winit => Some(Key::$variant), )* _ => None }
            }
        }
    };
}

keys! {
    A => KeyA, "a", "A"; B => KeyB, "b", "B"; C => KeyC, "c", "C"; D => KeyD, "d", "D";
    E => KeyE, "e", "E"; F => KeyF, "f", "F"; G => KeyG, "g", "G"; H => KeyH, "h", "H";
    I => KeyI, "i", "I"; J => KeyJ, "j", "J"; K => KeyK, "k", "K"; L => KeyL, "l", "L";
    M => KeyM, "m", "M"; N => KeyN, "n", "N"; O => KeyO, "o", "O"; P => KeyP, "p", "P";
    Q => KeyQ, "q", "Q"; R => KeyR, "r", "R"; S => KeyS, "s", "S"; T => KeyT, "t", "T";
    U => KeyU, "u", "U"; V => KeyV, "v", "V"; W => KeyW, "w", "W"; X => KeyX, "x", "X";
    Y => KeyY, "y", "Y"; Z => KeyZ, "z", "Z";
    Digit0 => Digit0, "0", "0"; Digit1 => Digit1, "1", "1"; Digit2 => Digit2, "2", "2";
    Digit3 => Digit3, "3", "3"; Digit4 => Digit4, "4", "4"; Digit5 => Digit5, "5", "5";
    Digit6 => Digit6, "6", "6"; Digit7 => Digit7, "7", "7"; Digit8 => Digit8, "8", "8";
    Digit9 => Digit9, "9", "9";
    F1 => F1, "f1", "F1"; F2 => F2, "f2", "F2"; F3 => F3, "f3", "F3"; F4 => F4, "f4", "F4";
    F5 => F5, "f5", "F5"; F6 => F6, "f6", "F6"; F7 => F7, "f7", "F7"; F8 => F8, "f8", "F8";
    F9 => F9, "f9", "F9"; F10 => F10, "f10", "F10"; F11 => F11, "f11", "F11";
    F12 => F12, "f12", "F12"; F13 => F13, "f13", "F13"; F14 => F14, "f14", "F14";
    F15 => F15, "f15", "F15"; F16 => F16, "f16", "F16"; F17 => F17, "f17", "F17";
    F18 => F18, "f18", "F18"; F19 => F19, "f19", "F19"; F20 => F20, "f20", "F20";
    F21 => F21, "f21", "F21"; F22 => F22, "f22", "F22"; F23 => F23, "f23", "F23";
    F24 => F24, "f24", "F24";
    Up => ArrowUp, "up", "Up Arrow"; Down => ArrowDown, "down", "Down Arrow";
    Left => ArrowLeft, "left", "Left Arrow"; Right => ArrowRight, "right", "Right Arrow";
    LeftShift => ShiftLeft, "left_shift", "Left Shift";
    RightShift => ShiftRight, "right_shift", "Right Shift";
    LeftControl => ControlLeft, "left_control", "Left Control";
    RightControl => ControlRight, "right_control", "Right Control";
    LeftAlt => AltLeft, "left_alt", "Left Alt"; RightAlt => AltRight, "right_alt", "Right Alt";
    LeftSuper => SuperLeft, "left_super", "Left Win"; RightSuper => SuperRight, "right_super", "Right Win";
    Space => Space, "space", "Space"; Enter => Enter, "enter", "Enter";
    Escape => Escape, "escape", "Escape"; Backspace => Backspace, "backspace", "Backspace";
    Tab => Tab, "tab", "Tab"; CapsLock => CapsLock, "caps_lock", "Caps Lock";
    Insert => Insert, "insert", "Insert"; Delete => Delete, "delete", "Delete";
    Home => Home, "home", "Home"; End => End, "end", "End";
    PageUp => PageUp, "page_up", "Page Up"; PageDown => PageDown, "page_down", "Page Down";
    PrintScreen => PrintScreen, "print_screen", "Print Screen";
    ScrollLock => ScrollLock, "scroll_lock", "Scroll Lock"; Pause => Pause, "pause", "Pause";
    NumLock => NumLock, "num_lock", "Num Lock"; Menu => ContextMenu, "menu", "Menu";
    Minus => Minus, "minus", "-"; Equal => Equal, "equal", "=";
    LeftBracket => BracketLeft, "left_bracket", "["; RightBracket => BracketRight, "right_bracket", "]";
    Backslash => Backslash, "backslash", "\\"; Semicolon => Semicolon, "semicolon", ";";
    Apostrophe => Quote, "apostrophe", "'"; Grave => Backquote, "grave", "`";
    Comma => Comma, "comma", ","; Period => Period, "period", ".";
    Slash => Slash, "slash", "/"; IntlBackslash => IntlBackslash, "world_1", "World 1";
    Numpad0 => Numpad0, "keypad.0", "Keypad 0"; Numpad1 => Numpad1, "keypad.1", "Keypad 1";
    Numpad2 => Numpad2, "keypad.2", "Keypad 2"; Numpad3 => Numpad3, "keypad.3", "Keypad 3";
    Numpad4 => Numpad4, "keypad.4", "Keypad 4"; Numpad5 => Numpad5, "keypad.5", "Keypad 5";
    Numpad6 => Numpad6, "keypad.6", "Keypad 6"; Numpad7 => Numpad7, "keypad.7", "Keypad 7";
    Numpad8 => Numpad8, "keypad.8", "Keypad 8"; Numpad9 => Numpad9, "keypad.9", "Keypad 9";
    NumpadAdd => NumpadAdd, "keypad.add", "Keypad +";
    NumpadSubtract => NumpadSubtract, "keypad.subtract", "Keypad -";
    NumpadMultiply => NumpadMultiply, "keypad.multiply", "Keypad *";
    NumpadDivide => NumpadDivide, "keypad.divide", "Keypad /";
    NumpadDecimal => NumpadDecimal, "keypad.decimal", "Keypad .";
    NumpadEnter => NumpadEnter, "keypad.enter", "Keypad Enter";
    NumpadEqual => NumpadEqual, "keypad.equal", "Keypad =";
}

impl Key {
    /// True for Shift/Control/Alt keys, which can also act as binding modifiers.
    pub fn is_modifier(self) -> bool {
        self.modifier_bit() != 0
    }

    /// The modifier bit this key contributes while held (0 if it is not a modifier key).
    pub fn modifier_bit(self) -> u8 {
        match self {
            Key::LeftShift | Key::RightShift => Modifiers::SHIFT.0,
            Key::LeftControl | Key::RightControl => Modifiers::CTRL.0,
            Key::LeftAlt | Key::RightAlt => Modifiers::ALT.0,
            _ => 0,
        }
    }

    /// Digit keys 1–9 map to hotbar slots 0–8.
    pub fn hotbar_index(self) -> Option<usize> {
        Some(match self {
            Key::Digit1 => 0,
            Key::Digit2 => 1,
            Key::Digit3 => 2,
            Key::Digit4 => 3,
            Key::Digit5 => 4,
            Key::Digit6 => 5,
            Key::Digit7 => 6,
            Key::Digit8 => 7,
            Key::Digit9 => 8,
            _ => return None,
        })
    }
}

/// A mouse button. `Back` is usually "mouse 4" and `Forward` "mouse 5".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}

impl MouseButton {
    pub fn from_winit(b: winit::event::MouseButton) -> MouseButton {
        use winit::event::MouseButton as W;
        match b {
            W::Left => MouseButton::Left,
            W::Right => MouseButton::Right,
            W::Middle => MouseButton::Middle,
            W::Back => MouseButton::Back,
            W::Forward => MouseButton::Forward,
            W::Other(n) => MouseButton::Other(n),
        }
    }

    fn name(self) -> String {
        match self {
            MouseButton::Left => "mouse.left".into(),
            MouseButton::Right => "mouse.right".into(),
            MouseButton::Middle => "mouse.middle".into(),
            MouseButton::Back => "mouse.4".into(),
            MouseButton::Forward => "mouse.5".into(),
            MouseButton::Other(n) => format!("mouse.{}", n as u32 + 6),
        }
    }

    fn from_name(s: &str) -> Option<MouseButton> {
        let rest = s.strip_prefix("mouse.")?;
        Some(match rest {
            "left" => MouseButton::Left,
            "right" => MouseButton::Right,
            "middle" => MouseButton::Middle,
            "4" => MouseButton::Back,
            "5" => MouseButton::Forward,
            n => {
                let n: u32 = n.parse().ok()?;
                if !(6..=u16::MAX as u32 + 6).contains(&n) {
                    return None;
                }
                MouseButton::Other((n - 6) as u16)
            }
        })
    }

    pub fn display_name(self) -> String {
        match self {
            MouseButton::Left => "Left Button".into(),
            MouseButton::Right => "Right Button".into(),
            MouseButton::Middle => "Middle Button".into(),
            MouseButton::Back => "Button 4".into(),
            MouseButton::Forward => "Button 5".into(),
            MouseButton::Other(n) => format!("Button {}", n as u32 + 6),
        }
    }
}

/// Any bindable physical input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InputKey {
    Keyboard(Key),
    Mouse(MouseButton),
}

impl InputKey {
    pub fn name(self) -> String {
        match self {
            InputKey::Keyboard(k) => k.name().to_owned(),
            InputKey::Mouse(b) => b.name(),
        }
    }

    pub fn from_name(s: &str) -> Option<InputKey> {
        if s.starts_with("mouse.") {
            MouseButton::from_name(s).map(InputKey::Mouse)
        } else {
            Key::from_name(s).map(InputKey::Keyboard)
        }
    }

    pub fn display_name(self) -> String {
        match self {
            InputKey::Keyboard(k) => k.display_name().to_owned(),
            InputKey::Mouse(b) => b.display_name(),
        }
    }

    pub fn modifier_bit(self) -> u8 {
        match self {
            InputKey::Keyboard(k) => k.modifier_bit(),
            InputKey::Mouse(_) => 0,
        }
    }
}

/// A set of held modifier keys (either side counts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Modifiers(pub u8);

impl Modifiers {
    pub const NONE: Modifiers = Modifiers(0);
    pub const SHIFT: Modifiers = Modifiers(1);
    pub const CTRL: Modifiers = Modifiers(2);
    pub const ALT: Modifiers = Modifiers(4);

    pub fn contains(self, other: Modifiers) -> bool {
        self.0 & other.0 == other.0
    }
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub fn union(self, other: Modifiers) -> Modifiers {
        Modifiers(self.0 | other.0)
    }
    pub fn count(self) -> u32 {
        self.0.count_ones()
    }
}

/// A binding: a key plus required modifiers (e.g. `ctrl+x`). `None` in the binding table means
/// "unbound".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Binding {
    pub key: InputKey,
    pub modifiers: Modifiers,
}

impl Binding {
    pub const fn key(key: Key) -> Binding {
        Binding {
            key: InputKey::Keyboard(key),
            modifiers: Modifiers::NONE,
        }
    }

    pub const fn mouse(b: MouseButton) -> Binding {
        Binding {
            key: InputKey::Mouse(b),
            modifiers: Modifiers::NONE,
        }
    }

    pub const fn with(key: Key, modifiers: Modifiers) -> Binding {
        Binding {
            key: InputKey::Keyboard(key),
            modifiers,
        }
    }

    /// Human-readable label, e.g. `Ctrl + X`.
    pub fn display_name(&self) -> String {
        let mut s = String::new();
        if self.modifiers.contains(Modifiers::CTRL) {
            s.push_str("Ctrl + ");
        }
        if self.modifiers.contains(Modifiers::SHIFT) {
            s.push_str("Shift + ");
        }
        if self.modifiers.contains(Modifiers::ALT) {
            s.push_str("Alt + ");
        }
        s.push_str(&self.key.display_name());
        s
    }
}

impl fmt::Display for Binding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.modifiers.contains(Modifiers::CTRL) {
            f.write_str("ctrl+")?;
        }
        if self.modifiers.contains(Modifiers::SHIFT) {
            f.write_str("shift+")?;
        }
        if self.modifiers.contains(Modifiers::ALT) {
            f.write_str("alt+")?;
        }
        f.write_str(&self.key.name())
    }
}

/// Error parsing a binding string.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown key binding {0:?}")]
pub struct BindingParseError(pub String);

impl FromStr for Binding {
    type Err = BindingParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || BindingParseError(s.to_owned());
        let lower = s.trim().to_ascii_lowercase();
        let mut parts: Vec<&str> = lower.split('+').map(str::trim).collect();
        let key_part = parts.pop().ok_or_else(err)?;
        let key = InputKey::from_name(key_part).ok_or_else(err)?;
        let mut modifiers = Modifiers::NONE;
        for m in parts {
            let bit = match m {
                "ctrl" | "control" => Modifiers::CTRL,
                "shift" => Modifiers::SHIFT,
                "alt" => Modifiers::ALT,
                _ => return Err(err()),
            };
            modifiers = modifiers.union(bit);
        }
        // A modifier key can't require itself (e.g. "shift+left_shift").
        if modifiers.0 & key.modifier_bit() != 0 {
            return Err(err());
        }
        Ok(Binding { key, modifiers })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_names_round_trip() {
        for &k in Key::ALL {
            assert_eq!(Key::from_name(k.name()), Some(k), "{k:?}");
        }
    }

    #[test]
    fn mouse_names_round_trip() {
        for b in [
            MouseButton::Left,
            MouseButton::Right,
            MouseButton::Middle,
            MouseButton::Back,
            MouseButton::Forward,
            MouseButton::Other(0),
            MouseButton::Other(7),
        ] {
            let k = InputKey::Mouse(b);
            assert_eq!(InputKey::from_name(&k.name()), Some(k));
        }
    }

    #[test]
    fn binding_parse_and_format() {
        let b: Binding = "ctrl+x".parse().unwrap();
        assert_eq!(b, Binding::with(Key::X, Modifiers::CTRL));
        assert_eq!(b.to_string(), "ctrl+x");
        assert_eq!(b.display_name(), "Ctrl + X");
        let m: Binding = "mouse.5".parse().unwrap();
        assert_eq!(m, Binding::mouse(MouseButton::Forward));
        let combo: Binding = "Shift + Ctrl + F3".parse().unwrap();
        assert_eq!(combo.modifiers, Modifiers::CTRL.union(Modifiers::SHIFT));
        assert_eq!(combo.to_string(), "ctrl+shift+f3");
        assert!("hyper+x".parse::<Binding>().is_err());
        assert!("nokey".parse::<Binding>().is_err());
        assert!("shift+left_shift".parse::<Binding>().is_err());
    }
}
