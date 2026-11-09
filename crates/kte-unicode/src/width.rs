//! The KTE width rule (KTE Width Spec §5).
//!
//! Khmer widths are measured in **half-units**: half the advance of a typical
//! Khmer consonant. The tables come from measurements of 160 Unicode Khmer
//! fonts (`docs/research/0001-khmer-advance-measurements.md`). Every value is
//! derived from codepoints only and never from a font.

use unicode_width::UnicodeWidthStr;

use crate::class::{COENG, CharClass, char_class, is_khmer};
use crate::segment::clusters;

/// How Khmer half-units map to terminal cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CellModel {
    /// One cell per half-unit: a typical consonant takes 2 cells. Matches
    /// real font advances closely (the KTE default).
    #[default]
    Wide,
    /// One cell per consonant: half-units rounded up per cluster. More compact,
    /// but syllables are less evenly spaced.
    Compact,
    /// Compatibility with applications that measure text with the system's
    /// per-codepoint `wcwidth()` (today's shells, editors and tmux). Not
    /// derived from half-units; see [`legacy_char_width`].
    Legacy(LegacyWidths),
}

/// Which system `wcwidth()` the [`CellModel::Legacy`] model reproduces. The
/// two differ only for Khmer spacing signs (ា េ ោ ះ …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LegacyWidths {
    /// GNU libc (Linux): spacing signs are 1 cell.
    Glibc,
    /// Apple libc (macOS, iOS): spacing signs are 0 cells.
    Apple,
}

impl LegacyWidths {
    /// The flavor of the platform this code was compiled for.
    pub fn native() -> LegacyWidths {
        if cfg!(target_vendor = "apple") {
            LegacyWidths::Apple
        } else {
            LegacyWidths::Glibc
        }
    }
}

/// Width of one codepoint as the system `wcwidth()` reports it (negative
/// values for control characters become 0).
///
/// Khmer bases are 1, subscript consonants are 1 (the preceding COENG is 0),
/// non-spacing signs are 0, and spacing signs depend on `flavor`.
pub fn legacy_char_width(c: char, flavor: LegacyWidths) -> usize {
    match char_class(c) {
        CharClass::Other => unicode_width::UnicodeWidthChar::width(c).unwrap_or(0),
        CharClass::SpacingSign => match flavor {
            LegacyWidths::Glibc => 1,
            LegacyWidths::Apple => 0,
        },
        CharClass::NonSpacingSign
        | CharClass::InherentVowel
        | CharClass::Coeng
        | CharClass::ZeroWidth => 0,
        _ => 1,
    }
}

/// Half-units of a base character (consonant, independent vowel, digit,
/// punctuation or symbol) on its own.
pub fn base_half_units(c: char) -> u32 {
    match c {
        // Narrow consonants: RO, VO.
        'រ' | 'វ' => 1,
        // Wide consonants and independent vowel QUU.
        'ឃ' | 'ញ' | 'ឍ' | 'យ' | 'ល' | 'ស' | 'ហ' | 'ឡ' | 'ឦ' => 3,
        // Extra-wide consonants: CHO, NNO.
        'ឈ' | 'ណ' => 4,
        // Punctuation: CAMNUC PII KUUH (៖) and the riel sign (៛) are narrow.
        '៖' | '៛' => 1,
        // BEYYAL (៘) is very wide; KOOMUUT (៚) is double.
        '៘' => 6,
        '៚' => 4,
        // Lunar date symbols with two-digit day numbers.
        '\u{19EA}'..='\u{19EF}' | '\u{19FA}'..='\u{19FF}' => 4,
        _ => 2,
    }
}

/// Half-units a spacing sign (Mc) adds to its syllable.
pub fn spacing_sign_half_units(c: char) -> u32 {
    match c {
        // Two-part vowels: a pre-base part plus a post-base part.
        'ឿ' | 'ៀ' | 'ោ' | 'ៅ' => 2,
        _ => 1,
    }
}

/// Half-units a subscript (COENG + `consonant`) adds to its syllable.
pub fn subscript_half_units(consonant: char) -> u32 {
    match consonant {
        // Subscripts drawn partly beside the base: KHO, CHO, DHO, BA, YO, RO, SSO, SA.
        'ឃ' | 'ឈ' | 'ឍ' | 'ប' | 'យ' | 'រ' | 'ឞ' | 'ស' => 1,
        _ => 0,
    }
}

