//! Terminal model with Khmer-aware clusters: VT/ANSI parsing, the cell grid,
//! cursor, scrolling, wrapping and selection.
//!
//! The terminal stores whole clusters (per the KTE Width Spec) in cells. A
//! Khmer syllable that arrives one codepoint at a time is extended in place,
//! and its width grows as spacing vowels and subscripts arrive.
//!
//! ```
//! use kte_terminal::Terminal;
//! use kte_unicode::CellModel;
//!
//! let mut term = Terminal::new(4, 20, CellModel::Wide);
//! term.feed("សួស្តី\r\n\x1b[1mok".as_bytes());
//! assert_eq!(term.row_text(0), "សួស្តី");
//! assert_eq!(term.cursor(), (1, 2));
//! assert!(term.cell(1, 0).attrs.bold);
//! ```

mod grid;

use std::mem;

use kte_unicode::{CellModel, cluster_width, clusters};

pub use grid::{Attrs, Cell, Color, Row};

/// DEC private mode 2027: grapheme-cluster width measurement.
pub const MODE_GRAPHEME_CLUSTERS: u16 = 2027;

/// A terminal screen fed by a byte stream.
pub struct Terminal {
    state: State,
    parser: vte::Parser,
}

impl std::fmt::Debug for Terminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminal")
            .field("rows", &self.state.rows)
            .field("cols", &self.state.cols)
            .field("cursor", &(self.state.row, self.state.col))
            .field("model", &self.state.model)
            .finish_non_exhaustive()
    }
}

struct State {
    rows: usize,
    cols: usize,
    model: CellModel,
    grid: Vec<Row>,
    alt_grid: Option<Vec<Row>>,
    scrollback: Vec<Row>,
    scrollback_limit: usize,
    row: usize,
    col: usize,
    pending_wrap: bool,
    saved: Option<(usize, usize, Attrs)>,
    attrs: Attrs,
    scroll_top: usize,
    scroll_bottom: usize,
    autowrap: bool,
    cursor_visible: bool,
    /// Start of the last printed cluster, which later codepoints may extend.
    last: Option<(usize, usize)>,
    title: String,
    responses: Vec<u8>,
}

impl Terminal {
    /// Creates a blank terminal of `rows` × `cols` cells (each at least 1).
    pub fn new(rows: usize, cols: usize, model: CellModel) -> Self {
        let (rows, cols) = (rows.max(1), cols.max(1));
        Terminal {
            state: State {
                rows,
                cols,
                model,
                grid: (0..rows)
                    .map(|_| Row::new(cols, Attrs::default()))
                    .collect(),
                alt_grid: None,
                scrollback: Vec::new(),
                scrollback_limit: 10_000,
                row: 0,
                col: 0,
                pending_wrap: false,
                saved: None,
                attrs: Attrs::default(),
                scroll_top: 0,
                scroll_bottom: rows - 1,
                autowrap: true,
                cursor_visible: true,
                last: None,
                title: String::new(),
                responses: Vec::new(),
            },
            parser: vte::Parser::new(),
        }
    }

