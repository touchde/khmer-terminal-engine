//! Font discovery, metrics, shaping and glyph rasterization.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use harfrust::font::glyph::outline::{DrawMemory, DrawOptions, PathSink};
use tiny_skia::{FillRule, Mask, PathBuilder, Transform};

/// A font file plus the index of the face inside it (for `.ttc` collections).
#[derive(Debug, Clone)]
pub struct FontSource {
    pub path: PathBuf,
    pub index: u32,
}

/// Fonts are the host's defaults, so kte-term looks like the rest of the
/// system (and like the same text in VS Code or Terminal.app):
///
/// - macOS: SF Mono (Terminal.app's default) and Khmer Sangam MN (the font
///   macOS itself uses for Khmer).
/// - Windows: Cascadia Mono (Windows Terminal's default) or Consolas, and
///   Leelawadee UI or Khmer UI (Windows' Khmer fallback).
/// - Linux and other Unix: whatever fontconfig resolves `monospace` and
///   Khmer (`:lang=km`) to, honoring the user's fontconfig settings.
///
/// `KTE_FONT` / `KTE_KHMER_FONT` override the choice.
fn find(env: &str, fc_pattern: &str, candidates: &[&str]) -> Option<FontSource> {
    if let Some(path) = std::env::var_os(env)
        .map(PathBuf::from)
        .filter(|p| p.is_file())
    {
        return Some(FontSource { path, index: 0 });
    }
    if cfg!(all(unix, not(target_vendor = "apple"))) {
        if let Some(found) = fc_match(fc_pattern) {
            return Some(found);
        }
    }
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .map(|path| FontSource { path, index: 0 })
}

/// Asks fontconfig which font file it would use for `pattern`.
fn fc_match(pattern: &str) -> Option<FontSource> {
    let out = std::process::Command::new("fc-match")
        .args(["--format=%{file}\n%{index}", pattern])
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    let mut lines = text.lines();
    let path = PathBuf::from(lines.next()?);
    let index = lines
        .next()
        .and_then(|i| i.trim().parse().ok())
        .unwrap_or(0);
    path.is_file().then_some(FontSource { path, index })
}

/// The host's default monospace font.
pub fn find_latin() -> Option<FontSource> {
    find(
        "KTE_FONT",
        "monospace",
        &[
            // macOS
            "/System/Library/Fonts/SFNSMono.ttf",
            "/System/Library/Fonts/Menlo.ttc",
            // Windows
            "C:\\Windows\\Fonts\\CascadiaMono.ttf",
            "C:\\Windows\\Fonts\\consola.ttf",
            // Unix without fontconfig
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
        ],
    )
}

/// The host's default Khmer font.
pub fn find_khmer() -> Option<FontSource> {
    find(
        "KTE_KHMER_FONT",
        ":lang=km",
        &[
            // macOS
            "/System/Library/Fonts/Supplemental/Khmer Sangam MN.ttf",
            "/System/Library/Fonts/Supplemental/Khmer MN.ttc",
            // Windows
            "C:\\Windows\\Fonts\\LeelawUI.ttf",
            "C:\\Windows\\Fonts\\KhmerUI.ttf",
            // Unix without fontconfig
            "/usr/share/fonts/truetype/noto/NotoSansKhmer-Regular.ttf",
            "/usr/share/fonts/truetype/khmeros/KhmerOSsys.ttf",
        ],
    )
}

pub fn read(source: &FontSource) -> Result<Vec<u8>, String> {
    std::fs::read(&source.path).map_err(|e| format!("{}: {e}", source.path.display()))
}

/// A glyph positioned by shaping, in font units.
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    pub id: u32,
    pub x_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
}

/// A loaded font with the metrics the renderer needs.
pub struct Face {
    pub font: harfrust::Font,
    pub units_per_em: f32,
    /// Distance from baseline to the top of the line, in font units (positive).
    pub ascender: f32,
    /// Distance from baseline to the bottom of the line, in font units (positive).
    pub descender: f32,
    pub line_gap: f32,
}

impl Face {
    pub fn load(source: &FontSource) -> Result<Face, String> {
        let data = read(source)?;
        let font = harfrust::Font::new(data, source.index)
            .ok_or_else(|| format!("{}: not a font", source.path.display()))?;
        Ok(Face::from_font(font))
    }

