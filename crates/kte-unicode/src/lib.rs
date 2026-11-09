//! Khmer Unicode classification, cluster segmentation, and the KTE cell-width rule.
//!
//! This crate implements the [KTE Width Spec]. Widths are computed from
//! codepoints only and never depend on fonts, so applications and terminals
//! can agree on them without exchanging font information.
//!
//! ```
//! use kte_unicode::{CellModel, clusters, cluster_width};
//!
//! for (_, cluster) in clusters("សួស្តី") {
//!     println!("{cluster}: {} cells", cluster_width(cluster, CellModel::Wide));
//! }
//! ```
//!
//! [KTE Width Spec]: https://github.com/touchde/khmer-terminal-engine/blob/main/spec/kte-width-spec.md

mod class;
mod segment;
mod width;

pub use class::{COENG, CharClass, char_class, is_khmer};
pub use segment::{Clusters, cluster_start, clusters, next_boundary, prev_boundary};
pub use width::{
    CellModel, base_half_units, cells_for_half_units, cluster_width, half_units, is_zero_width,
    spacing_sign_half_units, str_width, subscript_half_units,
};

/// Version of the KTE Width Spec this crate implements.
pub const SPEC_VERSION: &str = "0.1.0";