/// Half-units of a Khmer cluster, or `None` if `cluster` does not start with a
/// Khmer character (or a Khmer sign with no base).
///
/// A cluster that starts with a dependent sign (no base) counts as if it had a
/// standard base, since renderers draw a dotted circle there.
///
/// ```
/// use kte_unicode::half_units;
/// assert_eq!(half_units("ក"), Some(2));
/// assert_eq!(half_units("កា"), Some(3));
/// assert_eq!(half_units("កោះ"), Some(5));
/// assert_eq!(half_units("ស្ត្រី"), Some(4)); // ស 3 + ្ត 0 + ្រ 1 + ី 0
/// assert_eq!(half_units("a"), None);
/// ```
pub fn half_units(cluster: &str) -> Option<u32> {
    let first = cluster.chars().next()?;
    if !is_khmer(first) {
        return None;
    }
    let mut total = 0;
    let mut chars = cluster.chars().peekable();
    if !char_class(first).is_base() {
        total += 2;
    }
    while let Some(c) = chars.next() {
        total += match char_class(c) {
            class if class.is_base() => base_half_units(c),
            CharClass::SpacingSign => spacing_sign_half_units(c),
            CharClass::Coeng => match chars.peek() {
                Some(&next) if char_class(next) == CharClass::Consonant => {
                    chars.next();
                    subscript_half_units(next)
                }
                _ => 0,
            },
            CharClass::Other => {
                UnicodeWidthStr::width(c.encode_utf8(&mut [0; 4]) as &str) as u32 * 2
            }
            _ => 0,
        };
    }
    Some(total)
}

/// Converts half-units to cells under `model`.
///
/// [`CellModel::Legacy`] widths are not derived from half-units; for it this
/// returns the Compact value as an approximation. Use [`cluster_width`] for
/// exact widths.
pub fn cells_for_half_units(half_units: u32, model: CellModel) -> usize {
    match model {
        CellModel::Wide => half_units as usize,
        CellModel::Compact | CellModel::Legacy(_) => half_units.div_ceil(2) as usize,
    }
}

/// Width in cells of one cluster (as produced by [`clusters`](crate::clusters)).
///
/// Khmer clusters follow the KTE rule. Other clusters keep their standard
/// terminal width (UAX #11 via `unicode-width`): Latin 1, CJK 2, and so on.
pub fn cluster_width(cluster: &str, model: CellModel) -> usize {
    if let CellModel::Legacy(flavor) = model {
        return cluster.chars().map(|c| legacy_char_width(c, flavor)).sum();
    }
    match half_units(cluster) {
        Some(h) => cells_for_half_units(h, model),
        None => UnicodeWidthStr::width(cluster),
    }
}

/// Width in cells of a string: the sum of its clusters' widths.
///
/// ```
/// use kte_unicode::{CellModel, str_width};
/// assert_eq!(str_width("ok ភាសា", CellModel::Wide), 3 + 7); // ភា 3 + សា 4
/// assert_eq!(str_width("ok ភាសា", CellModel::Compact), 3 + 4);
/// ```
pub fn str_width(text: &str, model: CellModel) -> usize {
    clusters(text).map(|(_, c)| cluster_width(c, model)).sum()
}

/// Whether `c` never occupies a cell on its own.
pub fn is_zero_width(c: char) -> bool {
    matches!(
        char_class(c),
        CharClass::NonSpacingSign | CharClass::InherentVowel | CharClass::ZeroWidth
    ) || c == COENG
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consonants() {
        assert_eq!(half_units("ក"), Some(2));
        assert_eq!(half_units("រ"), Some(1));
        assert_eq!(half_units("ស"), Some(3));
        assert_eq!(half_units("ណ"), Some(4));
    }

    #[test]
    fn signs() {
        assert_eq!(half_units("កិ"), Some(2)); // Mn adds nothing
        assert_eq!(half_units("កេ"), Some(3));
        assert_eq!(half_units("កៅ"), Some(4));
        assert_eq!(half_units("ក់"), Some(2));
    }

    #[test]
    fn subscripts() {
        assert_eq!(half_units("ក្ក"), Some(2));
        assert_eq!(half_units("ក្រ"), Some(3));
        assert_eq!(half_units("ក្ស"), Some(3));
        assert_eq!(half_units("ក្"), Some(2)); // dangling coeng
    }

    #[test]
    fn base_less_cluster_counts_a_placeholder() {
        assert_eq!(half_units("\u{17B6}"), Some(3)); // ◌ា
        assert_eq!(half_units("\u{17B7}"), Some(2)); // ◌ិ
    }

    #[test]
    fn models() {
        assert_eq!(cluster_width("កា", CellModel::Wide), 3);
        assert_eq!(cluster_width("កា", CellModel::Compact), 2);
        assert_eq!(cluster_width("ក", CellModel::Compact), 1);
        assert_eq!(cluster_width("a", CellModel::Wide), 1);
        assert_eq!(cluster_width("中", CellModel::Wide), 2);
    }

    #[test]
    fn legacy_matches_system_wcwidth() {
        let glibc = CellModel::Legacy(LegacyWidths::Glibc);
        let apple = CellModel::Legacy(LegacyWidths::Apple);
        // ស 1 + ្ 0 + ត 1 + ី 0
        assert_eq!(cluster_width("ស្តី", glibc), 2);
        // ក 1 + ោ (Mc): 1 on glibc, 0 on Apple
        assert_eq!(cluster_width("កោ", glibc), 2);
        assert_eq!(cluster_width("កោ", apple), 1);
        assert_eq!(str_width("ok 中", glibc), 5);
    }

    #[test]
    fn zero_width_space_is_free() {
        assert_eq!(str_width("ក\u{200B}ក", CellModel::Wide), 4);
    }

    #[test]
    fn digits_are_uniform() {
        for d in '០'..='៩' {
            assert_eq!(base_half_units(d), 2, "{d}");
        }
    }
}
