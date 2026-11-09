//! Mapping of Khmer-aware clusters onto terminal cells.
//!
//! [`layout`] assigns each cluster a column and width, [`wrap`] breaks text
//! into lines without splitting clusters, and [`fit`] computes how to draw a
//! cluster's shaped glyphs inside its cells.

use std::ops::Range;

use kte_unicode::{CellModel, cluster_width, clusters};

/// A cluster placed on the grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    /// Byte range of the cluster in the source text.
    pub bytes: Range<usize>,
    /// First column the cluster occupies.
    pub col: usize,
    /// Number of cells the cluster occupies (0 for zero-width clusters such as ZWSP).
    pub width: usize,
}

impl Placed {
    /// Columns covered by the cluster.
    pub fn cols(&self) -> Range<usize> {
        self.col..self.col + self.width
    }
}

/// Places every cluster of `text` on a single line starting at column 0.
///
/// ```
/// use kte_cell::layout;
/// use kte_unicode::CellModel;
///
/// let placed = layout("កា ok", CellModel::Wide);
/// let cols: Vec<_> = placed.iter().map(|p| (p.col, p.width)).collect();
/// assert_eq!(cols, [(0, 3), (3, 1), (4, 1), (5, 1)]);
/// ```
pub fn layout(text: &str, model: CellModel) -> Vec<Placed> {
    let mut col = 0;
    clusters(text)
        .map(|(start, c)| {
            let width = cluster_width(c, model);
            let placed = Placed {
                bytes: start..start + c.len(),
                col,
                width,
            };
            col += width;
            placed
        })
        .collect()
}

/// Index of the cluster covering column `col`, if any.
///
/// A column inside a multi-cell cluster maps to that cluster, so cursor and
/// selection positions always snap to whole clusters.
pub fn cluster_at_col(placed: &[Placed], col: usize) -> Option<usize> {
    placed.iter().position(|p| p.cols().contains(&col))
}

/// Breaks `text` into lines of at most `cols` cells, never splitting a
/// cluster. Returns the byte range of each line. A cluster wider than `cols`
/// gets a line of its own.
///
/// Newlines (`\n`) end a line and are not included in any range.
///
/// ```
/// use kte_cell::wrap;
/// use kte_unicode::CellModel;
///
/// // ភា (3) + សា (4) + ខ្មែ (3) + រ (1): a 7-cell line fits ភាសា, not ខ្មែ.
/// let text = "ភាសាខ្មែរ";
/// let lines: Vec<&str> = wrap(text, 7, CellModel::Wide).into_iter().map(|r| &text[r]).collect();
/// assert_eq!(lines, ["ភាសា", "ខ្មែរ"]);
/// ```
pub fn wrap(text: &str, cols: usize, model: CellModel) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut offset = 0;
    for hard_line in text.split('\n') {
        let mut start = offset;
        let mut used = 0;
        for (rel, c) in clusters(hard_line) {
            let at = offset + rel;
            let width = cluster_width(c, model);
            if used > 0 && used + width > cols {
                lines.push(start..at);
                start = at;
                used = 0;
            }
            used += width;
        }
        lines.push(start..offset + hard_line.len());
        offset += hard_line.len() + 1;
    }
    lines
}

/// How to draw a cluster's glyphs inside its cells.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    /// Horizontal scale to apply to the glyphs (1.0 = natural size).
    pub scale_x: f32,
    /// Offset in pixels from the first cell's left edge to the glyphs' origin.
    pub offset_x: f32,
}

/// Fits glyphs with a natural advance of `advance_px` into `cells` cells of
/// `cell_px` pixels each.
///
/// A cluster narrower than its box is centered at natural size; a wider one is
/// compressed horizontally to fit exactly.
///
/// ```
/// use kte_cell::fit;
/// let f = fit(10.0, 2, 8.0); // 10 px of glyphs in a 16 px box
/// assert_eq!((f.scale_x, f.offset_x), (1.0, 3.0));
/// let f = fit(20.0, 2, 8.0); // too wide: compress to 16 px
/// assert_eq!((f.scale_x, f.offset_x), (0.8, 0.0));
/// ```
pub fn fit(advance_px: f32, cells: usize, cell_px: f32) -> Fit {
    let box_px = cells as f32 * cell_px;
    if advance_px <= 0.0 || box_px <= 0.0 {
        return Fit {
            scale_x: 1.0,
            offset_x: 0.0,
        };
    }
    if advance_px <= box_px {
        Fit {
            scale_x: 1.0,
            offset_x: (box_px - advance_px) / 2.0,
        }
    } else {
        Fit {
            scale_x: box_px / advance_px,
            offset_x: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_contiguous_and_lossless() {
        let text = "សួស្តី ពិភពលោក!";
        let placed = layout(text, CellModel::Wide);
        let mut col = 0;
        let mut rebuilt = String::new();
        for p in &placed {
            assert_eq!(p.col, col);
            col += p.width;
            rebuilt.push_str(&text[p.bytes.clone()]);
        }
        assert_eq!(rebuilt, text);
        assert_eq!(col, 21);
    }

    #[test]
    fn columns_snap_to_clusters() {
        let placed = layout("សួស្តី", CellModel::Wide); // សួ 0..3, ស្តី 3..6
        assert_eq!(cluster_at_col(&placed, 0), Some(0));
        assert_eq!(cluster_at_col(&placed, 2), Some(0));
        assert_eq!(cluster_at_col(&placed, 3), Some(1));
        assert_eq!(cluster_at_col(&placed, 6), None);
    }

    #[test]
    fn wrap_handles_newlines_and_oversized_clusters() {
        let text = "ab\nណាaa";
        let lines: Vec<&str> = wrap(text, 3, CellModel::Wide)
            .into_iter()
            .map(|r| &text[r])
            .collect();
        // ណា is 5 cells: wider than the line, so it stands alone.
        assert_eq!(lines, ["ab", "ណា", "aa"]);
    }

    #[test]
    fn wrap_never_exceeds_width_except_for_single_clusters() {
        let text = "មនុស្សទាំងអស់កើតមកមានសេរីភាព និងសមភាព";
        for cols in 4..30 {
            for line in wrap(text, cols, CellModel::Wide) {
                let w = kte_unicode::str_width(&text[line.clone()], CellModel::Wide);
                let n = clusters(&text[line]).count();
                assert!(w <= cols || n == 1, "cols={cols} width={w}");
            }
        }
    }

    #[test]
    fn empty_input() {
        assert!(layout("", CellModel::Wide).is_empty());
        assert_eq!(wrap("", 10, CellModel::Wide), vec![0..0]);
    }
}
