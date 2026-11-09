//! CPU renderer: draws a `Terminal` into a 0RGB pixel buffer.
//!
//! Two passes per frame: all backgrounds first, then all glyphs. Khmer
//! subscripts and above-vowels may extend into neighboring rows; drawing
//! glyphs last keeps the next row's background from covering them
//! (KTE Width Spec §7: never clip glyphs vertically).

use std::collections::HashMap;
use std::rc::Rc;

use kte_core::cell::fit_with_stretch;
use kte_core::terminal::{Attrs, Color};
use kte_core::{CellModel, Shaper, Terminal};

use crate::fonts::{Face, GlyphCache, GlyphMask, ShapedGlyph};

const LATIN: u8 = 0;
const KHMER: u8 = 1;

/// Pixel buffer in softbuffer's format: one `0x00RRGGBB` word per pixel.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u32>,
    /// While drawing glyphs: a rectangle `(x0, y0, x1, y1)` (the block
    /// cursor) inside which glyph pixels use the second color instead, so
    /// text and marks over the cursor stay visible.
    inverse: Option<((i32, i32, i32, i32), u32)>,
}

impl Frame {
    pub fn new(width: u32, height: u32) -> Frame {
        Frame {
            width,
            height,
            pixels: vec![0; (width * height) as usize],
            inverse: None,
        }
    }

    fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        let (x0, y0) = (x.max(0), y.max(0));
        let x1 = (x + w).min(self.width as i32);
        let y1 = (y + h).min(self.height as i32);
        for yy in y0..y1 {
            let row = (yy as u32 * self.width) as usize;
            self.pixels[row + x0 as usize..row + x1.max(x0) as usize].fill(color);
        }
    }

    fn blend_mask(&mut self, mask: &GlyphMask, x: i32, y: i32, color: u32) {
        for my in 0..mask.height as i32 {
            let py = y + mask.top + my;
            if py < 0 || py >= self.height as i32 {
                continue;
            }
            for mx in 0..mask.width as i32 {
                let px = x + mask.left + mx;
                if px < 0 || px >= self.width as i32 {
                    continue;
                }
                let a = u32::from(mask.alpha[(my * mask.width as i32 + mx) as usize]);
                if a == 0 {
                    continue;
                }
                let i = (py as u32 * self.width + px as u32) as usize;
                let color = match self.inverse {
                    Some(((x0, y0, x1, y1), alt)) if px >= x0 && px < x1 && py >= y0 && py < y1 => {
                        alt
                    }
                    _ => color,
                };
                self.pixels[i] = blend(self.pixels[i], color, a);
            }
        }
    }
}

fn blend(dst: u32, src: u32, a: u32) -> u32 {
    let ch = |shift: u32| {
        let d = (dst >> shift) & 0xff;
        let s = (src >> shift) & 0xff;
        ((s * a + d * (255 - a)) / 255) << shift
    };
    ch(16) | ch(8) | ch(0)
}

/// Colors. Defaults are a dark theme; indices 0–15 are the ANSI colors.
pub struct Theme {
    pub background: u32,
    pub foreground: u32,
    pub cursor: u32,
    pub selection: u32,
    pub ansi: [u32; 16],
}

impl Default for Theme {
    fn default() -> Theme {
        Theme {
            background: 0x1e1e2e,
            foreground: 0xcdd6f4,
            cursor: 0xf5e0dc,
            selection: 0x45475a,
            ansi: [
                0x45475a, 0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xbac2de,
                0x585b70, 0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xa6adc8,
            ],
        }
    }
}

impl Theme {
    fn color(&self, c: Color, default: u32) -> u32 {
        match c {
            Color::Default => default,
            Color::Indexed(i) if i < 16 => self.ansi[i as usize],
            Color::Indexed(i) if i < 232 => {
                // 6×6×6 color cube.
                let i = i - 16;
                let level = |v: u8| if v == 0 { 0 } else { 55 + u32::from(v) * 40 };
                (level(i / 36) << 16) | (level((i / 6) % 6) << 8) | level(i % 6)
            }
            Color::Indexed(i) => {
                let g = 8 + u32::from(i - 232) * 10;
                (g << 16) | (g << 8) | g
            }
            Color::Rgb(r, g, b) => (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b),
        }
    }