    pub fn from_font(font: harfrust::Font) -> Face {
        let metrics = font.metrics();
        let units_per_em = f32::from(metrics.units_per_em);
        let line = match (
            metrics.use_typo_metrics,
            metrics.typo_line,
            metrics.hhea_line,
        ) {
            (true, Some(typo), _) => Some(typo),
            (_, _, Some(hhea)) => Some(hhea),
            (_, typo, None) => typo,
        };
        let (ascender, descender, line_gap) = match line {
            Some(l) => (
                l.ascender.to_f64() as f32,
                -(l.descender.to_f64() as f32),
                l.line_gap.to_f64() as f32,
            ),
            None => (units_per_em * 0.8, units_per_em * 0.2, 0.0),
        };
        Face {
            font,
            units_per_em,
            ascender,
            descender,
            line_gap,
        }
    }

    /// Height of the ink of `c`'s glyph, in font units.
    pub fn ink_height(&self, c: char) -> Option<f32> {
        let glyph = self.font.charmap().map_unicode(c)?;
        let extents = self.font.outline_glyphs().get(glyph)?.extents().ok()?;
        (extents.height > 0.0).then_some(extents.height)
    }

    pub fn has_char(&self, c: char) -> bool {
        self.font.charmap().map_unicode(c).is_some()
    }

    /// Shapes `text`; `script` is guessed from the text when `None`.
    pub fn shape(&self, text: &str, script: Option<harfrust::Script>) -> Vec<ShapedGlyph> {
        let shaper = harfrust::ShaperFont::new(&self.font);
        let mut buffer = harfrust::Buffer::new();
        buffer.push_str(text);
        match script {
            Some(script) => {
                buffer.set_script(Some(script));
                buffer.set_direction(harfrust::Direction::LeftToRight);
            }
            None => buffer.guess_segment_properties(),
        }
        if harfrust::shape(&shaper, &mut buffer, harfrust::ShapeOptions::new()).is_err() {
            return Vec::new();
        }
        buffer
            .glyph_infos()
            .iter()
            .zip(buffer.glyph_positions())
            .map(|(info, pos)| ShapedGlyph {
                id: info.glyph_id,
                x_advance: pos.x_advance,
                x_offset: pos.x_offset,
                y_offset: pos.y_offset,
            })
            .collect()
    }
}

/// An antialiased glyph coverage mask, positioned relative to the pen
/// (origin on the baseline, y growing downward).
pub struct GlyphMask {
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
    pub alpha: Vec<u8>,
}

/// Builds a tiny-skia path from font outline commands, flipping y so that
/// it grows downward.
struct Sink {
    path: PathBuilder,
    scale_x: f32,
}

impl PathSink for Sink {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to(x * self.scale_x, -y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to(x * self.scale_x, -y);
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.path
            .quad_to(cx * self.scale_x, -cy, x * self.scale_x, -y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let s = self.scale_x;
        self.path.cubic_to(cx0 * s, -cy0, cx1 * s, -cy1, x * s, -y);
    }
    fn close(&mut self) {
        self.path.close();
    }
}

/// Rasterizes `glyph` at `px` pixels per em, compressed horizontally by `scale_x`.
pub fn rasterize(font: &harfrust::Font, glyph: u32, px: f32, scale_x: f32) -> Option<GlyphMask> {
    let outline = font.outline_glyphs().get(harfrust::GlyphId::new(glyph))?;
    let mut sink = Sink {
        path: PathBuilder::new(),
        scale_x,
    };
    outline
        .draw(
            &DrawOptions::new().with_ppem(Some(px)),
            DrawMemory::Scratch,
            &mut sink,
        )
        .ok()?;
    let path = sink.path.finish()?;
    let bounds = path.bounds();
    let left = bounds.left().floor();
    let top = bounds.top().floor();
    let width = (bounds.right().ceil() - left) as u32 + 1;
    let height = (bounds.bottom().ceil() - top) as u32 + 1;
    let mut mask = Mask::new(width, height)?;
    mask.fill_path(
        &path,
        FillRule::Winding,
        true,
        Transform::from_translate(-left, -top),
    );
    Some(GlyphMask {
        width,
        height,
        left: left as i32,
        top: top as i32,
        alpha: mask.data().to_vec(),
    })
}

/// Cache of rasterized glyphs, keyed by face, glyph, size and horizontal scale.
#[derive(Default)]
pub struct GlyphCache {
    map: HashMap<(u8, u32, u32, u32), Option<GlyphMask>>,
}

impl GlyphCache {
    pub fn get(
        &mut self,
        face_id: u8,
        font: &harfrust::Font,
        glyph: u32,
        px: f32,
        scale_x: f32,
    ) -> Option<&GlyphMask> {
        // Quantize the scale so near-identical compressions share an entry.
        let scale_key = (scale_x * 64.0).round() as u32;
        self.map
            .entry((face_id, glyph, px.to_bits(), scale_key))
            .or_insert_with(|| rasterize(font, glyph, px, scale_key as f32 / 64.0))
            .as_ref()
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }
}

pub fn describe(source: &FontSource) -> String {
    Path::new(&source.path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}