    /// Processes output from the application (the PTY's output side).
    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.state, bytes);
    }

    /// Bytes the terminal must send back to the application (e.g. replies to
    /// cursor-position and mode queries). Write them to the PTY's input side.
    pub fn take_responses(&mut self) -> Vec<u8> {
        mem::take(&mut self.state.responses)
    }

    /// Like [`take_responses`](Self::take_responses), but takes at most
    /// `max` bytes; the rest stays queued.
    pub fn take_responses_max(&mut self, max: usize) -> Vec<u8> {
        let n = max.min(self.state.responses.len());
        self.state.responses.drain(..n).collect()
    }

    /// Number of rows.
    pub fn rows(&self) -> usize {
        self.state.rows
    }

    /// Number of columns.
    pub fn cols(&self) -> usize {
        self.state.cols
    }

    /// The cell model used for Khmer widths.
    pub fn model(&self) -> CellModel {
        self.state.model
    }

    /// Cursor position as `(row, col)`, zero-based.
    pub fn cursor(&self) -> (usize, usize) {
        (self.state.row, self.state.col)
    }

    /// Whether the cursor should be drawn (DECTCEM).
    pub fn cursor_visible(&self) -> bool {
        self.state.cursor_visible
    }

    /// Window title set with OSC 0 or OSC 2.
    pub fn title(&self) -> &str {
        &self.state.title
    }

    /// Whether the alternate screen (used by full-screen apps) is active.
    pub fn alt_screen(&self) -> bool {
        self.state.alt_grid.is_some()
    }

    /// The cell at `(row, col)`. Panics if out of range.
    pub fn cell(&self, row: usize, col: usize) -> &Cell {
        &self.state.grid[row].cells[col]
    }

    /// The row at `row` on screen. Panics if out of range.
    pub fn row(&self, row: usize) -> &Row {
        &self.state.grid[row]
    }

    /// Rows that scrolled off the top of the main screen, oldest first.
    pub fn scrollback(&self) -> &[Row] {
        &self.state.scrollback
    }

    /// Text of a screen row, without trailing blanks.
    pub fn row_text(&self, row: usize) -> String {
        self.state.grid[row].text()
    }

    /// Text of the whole screen, one line per row, without trailing blank lines.
    pub fn screen_text(&self) -> String {
        let mut lines: Vec<String> = (0..self.state.rows).map(|r| self.row_text(r)).collect();
        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        lines.join("\n")
    }

    /// Columns covered by the cluster at `(row, col)`. A column inside a
    /// multi-cell cluster returns the whole cluster's span.
    pub fn cluster_span(&self, row: usize, col: usize) -> std::ops::Range<usize> {
        let r = &self.state.grid[row];
        let start = r.cluster_start(col);
        start..start + (r.cells[start].width as usize).max(1)
    }

    /// Text between two positions (inclusive, in reading order), snapped
    /// outward to whole clusters. Rows are joined with `\n` unless the row was
    /// soft-wrapped. The original codepoints are returned unchanged.
    pub fn selection_text(&self, from: (usize, usize), to: (usize, usize)) -> String {
        let (a, b) = if from <= to { (from, to) } else { (to, from) };
        let mut out = String::new();
        for row in a.0..=b.0.min(self.state.rows - 1) {
            let r = &self.state.grid[row];
            let first = if row == a.0 { r.cluster_start(a.1) } else { 0 };
            let last = if row == b.0 {
                self.cluster_span(row, b.1.min(self.state.cols - 1)).end
            } else {
                self.state.cols
            };
            let mut line = Row {
                cells: r.cells[first..last].to_vec(),
                wrapped: false,
            };
            line.repair_leading_continuations();
            out.push_str(&line.text());
            if row != b.0 && !r.wrapped {
                out.push('\n');
            }
        }
        out
    }

    /// Changes the screen size. Content is kept where it fits (no reflow);
    /// clusters cut by the new right edge are erased.
    pub fn resize(&mut self, rows: usize, cols: usize) {
        let s = &mut self.state;
        let (rows, cols) = (rows.max(1), cols.max(1));
        let on_alt = s.alt_grid.is_some();
        for (is_main_screen, grid) in [(!on_alt, &mut s.grid)]
            .into_iter()
            .chain(s.alt_grid.as_mut().map(|g| (true, g)))
        {
            while grid.len() > rows {
                let row = grid.remove(0);
                if is_main_screen {
                    s.scrollback.push(row);
                }
            }
            while grid.len() < rows {
                grid.push(Row::new(cols, Attrs::default()));
            }
            for row in grid.iter_mut() {
                row.cells.resize(cols, Cell::blank(Attrs::default()));
                row.repair();
            }
        }
        s.rows = rows;
        s.cols = cols;
        s.row = s.row.min(rows - 1);
        s.col = s.col.min(cols - 1);
        s.scroll_top = 0;
        s.scroll_bottom = rows - 1;
        s.pending_wrap = false;
        s.last = None;
    }
}

