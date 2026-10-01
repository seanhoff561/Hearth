//! The content platform (v2 §3): every piece of game content lives in versioned,
//! schema-validated data files under `data/<namespace>/`, organised by domain. This crate loads
//! them into typed tables, generates form × material items, lints the whole set
//! (cross-references, reachability, habitats, food webs, unit sanity, effort rollup) and exports
//! graphs. Rust code elsewhere implements mechanisms; the data here supplies parameters.

pub mod balance;
pub mod content;
pub mod diag;
pub mod generate;
pub mod graph;
pub mod id;
pub mod lint;
pub mod schema;
pub mod source;
pub mod time;
pub mod triggers;
pub mod validate;

pub use balance::Balance;
pub use content::{Content, Origin, Table};
pub use diag::{Diagnostic, Report, Severity};
pub use id::IdRef;
pub use lint::{LintContext, lint};
pub use time::TimeScales;
