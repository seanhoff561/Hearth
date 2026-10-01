//! The game's interface (v1 M11 framework, V2-3): its own pixel font ([`font`]), lists of
//! rectangles and text to draw in interface pixels ([`draw`]), and the words of the interface in
//! the player's language ([`lang`]). The renderer (`hearth_render::ui`) draws the lists on top of
//! the frame; nothing here touches the GPU.

pub mod draw;
pub mod font;
mod glyphs;
pub mod lang;

pub use draw::{DrawList, Rgba, UiVertex};
pub use font::Font;
pub use lang::Lang;

/// The interface's scale: screen pixels per interface pixel. `setting` 0 picks the largest
/// that keeps 300 interface pixels of height (about 4 at 1440p, 3 at 1080p).
pub fn gui_scale(setting: u32, window_height: u32) -> u32 {
    let auto = (window_height / 300).max(1);
    if setting == 0 {
        auto
    } else {
        setting.min(auto).max(1)
    }
}