impl Row {
    fn repair_leading_continuations(&mut self) {
        for cell in &mut self.cells {
            if !cell.is_continuation() {
                break;
            }
            *cell = Cell::blank(cell.attrs);
        }
    }
}

impl State {
    fn current_row(&mut self) -> &mut Row {
        &mut self.grid[self.row]
    }

    fn blank_row(&self) -> Row {
        let mut attrs = Attrs::default();
        attrs.bg = self.attrs.bg;
        Row::new(self.cols, attrs)
    }

    fn erase_attrs(&self) -> Attrs {
        Attrs {
            bg: self.attrs.bg,
            ..Attrs::default()
        }
    }

    fn scroll_up(&mut self, n: usize) {
        for _ in 0..n {
            let row = self.grid.remove(self.scroll_top);
            if self.scroll_top == 0 && self.alt_grid.is_none() {
                self.scrollback.push(row);
                if self.scrollback.len() > self.scrollback_limit {
                    self.scrollback.remove(0);
                }
            }
            let blank = self.blank_row();
            self.grid.insert(self.scroll_bottom, blank);
        }
        self.last = None;
    }

    fn scroll_down(&mut self, n: usize) {
        for _ in 0..n {
            self.grid.remove(self.scroll_bottom);
            let blank = self.blank_row();
            self.grid.insert(self.scroll_top, blank);
        }
        self.last = None;
    }

    fn linefeed(&mut self) {
        if self.row == self.scroll_bottom {
            self.scroll_up(1);
        } else if self.row + 1 < self.rows {
            self.row += 1;
        }
        self.pending_wrap = false;
    }

    fn reverse_index(&mut self) {
        if self.row == self.scroll_top {
            self.scroll_down(1);
        } else {
            self.row = self.row.saturating_sub(1);
        }
        self.pending_wrap = false;
    }

    fn goto(&mut self, row: usize, col: usize) {
        self.row = row.min(self.rows - 1);
        self.col = col.min(self.cols - 1);
        self.pending_wrap = false;
    }

    /// Whether the cursor sits just after the cluster that starts at `(row, col)`.
    fn cursor_follows(&self, row: usize, col: usize) -> bool {
        let Some(cell) = self.grid.get(row).and_then(|r| r.cells.get(col)) else {
            return false;
        };
        if cell.width == 0 || cell.text.is_empty() || row != self.row {
            return false;
        }
        let end = col + cell.width as usize;
        (end == self.col && !self.pending_wrap) || (self.pending_wrap && end == self.cols)
    }

    fn print_char(&mut self, c: char) {
        if let Some((row, col)) = self.last {
            if self.cursor_follows(row, col) {
                let mut combined = self.grid[row].cells[col].text.clone();
                combined.push(c);
                if clusters(&combined).nth(1).is_none() {
                    self.replace_last(row, col, combined);
                    return;
                }
            }
        }
        let mut buf = [0; 4];
        let text: &str = c.encode_utf8(&mut buf);
        let width = cluster_width(text, self.model);
        if width == 0 {
            // A zero-width character that starts no cluster (e.g. ZWSP): keep
            // it with the previous cluster so copying preserves it.
            if let Some((row, col)) = self.last.filter(|&(r, c)| self.cursor_follows(r, c)) {
                self.grid[row].cells[col].text.push(c);
            }
            return;
        }
        self.put_cluster(text.to_owned(), width);
    }

    /// Re-places the last cluster after a codepoint extended it.
    fn replace_last(&mut self, row: usize, col: usize, text: String) {
        let width = cluster_width(&text, self.model);
        let old = self.grid[row].cells[col].width as usize;
        if width == old {
            self.grid[row].cells[col].text = text;
            return;
        }
        let attrs = self.grid[row].cells[col].attrs;
        self.grid[row].erase(col..col + old, attrs);
        self.row = row;
        self.col = col;
        self.pending_wrap = false;
        let saved = mem::replace(&mut self.attrs, attrs);
        self.put_cluster(text, width);
        self.attrs = saved;
    }

