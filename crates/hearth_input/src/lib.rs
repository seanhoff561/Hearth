//! Input handling: physical keys and mouse buttons, rebindable actions grouped by category,
//! conflict detection, modifier combos, per-frame action state with toggle/hold modes, and
//! input contexts (gameplay, container screens, debug chords).
//!
//! The crate is independent of the game loop: the client feeds it window events and queries
//! action states once per frame.

pub mod action;
pub mod bindings;
pub mod key;
pub mod state;

pub use action::{ActionDef, ActionId, ActionRegistry, Category, Contexts, Kind, builtin};
pub use bindings::{CaptureResult, KeyBindings, RebindCapture};
pub use key::{Binding, InputKey, Key, Modifiers, MouseButton, PadButton};
pub use state::{InputOptions, InputState};
