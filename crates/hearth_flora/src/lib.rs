//! The growth model of trees and woody shrubs (v2 §6.2, `docs/design/flora.md`): how tall and
//! thick a species grows with age (`growth`), the branching skeleton of one tree at a stage
//! (`skeleton`), its blocks as templates grown once and cached (`template`), and its mesh
//! (`mesh`, S5). The world generator places trees from the templates, turned and mirrored per
//! tree; the distant terrain reads their crowns; the renderer draws their meshes.

pub mod growth;
pub mod mesh;
pub mod skeleton;
pub mod template;

pub use growth::{Species, Stage};
pub use template::{Part, Templates, TreeTemplate, Turn, VARIANTS};