    /// Foreground and background for a cell, applying bold-brightening and
    /// reverse video.
    fn cell_colors(&self, a: &Attrs) -> (u32, u32) {
        let fg_color = match a.fg {
            Color::Indexed(i) if a.bold && i < 8 => Color::Indexed(i + 8),
            c => c,
        };
        let fg = self.color(fg_color, self.foreground);
        let bg = self.color(a.bg, self.background);
        if a.inverse { (bg, fg) } else { (fg, bg) }
    }
}

/// What to draw besides the grid itself.
#[derive(Default, Clone, Copy)]
pub struct Overlay {
    /// Selected range in reading order, inclusive.
    pub selection: Option<((usize, usize), (usize, usize))>,
    /// Whether the window has keyboard focus (solid vs. hollow cursor).
    pub focused: bool,
}

/// A cluster shaped once and reused across frames.
struct Shaped {
    face: u8,
    glyphs: Vec<ShapedGlyph>,
    /// Total advance in pixels at the face's render size.
    advance_px: f32,
}

pub struct Renderer {
    latin: Face,
    khmer: Shaper,
    khmer_face: Face,
    pub theme: Theme,
    pub model: CellModel,
    pub padding: u32,
    latin_px: f32,
    khmer_px: f32,
    pub cell_width: u32,
    pub cell_height: u32,
    baseline: f32,
    glyphs: GlyphCache,
    shaped: HashMap<String, Rc<Shaped>>,
    /// Left edge of every column, per row, from the last frame (`cols + 1`
    /// entries per row, relative to the padding).
    row_xs: Vec<Vec<f32>>,
}

/// One drawable unit of a row, in column order.
enum Unit {
    /// A cell the application never wrote. It gives up its width when the
    /// text before it needs room.
    Empty,
    /// A non-Khmer cluster, drawn at its grid width.
    Fixed { text: String, cells: usize, fg: u32 },
    /// A Khmer word: consecutive Khmer cells of one color, shaped together.
    Khmer {
        text: String,
        cells: usize,
        fg: u32,
        natural: f32,
    },
}

impl Unit {
    fn cells(&self) -> usize {
        match self {
            Unit::Empty => 1,
            Unit::Fixed { cells, .. } | Unit::Khmer { cells, .. } => *cells,
        }
    }
}

/// Where each unit of a row goes: `(column, unit, x, width)`.
type Placed = Vec<(usize, Unit, f32, f32)>;

/// Font sizing options.
#[derive(Clone, Copy)]
pub struct Sizing {
    /// Latin font size in pixels.
    pub font_px: f32,
    /// Row height as a multiple of the Latin font's line height.
    pub line_height: f32,
    /// How much of its cells a typical Khmer consonant fills (0.5–1.0).
    pub khmer_fill: f32,
}

impl Renderer {
    pub fn new(latin: Face, khmer: Shaper, model: CellModel, sizing: Sizing) -> Renderer {
        let khmer_face = Face::from_font(khmer.font().clone());
        let mut r = Renderer {
            latin,
            khmer,
            khmer_face,
            theme: Theme::default(),
            model,
            padding: 8,
            latin_px: 0.0,
            khmer_px: 0.0,
            cell_width: 1,
            cell_height: 1,
            baseline: 0.0,
            glyphs: GlyphCache::default(),
            shaped: HashMap::new(),
            row_xs: Vec::new(),
        };
        r.resize_fonts(sizing);
        r
    }

    /// Recomputes cell metrics for a new font size (e.g. a DPI change).
    pub fn resize_fonts(&mut self, sizing: Sizing) {
        let latin = &self.latin;
        let scale = sizing.font_px / latin.units_per_em;
        let zero: i32 = latin.shape("0", None).iter().map(|g| g.x_advance).sum();
        self.cell_width = ((zero as f32 * scale).round() as u32).max(1);
        let line = (latin.ascender + latin.descender + latin.line_gap) * scale;
        self.cell_height = ((line * sizing.line_height).round() as u32).max(1);
        // Center the Latin line box vertically in the row.
        let extra = self.cell_height as f32 - (latin.ascender + latin.descender) * scale;
        self.baseline = (extra / 2.0 + latin.ascender * scale).round();
        self.latin_px = sizing.font_px;
        self.khmer_px =
            self.khmer
                .font_size_for_cell(self.cell_width as f32, self.model, sizing.khmer_fill);
        self.glyphs.clear();
        self.shaped.clear();
    }

