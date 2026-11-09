//! Khmer text shaping (HarfBuzz, via `harfrust`) and the metrics a terminal
//! needs to draw shaped clusters inside their cells.
//!
//! ```no_run
//! use kte_shaper::Shaper;
//! use kte_unicode::CellModel;
//!
//! let data = std::fs::read("NotoSansKhmer-Regular.ttf").unwrap();
//! let shaper = Shaper::new(data, 0).unwrap();
//! let size = shaper.font_size_for_cell(9.0, CellModel::Wide, 0.9);
//! for cluster in shaper.shape_clusters("ភាសាខ្មែរ", CellModel::Wide).unwrap() {
//!     println!("{:?}: {} glyphs", cluster.placed.cols(), cluster.glyphs.len());
//! }
//! # let _ = size;
//! ```

use std::fmt;

use kte_cell::{Placed, layout};
use kte_unicode::CellModel;

/// Errors from loading fonts or shaping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The data is not a font, or has no font at the requested index.
    InvalidFont,
    /// The font has no glyphs for the Khmer consonants.
    NoKhmerGlyphs,
    /// The shaper rejected the input (usually a malformed font).
    Shaping(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidFont => write!(f, "not a font, or no font at that index"),
            Error::NoKhmerGlyphs => write!(f, "font has no Khmer consonant glyphs"),
            Error::Shaping(e) => write!(f, "shaping failed: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// One positioned glyph, in font units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    /// Glyph index in the font.
    pub glyph_id: u32,
    /// UTF-8 byte offset of the source cluster this glyph belongs to.
    pub cluster: u32,
    /// Horizontal advance.
    pub x_advance: i32,
    /// Horizontal offset from the pen position.
    pub x_offset: i32,
    /// Vertical offset from the baseline (positive is up).
    pub y_offset: i32,
}

/// A KTE cluster with its grid placement and shaped glyphs.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapedCluster {
    /// Byte range, column and width in cells.
    pub placed: Placed,
    /// The glyphs drawn for this cluster, in visual order.
    pub glyphs: Vec<Glyph>,
    /// Sum of the glyphs' advances, in font units.
    pub advance: i32,
}

/// A Khmer font ready for shaping.
pub struct Shaper {
    font: harfrust::Font,
    units_per_em: u16,
    consonant_unit: f32,
}

impl fmt::Debug for Shaper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Shaper")
            .field("units_per_em", &self.units_per_em)
            .field("consonant_unit", &self.consonant_unit)
            .finish_non_exhaustive()
    }
}

impl Shaper {
    /// Loads the font at `index` in `data` (0 for a single font, or the
    /// position in a `.ttc` collection).
    pub fn new(data: Vec<u8>, index: u32) -> Result<Self, Error> {
        let font = harfrust::Font::new(data, index).ok_or(Error::InvalidFont)?;
        let units_per_em = font.units_per_em();
        let charmap = font.charmap();
        if ('\u{1780}'..='\u{17A2}').any(|c| charmap.map_unicode(c).is_none()) {
            return Err(Error::NoKhmerGlyphs);
        }
        let mut shaper = Shaper {
            font,
            units_per_em,
            consonant_unit: 0.0,
        };
        let mut advances = ('\u{1780}'..='\u{17A2}')
            .map(|c| {
                Ok(shaper
                    .shape(&c.to_string())?
                    .iter()
                    .map(|g| g.x_advance)
                    .sum::<i32>())
            })
            .collect::<Result<Vec<i32>, Error>>()?;
        advances.sort_unstable();
        shaper.consonant_unit = advances[advances.len() / 2] as f32;
        if shaper.consonant_unit <= 0.0 {
            return Err(Error::NoKhmerGlyphs);
        }
        Ok(shaper)
    }

    /// The underlying font, for drawing glyph outlines.
    pub fn font(&self) -> &harfrust::Font {
        &self.font
    }

    /// The font's units per em.
    pub fn units_per_em(&self) -> u16 {
        self.units_per_em
    }

    /// Median advance of the Khmer consonants in font units: the font's
    /// "typical consonant", which the width spec counts as 2 half-units.
    pub fn consonant_unit(&self) -> f32 {
        self.consonant_unit
    }

    /// Shapes `text` as one Khmer run and returns its glyphs in visual order.
    ///
    /// Pass runs that this font should draw (Khmer text); other scripts
    /// belong to their own fonts.
    pub fn shape(&self, text: &str) -> Result<Vec<Glyph>, Error> {
        let shaper = harfrust::ShaperFont::new(&self.font);
        let mut buffer = harfrust::Buffer::new();
        buffer.push_str(text);
        buffer.set_direction(harfrust::Direction::LeftToRight);
        buffer.set_script(Some(harfrust::Script::KHMER));
        harfrust::shape(&shaper, &mut buffer, harfrust::ShapeOptions::new())
            .map_err(|e| Error::Shaping(format!("{e:?}")))?;
        Ok(buffer
            .glyph_infos()
            .iter()
            .zip(buffer.glyph_positions())
            .map(|(info, pos)| Glyph {
                glyph_id: info.glyph_id,
                cluster: info.cluster,
                x_advance: pos.x_advance,
                x_offset: pos.x_offset,
                y_offset: pos.y_offset,
            })
            .collect())
    }

