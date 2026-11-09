//! Cells, rows and the screen grid.

/// A color in the terminal palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Color {
    /// The terminal's default foreground or background.
    #[default]
    Default,
    /// One of the 256 indexed colors (0–15 are the ANSI colors).
    Indexed(u8),
    /// A 24-bit color.
    Rgb(u8, u8, u8),
}

/// Text attributes set by SGR (`CSI … m`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Attrs {
    /// Foreground color.
    pub fg: Color,
    /// Background color.
    pub bg: Color,
    /// Bold (SGR 1).
    pub bold: bool,
    /// Italic (SGR 3).
    pub italic: bool,
    /// Underline (SGR 4).
    pub underline: bool,
    /// Reverse video (SGR 7).
    pub inverse: bool,
}

/// One grid cell.
///
/// A cluster `N` cells wide is stored in its first cell (`width == N`),
/// followed by `N - 1` continuation cells (`width == 0`, empty text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// The cluster's text; empty for blank and continuation cells.
    pub text: String,
    /// Cells covered: ≥ 1 for a cluster start or blank, 0 for a continuation.
    pub width: u8,
    /// Attributes the cell was written with.
    pub attrs: Attrs,
}

impl Cell {
    /// An empty cell with the given attributes (erased cells keep the background).
    pub fn blank(attrs: Attrs) -> Self {
        Cell {
            text: String::new(),
            width: 1,
            attrs,
        }
    }

    pub(crate) fn continuation(attrs: Attrs) -> Self {
        Cell {
            text: String::new(),
            width: 0,
            attrs,
        }
    }

    /// Whether this cell is the tail of a multi-cell cluster.
    pub fn is_continuation(&self) -> bool {
        self.width == 0
    }

    /// Whether this cell holds nothing.
    pub fn is_blank(&self) -> bool {
        self.width == 1 && self.text.is_empty()
    }
}

/// One screen line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The cells, exactly `cols` of them.
    pub cells: Vec<Cell>,
    /// Whether the line continues on the next row because of autowrap
    /// (selection joins wrapped rows without a newline).
    pub wrapped: bool,
}

impl Row {
    pub(crate) fn new(cols: usize, attrs: Attrs) -> Self {
        Row {
            cells: vec![Cell::blank(attrs); cols],
            wrapped: false,
        }
    }

    /// Column where the cluster covering `col` starts.
    pub fn cluster_start(&self, col: usize) -> usize {
        let mut c = col.min(self.cells.len().saturating_sub(1));
        while c > 0 && self.cells[c].is_continuation() {
            c -= 1;
        }
        c
    }

    /// Erases every cluster that overlaps `cols`, including the parts of
    /// those clusters outside the range, so no cluster is left half-drawn.
    pub(crate) fn erase(&mut self, cols: std::ops::Range<usize>, attrs: Attrs) {
        if cols.is_empty() || self.cells.is_empty() {
            return;
        }
        let start = self.cluster_start(cols.start);
        let mut end = cols.end.min(self.cells.len());
        while end < self.cells.len() && self.cells[end].is_continuation() {
            end += 1;
        }
        for cell in &mut self.cells[start..end] {
            *cell = Cell::blank(attrs);
        }
    }

    /// Repairs clusters broken by inserting or deleting cells: orphaned
    /// continuations and clusters missing their continuations become blanks.
    pub(crate) fn repair(&mut self) {
        let mut i = 0;
        while i < self.cells.len() {
            let w = self.cells[i].width as usize;
            if w == 0 {
                self.cells[i] = Cell::blank(self.cells[i].attrs);
                i += 1;
                continue;
            }
            let intact = i + w <= self.cells.len()
                && self.cells[i + 1..i + w].iter().all(Cell::is_continuation);
            if intact {
                i += w;
            } else {
                self.cells[i] = Cell::blank(self.cells[i].attrs);
                i += 1;
            }
        }
    }

    /// The row's text with trailing blanks removed. Blank cells inside the
    /// line read as spaces.
    pub fn text(&self) -> String {
        let mut out = String::new();
        let mut pending_spaces = 0;
        for cell in &self.cells {
            if cell.is_continuation() {
                continue;
            }
            if cell.text.is_empty() {
                pending_spaces += 1;
            } else {
                out.extend(std::iter::repeat_n(' ', pending_spaces));
                pending_spaces = 0;
                out.push_str(&cell.text);
            }
        }
        out
    }
}