    /// Rows and columns that fit in a window of `width` × `height` pixels.
    pub fn grid_size(&self, width: u32, height: u32) -> (usize, usize) {
        let w = width.saturating_sub(2 * self.padding);
        let h = height.saturating_sub(2 * self.padding);
        (
            (h / self.cell_height).max(1) as usize,
            (w / self.cell_width).max(1) as usize,
        )
    }

    /// Window size in pixels for a grid of `rows` × `cols`.
    pub fn window_size(&self, rows: usize, cols: usize) -> (u32, u32) {
        (
            cols as u32 * self.cell_width + 2 * self.padding,
            rows as u32 * self.cell_height + 2 * self.padding,
        )
    }

    /// Grid cell under a pixel position, clamped to the grid. Uses the
    /// layout of the last frame, where Khmer words may have moved text.
    pub fn cell_at(&self, term: &Terminal, x: f64, y: f64) -> (usize, usize) {
        let row = ((y - f64::from(self.padding)) / f64::from(self.cell_height)).max(0.0) as usize;
        let row = row.min(term.rows() - 1);
        let x = (x - f64::from(self.padding)) as f32;
        let col = match self.row_xs.get(row) {
            Some(xs) if xs.len() > 1 => xs[1..]
                .iter()
                .position(|edge| x < *edge)
                .unwrap_or(xs.len() - 2),
            _ => (x / self.cell_width as f32).max(0.0) as usize,
        };
        (row, col.min(term.cols() - 1))
    }

    fn shape(&mut self, text: &str) -> Rc<Shaped> {
        if let Some(s) = self.shaped.get(text) {
            return s.clone();
        }
        let first = text.chars().next().unwrap_or(' ');
        let khmer = kte_core::half_units(text).is_some()
            || (!self.latin.has_char(first) && self.khmer_face.has_char(first));
        let (face, glyphs, px, upem) = if khmer {
            (
                KHMER,
                self.khmer_face.shape(text, Some(harfrust::Script::KHMER)),
                self.khmer_px,
                self.khmer_face.units_per_em,
            )
        } else {
            (
                LATIN,
                self.latin.shape(text, None),
                self.latin_px,
                self.latin.units_per_em,
            )
        };
        let advance: i32 = glyphs.iter().map(|g| g.x_advance).sum();
        let shaped = Rc::new(Shaped {
            face,
            glyphs,
            advance_px: advance as f32 * px / upem,
        });
        if self.shaped.len() > 4096 {
            self.shaped.clear();
        }
        self.shaped.insert(text.to_owned(), shaped.clone());
        shaped
    }

    /// Splits a row into units: empty cells, non-Khmer clusters, and Khmer
    /// words (cells joined while they stay Khmer and keep one color).
    fn units(&mut self, term: &Terminal, row: usize) -> Vec<(usize, Unit)> {
        let khmer = |t: &str| t.chars().next().is_some_and(kte_core::unicode::is_khmer);
        let mut out = Vec::new();
        let mut col = 0;
        while col < term.cols() {
            let cell = term.cell(row, col);
            if cell.is_continuation() {
                col += 1;
                continue;
            }
            if cell.text.is_empty() {
                out.push((col, Unit::Empty));
                col += 1;
                continue;
            }
            let (fg, _) = self.theme.cell_colors(&cell.attrs);
            let mut text = cell.text.clone();
            let mut cells = cell.width.max(1) as usize;
            while col + cells < term.cols() {
                let next = term.cell(row, col + cells);
                if next.width == 0 || next.text.is_empty() {
                    break;
                }
                let same_word = khmer(&text)
                    && khmer(&next.text)
                    && self.theme.cell_colors(&next.attrs).0 == fg;
                if !same_word && !kte_core::unicode::joins_next(&text, &next.text) {
                    break;
                }
                text.push_str(&next.text);
                cells += next.width as usize;
            }
            let shaped = self.shape(&text);
            let unit = if shaped.face == KHMER {
                Unit::Khmer {
                    text,
                    cells,
                    fg,
                    natural: shaped.advance_px,
                }
            } else {
                Unit::Fixed { text, cells, fg }
            };
            out.push((col, unit));
            col += cells;
        }
        out
    }

