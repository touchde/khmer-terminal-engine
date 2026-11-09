//! Cluster segmentation: the units a terminal stores in cells and the cursor
//! moves over.
//!
//! A KTE cluster is a UAX #29 extended grapheme cluster, except that a cluster
//! ending in COENG is joined with the following cluster when that cluster
//! starts with a Khmer consonant. This keeps a consonant and its subscripts
//! (e.g. ស្ត្រី) together regardless of the Unicode version's grapheme rules.

use unicode_segmentation::{GraphemeIndices, UnicodeSegmentation};

use crate::class::{COENG, CharClass, char_class};

/// Iterator over the KTE clusters of a string, as `(byte offset, cluster)`.
///
/// Created by [`clusters`].
#[derive(Debug, Clone)]
pub struct Clusters<'a> {
    text: &'a str,
    inner: std::iter::Peekable<GraphemeIndices<'a>>,
}

/// Splits `text` into KTE clusters. Concatenating the clusters gives back
/// `text` exactly.
///
/// ```
/// let parts: Vec<&str> = kte_unicode::clusters("ស្ត្រី ok").map(|(_, c)| c).collect();
/// assert_eq!(parts, ["ស្ត្រី", " ", "o", "k"]);
/// ```
pub fn clusters(text: &str) -> Clusters<'_> {
    Clusters {
        text,
        inner: text.grapheme_indices(true).peekable(),
    }
}

impl<'a> Iterator for Clusters<'a> {
    type Item = (usize, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        let (start, first) = self.inner.next()?;
        let mut end = start + first.len();
        while self.text[..end].ends_with(COENG) {
            match self.inner.peek() {
                Some((_, next)) if starts_with_consonant(next) => {
                    end += next.len();
                    self.inner.next();
                }
                _ => break,
            }
        }
        Some((start, &self.text[start..end]))
    }
}

/// Whether `text` is exactly one cell under `model`.
///
/// For [`CellModel::Legacy`](crate::CellModel::Legacy), cells follow
/// `wcwidth()`-based applications (shells, editors): every character with a
/// width above zero starts a new cell, and zero-width characters attach to the
/// cell before them. So a subscript consonant, and on glibc a spacing vowel,
/// get cells of their own. Renderers re-join such cells for shaping (see
/// [`joins_next`]). Other models use KTE clusters.
pub fn is_single_cluster(text: &str, model: crate::CellModel) -> bool {
    match model {
        crate::CellModel::Legacy(flavor) => text
            .chars()
            .skip(1)
            .all(|c| crate::legacy_char_width(c, flavor) == 0),
        _ => clusters(text).nth(1).is_none(),
    }
}

/// Whether a cell holding `text` must be shaped together with the next cell,
/// holding `next`, because they belong to one Khmer syllable: `next` starts
/// with a dependent sign or COENG, or `text` ends with COENG and `next`
/// starts with a consonant (a subscript).
pub fn joins_next(text: &str, next: &str) -> bool {
    let Some(first) = next.chars().next() else {
        return false;
    };
    let dependent = matches!(
        char_class(first),
        CharClass::SpacingSign
            | CharClass::NonSpacingSign
            | CharClass::InherentVowel
            | CharClass::Coeng
    );
    let starts_khmer = text.chars().next().is_some_and(crate::is_khmer);
    starts_khmer && (dependent || (text.ends_with(COENG) && starts_with_consonant(next)))
}

fn starts_with_consonant(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| char_class(c) == CharClass::Consonant)
}

/// Byte offset of the start of the cluster containing byte `offset`.
/// Offsets past the end return `text.len()`.
pub fn cluster_start(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }
    clusters(text)
        .take_while(|(start, _)| *start <= offset)
        .last()
        .map_or(0, |(start, _)| start)
}

/// Byte offset of the start of the cluster after the one containing `offset`
/// (where a cursor moving right lands). Returns `text.len()` at the end.
pub fn next_boundary(text: &str, offset: usize) -> usize {
    clusters(text)
        .map(|(start, c)| start + c.len())
        .find(|end| *end > offset)
        .unwrap_or(text.len())
}

/// Byte offset of the start of the cluster before `offset` (where a cursor
/// moving left lands). Returns 0 at the start.
pub fn prev_boundary(text: &str, offset: usize) -> usize {
    clusters(text)
        .map(|(start, _)| start)
        .take_while(|start| *start < offset)
        .last()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(s: &str) -> Vec<&str> {
        clusters(s).map(|(_, c)| c).collect()
    }

    #[test]
    fn subscripts_stay_with_their_base() {
        assert_eq!(split("ស្ត"), ["ស្ត"]);
        assert_eq!(split("ស្ត្រី"), ["ស្ត្រី"]);
        assert_eq!(split("ក្រុម"), ["ក្រុ", "ម"]);
        assert_eq!(split("សួស្តី"), ["សួ", "ស្តី"]);
    }

    #[test]
    fn vowels_and_signs_attach() {
        assert_eq!(split("កោះ"), ["កោះ"]);
        assert_eq!(split("ភាសាខ្មែរ"), ["ភា", "សា", "ខ្មែ", "រ"]);
    }

    #[test]
    fn trailing_coeng_and_mixed_text() {
        assert_eq!(split("ក្"), ["ក្"]);
        assert_eq!(split("ក្ a"), ["ក្", " ", "a"]);
        assert_eq!(split("Hi ខ្ញុំ!"), ["H", "i", " ", "ខ្ញុំ", "!"]);
    }

    #[test]
    fn concatenation_is_lossless() {
        let s = "ការអភិវឌ្ឍកម្មវិធី\u{200B}ជាភាសាខ្មែរ 123 ✓";
        assert_eq!(split(s).concat(), s);
    }

    #[test]
    fn legacy_cells_split_at_coeng() {
        use crate::{CellModel, LegacyWidths};
        let legacy = CellModel::Legacy(LegacyWidths::Glibc);
        let glibc = CellModel::Legacy(LegacyWidths::Glibc);
        let apple = CellModel::Legacy(LegacyWidths::Apple);
        assert!(is_single_cluster("ស្", legacy));
        assert!(!is_single_cluster("ស្ត", legacy));
        assert!(is_single_cluster("ស្ត", CellModel::Wide));
        // ា is width 1 on glibc (own cell) and 0 on Apple (attaches).
        assert!(!is_single_cluster("កា", glibc));
        assert!(is_single_cluster("កា", apple));
        assert!(joins_next("ស្", "តី"));
        assert!(joins_next("ក", "ា"));
        assert!(!joins_next("ស", "ត"));
        assert!(!joins_next("a", "ា"));
    }

    #[test]
    fn cursor_boundaries() {
        let s = "សួស្តី"; // clusters: សួ (6 bytes), ស្តី (12 bytes)
        assert_eq!(next_boundary(s, 0), 6);
        assert_eq!(next_boundary(s, 6), s.len());
        assert_eq!(prev_boundary(s, s.len()), 6);
        assert_eq!(prev_boundary(s, 6), 0);
        assert_eq!(cluster_start(s, 9), 6);
        assert_eq!(cluster_start(s, 100), s.len());
    }
}