    fn put_cluster(&mut self, text: String, width: usize) {
        let width = width.min(self.cols);
        if self.pending_wrap {
            if self.autowrap {
                self.current_row().wrapped = true;
                self.linefeed();
                self.col = 0;
            }
            self.pending_wrap = false;
        }
        if self.col + width > self.cols {
            if self.autowrap {
                let (row, col, cols) = (self.row, self.col, self.cols);
                let attrs = self.erase_attrs();
                self.grid[row].erase(col..cols, attrs);
                self.current_row().wrapped = true;
                self.linefeed();
                self.col = 0;
            } else {
                self.col = self.cols - width;
            }
        }
        let (row, col, attrs) = (self.row, self.col, self.attrs);
        let r = &mut self.grid[row];
        r.erase(col..col + width, attrs);
        r.cells[col] = Cell {
            text,
            width: width as u8,
            attrs,
        };
        for cell in &mut r.cells[col + 1..col + width] {
            *cell = Cell::continuation(attrs);
        }
        self.last = Some((row, col));
        self.col += width;
        if self.col >= self.cols {
            self.col = self.cols - 1;
            self.pending_wrap = true;
        }
    }

    fn erase_in_display(&mut self, mode: u16) {
        let attrs = self.erase_attrs();
        let (row, col, cols) = (self.row, self.col, self.cols);
        match mode {
            0 => {
                self.grid[row].erase(col..cols, attrs);
                for r in &mut self.grid[row + 1..] {
                    *r = Row::new(cols, attrs);
                }
            }
            1 => {
                for r in &mut self.grid[..row] {
                    *r = Row::new(cols, attrs);
                }
                self.grid[row].erase(0..col + 1, attrs);
            }
            2 | 3 => {
                for r in &mut self.grid {
                    *r = Row::new(cols, attrs);
                }
                if mode == 3 {
                    self.scrollback.clear();
                }
            }
            _ => {}
        }
        self.last = None;
    }

    fn erase_in_line(&mut self, mode: u16) {
        let attrs = self.erase_attrs();
        let (row, col, cols) = (self.row, self.col, self.cols);
        let range = match mode {
            0 => col..cols,
            1 => 0..col + 1,
            2 => 0..cols,
            _ => return,
        };
        self.grid[row].erase(range, attrs);
        self.last = None;
    }

    fn insert_cells(&mut self, n: usize) {
        let (row, col, cols) = (self.row, self.col, self.cols);
        let attrs = self.erase_attrs();
        let cells = &mut self.grid[row].cells;
        let n = n.min(cols - col);
        cells.truncate(cols - n);
        for _ in 0..n {
            cells.insert(col, Cell::blank(attrs));
        }
        self.grid[row].repair();
        self.last = None;
    }

    fn delete_cells(&mut self, n: usize) {
        let (row, col, cols) = (self.row, self.col, self.cols);
        let attrs = self.erase_attrs();
        let cells = &mut self.grid[row].cells;
        let n = n.min(cols - col);
        cells.drain(col..col + n);
        cells.resize(cols, Cell::blank(attrs));
        self.grid[row].repair();
        self.last = None;
    }

    fn insert_lines(&mut self, n: usize) {
        if self.row < self.scroll_top || self.row > self.scroll_bottom {
            return;
        }
        for _ in 0..n.min(self.scroll_bottom - self.row + 1) {
            self.grid.remove(self.scroll_bottom);
            let blank = self.blank_row();
            self.grid.insert(self.row, blank);
        }
        self.col = 0;
        self.last = None;
    }

    fn delete_lines(&mut self, n: usize) {
        if self.row < self.scroll_top || self.row > self.scroll_bottom {
            return;
        }
        for _ in 0..n.min(self.scroll_bottom - self.row + 1) {
            self.grid.remove(self.row);
            let blank = self.blank_row();
            self.grid.insert(self.scroll_bottom, blank);
        }
        self.col = 0;
        self.last = None;
    }