    /// Lays a row out in pixels. Text starts at its grid position unless the
    /// text before it needs the room; Khmer words keep their natural width,
    /// scaled by `k`. Returns the placement, the column edges, and where the
    /// last non-empty unit ends.
    fn place(units: Vec<(usize, Unit)>, cols: usize, cw: f32, k: f32) -> (Placed, Vec<f32>, f32) {
        let mut xs = vec![0.0f32; cols + 1];
        let mut placed = Vec::with_capacity(units.len());
        let (mut pen, mut content_end) = (0.0f32, 0.0f32);
        for (col, unit) in units {
            let grid = col as f32 * cw;
            let x = grid.max(pen);
            let cells = unit.cells();
            let w = match &unit {
                Unit::Empty => (grid + cw - x).max(0.0),
                Unit::Fixed { cells, .. } => *cells as f32 * cw,
                Unit::Khmer { natural, .. } => natural * k,
            };
            // Columns inside the unit share its width evenly; a unit narrower
            // than its grid cells (other than an empty one) keeps the grid.
            let span = match unit {
                Unit::Empty => w,
                _ => w.max(cells as f32 * cw),
            };
            for i in 0..cells {
                if let Some(edge) = xs.get_mut(col + i) {
                    *edge = x + span * i as f32 / cells as f32;
                }
            }
            pen = x + w;
            if !matches!(unit, Unit::Empty) {
                content_end = content_end.max(pen);
            }
            placed.push((col, unit, x, w));
        }
        xs[cols] = pen.max(cols as f32 * cw);
        (placed, xs, content_end)
    }

    /// Lays out a row, compressing its Khmer words by one shared factor if
    /// the line would otherwise run past the window.
    fn layout_row(&mut self, term: &Terminal, row: usize) -> (Placed, Vec<f32>) {
        let cw = self.cell_width as f32;
        let limit = term.cols() as f32 * cw;
        let units = self.units(term, row);
        let khmer_total: f32 = units
            .iter()
            .map(|(_, u)| {
                if let Unit::Khmer { natural, .. } = u {
                    *natural
                } else {
                    0.0
                }
            })
            .sum();
        let (placed, xs, end) = Self::place(units, term.cols(), cw, 1.0);
        if end <= limit + 0.5 || khmer_total <= 0.0 {
            return (placed, xs);
        }
        let k = ((limit - (end - khmer_total)) / khmer_total).clamp(0.4, 1.0);
        let units = placed.into_iter().map(|(c, u, _, _)| (c, u)).collect();
        let (placed, xs, _) = Self::place(units, term.cols(), cw, k);
        (placed, xs)
    }

    pub fn render(&mut self, term: &Terminal, frame: &mut Frame, overlay: Overlay) {
        let bg = self.theme.background;
        frame.pixels.fill(bg);
        let (cw, ch, pad) = (
            self.cell_width as f32,
            self.cell_height as i32,
            self.padding as f32,
        );
        let layouts: Vec<(Placed, Vec<f32>)> = (0..term.rows())
            .map(|row| self.layout_row(term, row))
            .collect();
        let cursor = term.cursor();
        let cursor_span = term.cluster_span(cursor.0, cursor.1);
        let show_cursor = term.cursor_visible();
        let selected = |row: usize, col: usize| {
            overlay
                .selection
                .is_some_and(|(a, b)| (row, col) >= a && (row, col) <= b)
        };

        // Pass 1: backgrounds, selection, cursor block (all following the layout).
        for (row, (_, xs)) in layouts.iter().enumerate() {
            let y = pad as i32 + row as i32 * ch;
            for col in 0..term.cols() {
                let cell = term.cell(row, col);
                let (_, mut cell_bg) = self.theme.cell_colors(&cell.attrs);
                if selected(row, col) {
                    cell_bg = self.theme.selection;
                }
                if cell_bg != bg {
                    let (x0, x1) = (xs[col].round() as i32, xs[col + 1].round() as i32);
                    frame.fill(pad as i32 + x0, y, (x1 - x0).max(1), ch, cell_bg);
                }
            }
        }
        if show_cursor {
            let xs = &layouts[cursor.0].1;
            let x0 = xs[cursor_span.start.min(term.cols())];
            let x1 = xs[cursor_span.end.min(term.cols())].max(x0 + cw);
            let (x, w) = ((pad + x0).round() as i32, (x1 - x0).round() as i32);
            let y = pad as i32 + cursor.0 as i32 * ch;
            if overlay.focused {
                frame.fill(x, y, w, ch, self.theme.cursor);
                frame.inverse = Some(((x, y, x + w, y + ch), self.theme.background));
            } else {
                let c = self.theme.cursor;
                frame.fill(x, y, w, 1, c);
                frame.fill(x, y + ch - 1, w, 1, c);
                frame.fill(x, y, 1, ch, c);
                frame.fill(x + w - 1, y, 1, ch, c);
            }
        }

        // Pass 2: glyphs.
        self.row_xs.clear();
        for (row, (placed, xs)) in layouts.into_iter().enumerate() {
            let baseline = pad + row as f32 * ch as f32 + self.baseline;
            for (_, unit, x, w) in &placed {
                match unit {
                    Unit::Empty => {}
                    Unit::Fixed { text, cells, fg } => {
                        let shaped = self.shape(text);
                        let f = fit_with_stretch(shaped.advance_px, *cells, cw, 1.0);
                        self.draw_glyphs(
                            frame,
                            &shaped,
                            pad + x + f.offset_x,
                            baseline,
                            f.scale_x,
                            *fg,
                        );
                    }
                    Unit::Khmer {
                        text, fg, natural, ..
                    } => {
                        let shaped = self.shape(text);
                        let scale = if *natural > 0.0 { w / natural } else { 1.0 };
                        self.draw_glyphs(frame, &shaped, pad + x, baseline, scale, *fg);
                    }
                }
            }
            self.row_xs.push(xs);
        }
        frame.inverse = None;
    }

