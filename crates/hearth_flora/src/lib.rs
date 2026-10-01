//! The growth model of trees and woody shrubs (v2 §6.2, `docs/design/flora.md`): how tall and
//! thick a species grows with age (`growth`), the branching skeleton of one tree at a stage
//! (`skeleton`), and its blocks as templates grown once and cached (`template`). The world
//! generator places trees from the templates, turned and mirrored per tree; the distant terrain
//! reads their crowns.

pub mod growth;
pub mod skeleton;
pub mod template;

pub use growth::{Species, Stage};
pub use template::{Part, Templates, TreeTemplate, Turn, VARIANTS};