    /// Shapes `text` in one pass and groups the glyphs by KTE cluster, each
    /// with its column and cell width under `model`.
    pub fn shape_clusters(
        &self,
        text: &str,
        model: CellModel,
    ) -> Result<Vec<ShapedCluster>, Error> {
        let glyphs = self.shape(text)?;
        let mut out: Vec<ShapedCluster> = layout(text, model)
            .into_iter()
            .map(|placed| ShapedCluster {
                placed,
                glyphs: Vec::new(),
                advance: 0,
            })
            .collect();
        for glyph in glyphs {
            let byte = glyph.cluster as usize;
            // HarfBuzz may merge clusters; a glyph belongs to the KTE cluster
            // that contains its cluster offset.
            let i = out
                .iter()
                .rposition(|c| c.placed.bytes.start <= byte)
                .unwrap_or(0);
            if let Some(cluster) = out.get_mut(i) {
                cluster.advance += glyph.x_advance;
                cluster.glyphs.push(glyph);
            }
        }
        Ok(out)
    }

    /// Font size (in pixels per em) at which a typical consonant spans
    /// `fill` × its cells: 2 cells in the Wide model, 1 in Compact and Legacy.
    ///
    /// `fill` below 1.0 leaves even spacing between syllables; 0.85–0.95 is a
    /// good range.
    pub fn font_size_for_cell(&self, cell_width_px: f32, model: CellModel, fill: f32) -> f32 {
        let cells_per_consonant = match model {
            CellModel::Wide => 2.0,
            CellModel::Compact | CellModel::Legacy(_) => 1.0,
        };
        cells_per_consonant * cell_width_px * fill * f32::from(self.units_per_em)
            / self.consonant_unit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Finds an installed Khmer font: `KTE_TEST_FONT`, or a known system path.
    fn test_font() -> Option<Vec<u8>> {
        let mut candidates: Vec<String> = std::env::var("KTE_TEST_FONT").into_iter().collect();
        let home = std::env::var("HOME").unwrap_or_default();
        candidates.extend(
            [
                "/Library/Fonts/NotoSansKhmer-Regular.ttf",
                "/System/Library/Fonts/Supplemental/Khmer Sangam MN.ttf",
                "/usr/share/fonts/truetype/noto/NotoSansKhmer-Regular.ttf",
                "/usr/share/fonts/truetype/khmeros/KhmerOS.ttf",
                "C:\\Windows\\Fonts\\KhmerUI.ttf",
                "C:\\Windows\\Fonts\\LeelawUI.ttf",
            ]
            .map(String::from),
        );
        candidates.push(format!("{home}/Library/Fonts/NotoSansKhmer-Regular.ttf"));
        candidates.into_iter().find_map(|p| std::fs::read(p).ok())
    }

    macro_rules! shaper_or_skip {
        () => {
            match test_font() {
                Some(data) => Shaper::new(data, 0).expect("load test font"),
                None => {
                    eprintln!("skipping: no Khmer font found (set KTE_TEST_FONT)");
                    return;
                }
            }
        };
    }

    #[test]
    fn rejects_non_fonts() {
        assert_eq!(
            Shaper::new(b"not a font".to_vec(), 0).unwrap_err(),
            Error::InvalidFont
        );
    }

    #[test]
    fn consonant_unit_is_positive() {
        let shaper = shaper_or_skip!();
        assert!(shaper.consonant_unit() > 0.0);
        assert!(shaper.units_per_em() > 0);
    }

    #[test]
    fn glyphs_group_into_kte_clusters() {
        let shaper = shaper_or_skip!();
        let text = "ស្ត្រីកោះ";
        let clusters = shaper.shape_clusters(text, CellModel::Wide).unwrap();
        assert_eq!(clusters.len(), 2);
        assert_eq!(&text[clusters[0].placed.bytes.clone()], "ស្ត្រី");
        assert_eq!(clusters[1].placed.col, 4);
        for c in &clusters {
            assert!(!c.glyphs.is_empty(), "every cluster has glyphs");
            assert!(c.advance > 0);
        }
    }

    #[test]
    fn spec_widths_track_real_advances() {
        // The spec is a best fit across fonts, so individual glyphs may
        // differ in a given font (e.g. ស is narrower in Windows' Khmer UI).
        // Over real text, most syllables should still fit their cells within
        // ±25% after one global scale, as in Research 0001.
        let shaper = shaper_or_skip!();
        let half = shaper.consonant_unit() / 2.0;
        let text = "មនុស្សទាំងអស់កើតមកមានសេរីភាពនិងសមភាពក្នុងសិទ្ធិនិងសេចក្ដីថ្លៃថ្នូរ\
                    ខ្ញុំស្រឡាញ់ការសរសេរកម្មវិធីជាភាសាខ្មែរ";
        let mut fits: Vec<f32> = shaper
            .shape_clusters(text, CellModel::Wide)
            .unwrap()
            .iter()
            .filter(|c| c.placed.width > 0)
            .map(|c| c.advance as f32 / half / c.placed.width as f32)
            .collect();
        fits.sort_by(f32::total_cmp);
        let scale = fits[fits.len() / 2];
        let good = fits
            .iter()
            .filter(|f| (0.75..=1.25).contains(&(*f / scale)))
            .count() as f32
            / fits.len() as f32;
        assert!(
            good >= 0.8,
            "only {:.0}% of syllables fit within ±25% (scale {scale:.2})",
            good * 100.0
        );
    }

    #[test]
    fn font_size_scales_with_model() {
        let shaper = shaper_or_skip!();
        let wide = shaper.font_size_for_cell(10.0, CellModel::Wide, 1.0);
        let compact = shaper.font_size_for_cell(10.0, CellModel::Compact, 1.0);
        assert!((wide - 2.0 * compact).abs() < 1e-3);
        // At that size a typical consonant spans exactly 2 cells.
        let px = shaper.consonant_unit() * wide / f32::from(shaper.units_per_em());
        assert!((px - 20.0).abs() < 1e-3);
    }
}