    fn set_alt_screen(&mut self, on: bool) {
        match (on, self.alt_grid.is_some()) {
            (true, false) => {
                self.saved = Some((self.row, self.col, self.attrs));
                let alt = (0..self.rows)
                    .map(|_| Row::new(self.cols, Attrs::default()))
                    .collect();
                self.alt_grid = Some(mem::replace(&mut self.grid, alt));
            }
            (false, true) => {
                if let Some(main) = self.alt_grid.take() {
                    self.grid = main;
                }
                if let Some((row, col, attrs)) = self.saved {
                    self.goto(row, col);
                    self.attrs = attrs;
                }
            }
            _ => {}
        }
        self.last = None;
    }

    fn set_private_mode(&mut self, mode: u16, on: bool) {
        match mode {
            7 => self.autowrap = on,
            25 => self.cursor_visible = on,
            47 | 1047 | 1049 => self.set_alt_screen(on),
            // Mode 2027 is always on: KTE always measures whole clusters.
            _ => {}
        }
    }

    fn private_mode_state(&self, mode: u16) -> u8 {
        // DECRPM values: 1 = set, 2 = reset, 3 = permanently set, 0 = unknown.
        match mode {
            7 => {
                if self.autowrap {
                    1
                } else {
                    2
                }
            }
            25 => {
                if self.cursor_visible {
                    1
                } else {
                    2
                }
            }
            1049 => {
                if self.alt_grid.is_some() {
                    1
                } else {
                    2
                }
            }
            MODE_GRAPHEME_CLUSTERS => 3,
            _ => 0,
        }
    }