    fn draw_glyphs(
        &mut self,
        frame: &mut Frame,
        shaped: &Shaped,
        x: f32,
        baseline: f32,
        scale_x: f32,
        color: u32,
    ) {
        let (font, px, upem) = if shaped.face == KHMER {
            (
                &self.khmer_face.font,
                self.khmer_px,
                self.khmer_face.units_per_em,
            )
        } else {
            (&self.latin.font, self.latin_px, self.latin.units_per_em)
        };
        let to_px = px / upem;
        let mut pen = x;
        for g in &shaped.glyphs {
            let gx = pen + g.x_offset as f32 * to_px * scale_x;
            let gy = baseline - g.y_offset as f32 * to_px;
            if let Some(mask) = self.glyphs.get(shaped.face, font, g.id, px, scale_x) {
                frame.blend_mask(mask, gx.round() as i32, gy.round() as i32, color);
            }
            pen += g.x_advance as f32 * to_px * scale_x;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn khmer(cells: usize, natural: f32) -> Unit {
        Unit::Khmer {
            text: String::new(),
            cells,
            fg: 0,
            natural,
        }
    }

    fn fixed(cells: usize) -> Unit {
        Unit::Fixed {
            text: String::new(),
            cells,
            fg: 0,
        }
    }

    #[test]
    fn wide_word_pushes_following_text() {
        // A 2-cell Khmer word that needs 30 px (3 cells of 10 px), then "a".
        let units = vec![(0, khmer(2, 30.0)), (2, fixed(1)), (3, Unit::Empty)];
        let (placed, xs, end) = Renderer::place(units, 4, 10.0, 1.0);
        assert_eq!(placed[1].2, 30.0); // "a" starts after the word, not at 20
        assert_eq!(end, 40.0);
        assert_eq!(&xs[..3], &[0.0, 15.0, 30.0]);
    }

    #[test]
    fn empty_cells_absorb_overflow() {
        let units = vec![
            (0, khmer(1, 25.0)),
            (1, Unit::Empty),
            (2, Unit::Empty),
            (3, fixed(1)),
        ];
        let (placed, _, end) = Renderer::place(units, 4, 10.0, 1.0);
        // The word overflows by 15 px; the two empty cells give that back,
        // so the text after them stays on its grid position.
        assert_eq!(placed[3].2, 30.0);
        assert_eq!(end, 40.0);
    }

    #[test]
    fn narrow_word_keeps_the_grid() {
        let units = vec![(0, khmer(3, 12.0)), (3, fixed(1))];
        let (placed, xs, _) = Renderer::place(units, 4, 10.0, 1.0);
        assert_eq!(placed[1].2, 30.0);
        assert_eq!(&xs[..4], &[0.0, 10.0, 20.0, 30.0]);
    }
}
