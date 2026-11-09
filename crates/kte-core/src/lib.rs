//! Stable public API of Khmer Terminal Engine.
//!
//! Most users only need this crate:
//!
//! ```
//! use kte_core::{CellModel, Terminal, str_width};
//!
//! assert_eq!(str_width("ភាសាខ្មែរ", CellModel::Wide), 11);
//!
//! let mut term = Terminal::new(24, 80, CellModel::Wide);
//! term.feed("សួស្តី ពិភពលោក!".as_bytes());
//! assert_eq!(term.cursor(), (0, 21));
//! ```
//!
//! The component crates are re-exported as modules for advanced use.

pub use kte_cell as cell;
pub use kte_shaper as shaper;
pub use kte_terminal as terminal;
pub use kte_unicode as unicode;

pub use kte_shaper::Shaper;
pub use kte_terminal::Terminal;
pub use kte_unicode::{CellModel, SPEC_VERSION, cluster_width, clusters, half_units, str_width};