    fn sgr(&mut self, params: &vte::Params) {
        let mut it = params.iter().map(|p| p.first().copied().unwrap_or(0));
        let mut any = false;
        while let Some(p) = it.next() {
            any = true;
            let a = &mut self.attrs;
            match p {
                0 => *a = Attrs::default(),
                1 => a.bold = true,
                3 => a.italic = true,
                4 => a.underline = true,
                7 => a.inverse = true,
                22 => a.bold = false,
                23 => a.italic = false,
                24 => a.underline = false,
                27 => a.inverse = false,
                30..=37 => a.fg = Color::Indexed((p - 30) as u8),
                39 => a.fg = Color::Default,
                40..=47 => a.bg = Color::Indexed((p - 40) as u8),
                49 => a.bg = Color::Default,
                90..=97 => a.fg = Color::Indexed((p - 90 + 8) as u8),
                100..=107 => a.bg = Color::Indexed((p - 100 + 8) as u8),
                38 | 48 => {
                    let color = match it.next() {
                        Some(5) => it.next().map(|n| Color::Indexed(n as u8)),
                        Some(2) => match (it.next(), it.next(), it.next()) {
                            (Some(r), Some(g), Some(b)) => {
                                Some(Color::Rgb(r as u8, g as u8, b as u8))
                            }
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(color) = color {
                        if p == 38 {
                            self.attrs.fg = color;
                        } else {
                            self.attrs.bg = color;
                        }
                    }
                }
                _ => {}
            }
        }
        if !any {
            self.attrs = Attrs::default();
        }
    }
}

/// The `i`-th parameter, or `default` when absent or zero.
fn param(params: &vte::Params, i: usize, default: u16) -> u16 {
    match params.iter().nth(i).and_then(|p| p.first().copied()) {
        None | Some(0) => default,
        Some(v) => v,
    }
}

impl vte::Perform for State {
    fn print(&mut self, c: char) {
        self.print_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x08 => {
                self.col = self.col.saturating_sub(1);
                self.pending_wrap = false;
            }
            0x09 => {
                self.col = ((self.col / 8 + 1) * 8).min(self.cols - 1);
                self.pending_wrap = false;
            }
            0x0A..=0x0C => self.linefeed(),
            0x0D => {
                self.col = 0;
                self.pending_wrap = false;
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        ignore: bool,
        action: char,
    ) {
        if ignore {
            return;
        }
        let p = |i, d| param(params, i, d) as usize;
        let private = intermediates.first() == Some(&b'?');
        match (action, private) {
            ('A', false) => {
                let top = if self.row >= self.scroll_top {
                    self.scroll_top
                } else {
                    0
                };
                self.goto(self.row.saturating_sub(p(0, 1)).max(top), self.col);
            }
            ('B', false) | ('e', false) => {
                let bottom = if self.row <= self.scroll_bottom {
                    self.scroll_bottom
                } else {
                    self.rows - 1
                };
                self.goto((self.row + p(0, 1)).min(bottom), self.col);
            }
            ('C', false) | ('a', false) => self.goto(self.row, self.col + p(0, 1)),
            ('D', false) => self.goto(self.row, self.col.saturating_sub(p(0, 1))),
            ('E', false) => self.goto(self.row + p(0, 1), 0),
            ('F', false) => self.goto(self.row.saturating_sub(p(0, 1)), 0),
            ('G', false) | ('`', false) => self.goto(self.row, p(0, 1) - 1),
            ('d', false) => self.goto(p(0, 1) - 1, self.col),
            ('H', false) | ('f', false) => self.goto(p(0, 1) - 1, p(1, 1) - 1),
            ('J', _) => self.erase_in_display(param(params, 0, 0)),
            ('K', _) => self.erase_in_line(param(params, 0, 0)),
            ('@', false) => self.insert_cells(p(0, 1)),
            ('P', false) => self.delete_cells(p(0, 1)),
            ('X', false) => {
                let (row, col) = (self.row, self.col);
                let end = (col + p(0, 1)).min(self.cols);
                let attrs = self.erase_attrs();
                self.grid[row].erase(col..end, attrs);
                self.last = None;
            }
            ('L', false) => self.insert_lines(p(0, 1)),
            ('M', false) => self.delete_lines(p(0, 1)),
            ('S', false) => self.scroll_up(p(0, 1)),
            ('T', false) => self.scroll_down(p(0, 1)),
            ('m', false) => self.sgr(params),
            ('r', false) => {
                let top = p(0, 1) - 1;
                let bottom = (p(1, self.rows.min(u16::MAX as usize) as u16) - 1).min(self.rows - 1);
                if top < bottom {
                    self.scroll_top = top;
                    self.scroll_bottom = bottom;
                    self.goto(0, 0);
                }
            }
            ('s', false) => self.saved = Some((self.row, self.col, self.attrs)),
            ('u', false) => {
                if let Some((row, col, attrs)) = self.saved {
                    self.goto(row, col);
                    self.attrs = attrs;
                }
            }
            ('n', false) => match param(params, 0, 0) {
                5 => self.responses.extend_from_slice(b"\x1b[0n"),
                6 => {
                    let reply = format!("\x1b[{};{}R", self.row + 1, self.col + 1);
                    self.responses.extend_from_slice(reply.as_bytes());
                }
                _ => {}
            },
            ('h', true) | ('l', true) => {
                for mode in params.iter().filter_map(|p| p.first().copied()) {
                    self.set_private_mode(mode, action == 'h');
                }
            }
            ('p', true) if intermediates.get(1) == Some(&b'$') => {
                let mode = param(params, 0, 0);
                let reply = format!("\x1b[?{mode};{}$y", self.private_mode_state(mode));
                self.responses.extend_from_slice(reply.as_bytes());
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        if !intermediates.is_empty() {
            return;
        }
        match byte {
            b'7' => self.saved = Some((self.row, self.col, self.attrs)),
            b'8' => {
                if let Some((row, col, attrs)) = self.saved {
                    self.goto(row, col);
                    self.attrs = attrs;
                }
            }
            b'D' => self.linefeed(),
            b'E' => {
                self.linefeed();
                self.col = 0;
            }
            b'M' => self.reverse_index(),
            b'c' => {
                let (rows, cols, model) = (self.rows, self.cols, self.model);
                let fresh = Terminal::new(rows, cols, model).state;
                let responses = mem::take(&mut self.responses);
                *self = fresh;
                self.responses = responses;
            }
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if let [b"0" | b"2", title, ..] = params {
            self.title = String::from_utf8_lossy(title).into_owned();
        }
    }
}

#[cfg(test)]
mod tests;
