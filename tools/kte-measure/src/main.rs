//! Measures shaped Khmer advances in real fonts to inform the KTE Width Spec.
//!
//! Usage: `cargo run -p kte-measure --release -- <font file or directory>...`
//!
//! For every font that covers the Khmer block, text is shaped with harfrust
//! (the HarfBuzz project's Rust port) and advances are expressed in **consonant units**: the
//! median advance of the 35 Khmer consonants shaped alone in that font.
//!
//! The report (Markdown, on stdout) has three parts:
//!
//! 1. How much each sign or subscript widens a syllable.
//! 2. A width table derived from that data, in half-consonant steps.
//! 3. An evaluation of candidate cell-width rules against real sentences,
//!    syllable by syllable.

use std::collections::BTreeMap;

use read_fonts::types::NameId;
use std::fs;
use std::path::{Path, PathBuf};

const COENG: char = '\u{17D2}';
/// Reference base consonant for sign and subscript measurements: KA.
const KA: char = '\u{1780}';

fn consonants() -> impl Iterator<Item = char> {
    '\u{1780}'..='\u{17A2}'
}

/// Characters that start a syllable: consonants and independent vowels
/// (excluding the deprecated U+17A3 and U+17A4).
fn bases() -> impl Iterator<Item = char> {
    ('\u{1780}'..='\u{17B3}').filter(|c| !matches!(c, '\u{17A3}' | '\u{17A4}'))
}

/// Dependent vowels and signs with General_Category Mc (spacing).
const SPACING: &[char] = &[
    '\u{17B6}', '\u{17BE}', '\u{17BF}', '\u{17C0}', '\u{17C1}', '\u{17C2}', '\u{17C3}', '\u{17C4}',
    '\u{17C5}', '\u{17C7}', '\u{17C8}',
];

/// Two-part vowels: a pre-base part plus a post-base part.
const TWO_PART: &[char] = &['\u{17BF}', '\u{17C0}', '\u{17C4}', '\u{17C5}'];

/// Dependent vowels and signs with General_Category Mn (non-spacing).
const NON_SPACING: &[char] = &[
    '\u{17B7}', '\u{17B8}', '\u{17B9}', '\u{17BA}', '\u{17BB}', '\u{17BC}', '\u{17BD}', '\u{17C6}',
    '\u{17C9}', '\u{17CA}', '\u{17CB}', '\u{17CC}', '\u{17CD}', '\u{17CE}', '\u{17CF}', '\u{17D0}',
    '\u{17D1}', '\u{17D3}', '\u{17DD}',
];

/// Other Khmer characters that stand alone: punctuation, currency, digits,
/// numeric symbols and lunar date symbols.
fn others() -> impl Iterator<Item = char> {
    ('\u{17D4}'..='\u{17DC}')
        .chain('\u{17E0}'..='\u{17E9}')
        .chain('\u{17F0}'..='\u{17F9}')
        .chain('\u{19E0}'..='\u{19FF}')
}

/// Fonts a terminal is likely to use for Khmer (system and UI fonts).
const COMMON_FONTS: &[&str] = &[
    "Noto Sans Khmer",
    "Noto Sans Khmer UI",
    "Noto Serif Khmer",
    "Kantumruy",
    "Kantumruy Pro",
    "Battambang",
    "Hanuman",
    "Nokora",
    "Siemreap",
    "Content",
    "Khmer OS",
    "Khmer OS System",
    "Khmer OS Siemreap",
    "Khmer OS Battambang",
    "Khmer OS Content",
    "Khmer Sangam MN",
    "Khmer MN",
    "Leelawadee UI",
    "Khmer UI",
    "DaunPenh",
    "MoolBoran",
];

/// Sample text: test corpus phrases and common sentences (UDHR article 1, national motto).
const SAMPLE: &[&str] = &[
    "សួស្តី ពិភពលោក!",
    "ភាសាខ្មែរ",
    "ខ្ញុំស្រឡាញ់ការសរសេរកម្មវិធី",
    "ការអភិវឌ្ឍកម្មវិធីជាភាសាខ្មែរ",
    "ក្តីស្រមៃ",
    "សក្តានុពល",
    "មនុស្សទាំងអស់កើតមកមានសេរីភាព និងសមភាព ក្នុងសិទ្ធិ និងសេចក្ដីថ្លៃថ្នូរ។",
    "មនុស្សគ្រប់រូបសុទ្ធតែមានវិចារណញ្ញាណ និងសតិសម្បជញ្ញៈ ហើយត្រូវប្រព្រឹត្តចំពោះគ្នាទៅវិញទៅមក ក្នុងស្មារតីភាតរភាពជាបងប្អូន។",
    "ព្រះរាជាណាចក្រកម្ពុជា ជាតិ សាសនា ព្រះមហាក្សត្រ",
    "ខ្ញុំរៀនភាសាខ្មែរនៅសាលារៀន។",
    "កៅអី ឪពុក ម្តាយ ស្ត្រី ប្រទេសកម្ពុជា",
];

struct Font {
    name: String,
    font: harfrust::Font,
}

struct Metrics {
    name: String,
    /// Advance of each base (same order as `bases()`), in consonant units.
    base: Vec<f32>,
    /// Advance of each `others()` character, in consonant units; `None` if the font lacks it.
    other: Vec<Option<f32>>,
    /// Extra advance each spacing sign adds to KA, in consonant units.
    spacing: Vec<f32>,
    /// Extra advance each non-spacing sign adds to KA, in consonant units.
    non_spacing: Vec<f32>,
    /// Extra advance of KA + coeng + consonant over KA, in consonant units.
    coeng: Vec<f32>,
    /// Advance of each sample syllable, in consonant units (same order as `syllables()`).
    syllables: Vec<f32>,
}

fn main() {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.is_empty() {
        eprintln!("usage: kte-measure <font file or directory>...");
        std::process::exit(2);
    }

    let mut fonts = Vec::new();
    for path in &args {
        collect_fonts(path, &mut fonts);
    }
    // Hidden system fonts (e.g. `.LastResort`) draw placeholder boxes, not Khmer.
    fonts.retain(|f| !f.name.starts_with('.'));

    let syllables = syllables();
    let mut results: Vec<Metrics> = fonts
        .iter()
        .filter_map(|f| measure(f, &syllables))
        .collect();
    results.sort_by(|a, b| a.name.cmp(&b.name));
    results.dedup_by(|a, b| a.name == b.name);

    report(&results, fonts.len(), &syllables);
}

fn collect_fonts(path: &Path, out: &mut Vec<Font>) {
    if path.is_dir() {
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            collect_fonts(&p, out);
        }
        return;
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "ttf" | "otf" | "ttc") {
        return;
    }
    let Ok(data) = fs::read(path) else { return };
    // A collection (.ttc) holds several fonts; `Font::new` returns `None` past the last one.
    for index in 0.. {
        let Some(font) = harfrust::Font::new(data.clone(), index) else {
            break;
        };
        let name = font_name(&font).unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
        out.push(Font {
            name: name.trim().to_owned(),
            font,
        });
    }
}

fn font_name(font: &harfrust::Font) -> Option<String> {
    let names = font.names();
    [NameId::FULL_NAME, NameId::FAMILY_NAME]
        .into_iter()
        .find_map(|id| names.get(id)?.english_or_first())
        .map(|n| n.name().chars().collect())
}

/// Splits Khmer text into syllables: a new syllable starts at a consonant,
/// independent vowel or Khmer punctuation that does not follow a coeng.
/// Spaces and non-Khmer characters are dropped.
fn split_syllables(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut after_coeng = false;
    for c in text.chars() {
        let starts = matches!(c, '\u{1780}'..='\u{17B3}' | '\u{17D4}'..='\u{17DC}') && !after_coeng;
        if starts {
            out.push(String::new());
        }
        if ('\u{1780}'..='\u{17FF}').contains(&c) {
            if let Some(last) = out.last_mut() {
                last.push(c);
            }
        }
        after_coeng = c == COENG;
    }
    out
}

/// Every syllable in the sample text, with repetitions (frequency matters).
fn syllables() -> Vec<String> {
    SAMPLE.iter().flat_map(|s| split_syllables(s)).collect()
}

fn measure(font: &Font, syllables: &[String]) -> Option<Metrics> {
    let shaper = harfrust::ShaperFont::new(&font.font);
    let charmap = font.font.charmap();

    // Only Unicode-encoded Khmer fonts: legacy fonts map Khmer glyphs onto Latin codepoints.
    let required = bases()
        .chain(SPACING.iter().copied())
        .chain(NON_SPACING.iter().copied());
    if required
        .chain([COENG])
        .any(|c| charmap.map_unicode(c).is_none())
    {
        return None;
    }

    let adv = |s: &str| advance(&shaper, s);
    let mut consonant_advances = consonants()
        .map(|c| adv(&c.to_string()))
        .collect::<Option<Vec<f32>>>()?;
    let unit = percentile(&mut consonant_advances, 0.5);
    if unit <= 0.0 {
        return None;
    }

    let base = adv(&KA.to_string())?;
    let extra = |s: String| Some((adv(&s)? - base) / unit);

    Some(Metrics {
        name: font.name.clone(),
        base: bases()
            .map(|c| Some(adv(&c.to_string())? / unit))
            .collect::<Option<_>>()?,
        other: others()
            .map(|c| {
                charmap.map_unicode(c)?;
                Some(adv(&c.to_string())? / unit)
            })
            .collect(),
        spacing: SPACING
            .iter()
            .map(|v| extra(format!("{KA}{v}")))
            .collect::<Option<_>>()?,
        non_spacing: NON_SPACING
            .iter()
            .map(|v| extra(format!("{KA}{v}")))
            .collect::<Option<_>>()?,
        coeng: consonants()
            .map(|c| extra(format!("{KA}{COENG}{c}")))
            .collect::<Option<_>>()?,
        syllables: syllables
            .iter()
            .map(|s| Some(adv(s)? / unit))
            .collect::<Option<_>>()?,
    })
}

/// Total shaped advance of `text`, in font units. `None` if shaping fails
/// (e.g. on a malformed font).
fn advance(shaper: &harfrust::ShaperFont, text: &str) -> Option<f32> {
    let mut buffer = harfrust::Buffer::new();
    buffer.push_str(text);
    buffer.set_direction(harfrust::Direction::LeftToRight);
    buffer.set_script(Some(harfrust::Script::KHMER));
    harfrust::shape(shaper, &mut buffer, harfrust::ShapeOptions::new()).ok()?;
    Some(
        buffer
            .glyph_positions()
            .iter()
            .map(|p| p.x_advance as f32)
            .sum(),
    )
}

/// Linear-interpolated percentile (`q` in 0..=1). Sorts `values`.
fn percentile(values: &mut [f32], q: f32) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f32::total_cmp);
    let pos = q * (values.len() - 1) as f32;
    let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
    values[lo] + (values[hi] - values[lo]) * (pos - lo as f32)
}

/// (p10, median, p90) across fonts of `pick(font)`.
fn spread(results: &[&Metrics], pick: impl Fn(&Metrics) -> f32) -> (f32, f32, f32) {
    let mut v: Vec<f32> = results.iter().map(|m| pick(m)).collect();
    (
        percentile(&mut v, 0.1),
        percentile(&mut v, 0.5),
        percentile(&mut v, 0.9),
    )
}

/// Rounds to the nearest half.
fn half(x: f32) -> f32 {
    (x * 2.0).round() / 2.0
}

/// Per-codepoint widths derived from the measurements, in half-consonant steps.
struct Derived {
    base: BTreeMap<char, f32>,
    spacing: BTreeMap<char, f32>,
    subscript: BTreeMap<char, f32>,
}

fn derive(results: &[&Metrics]) -> Derived {
    let spacing = SPACING
        .iter()
        .enumerate()
        .map(|(i, &v)| (v, half(spread(results, |m| m.spacing[i]).1)))
        .collect();
    let subscript = consonants()
        .enumerate()
        .map(|(i, c)| (c, half(spread(results, |m| m.coeng[i]).1)))
        .collect();
    let base = bases()
        .enumerate()
        .map(|(i, c)| (c, half(spread(results, |m| m.base[i]).1)))
        .collect();
    Derived {
        base,
        spacing,
        subscript,
    }
}

/// A candidate rule: cell width of one syllable.
struct Rule {
    name: &'static str,
    width: Box<dyn Fn(&str) -> f32>,
}

/// Sums per-codepoint widths over a syllable. `sign(c)` gives a spacing sign's
/// width and `sub(c)` a subscript consonant's width.
fn sum_width(
    syllable: &str,
    base: &dyn Fn(char) -> f32,
    sign: &dyn Fn(char) -> f32,
    sub: &dyn Fn(char) -> f32,
) -> f32 {
    let mut width = 0.0;
    let mut after_coeng = false;
    for c in syllable.chars() {
        width += match c {
            COENG => 0.0,
            '\u{1780}'..='\u{17B3}' if after_coeng => sub(c),
            '\u{1780}'..='\u{17B3}' => base(c),
            '\u{17D4}'..='\u{17DC}' => 1.0,
            c if SPACING.contains(&c) => sign(c),
            _ => 0.0,
        };
        after_coeng = c == COENG;
    }
    width
}

fn rules(d: &Derived) -> Vec<Rule> {
    // Each closure gets its own copy of the derived tables.
    let tables = || (d.base.clone(), d.spacing.clone(), d.subscript.clone());
    let measured = |all_bases: bool, finish: fn(f32) -> f32| -> Box<dyn Fn(&str) -> f32> {
        let (b, sp, sb) = tables();
        Box::new(move |s| {
            let base = |c: char| if all_bases { b[&c] } else { 1.0 };
            finish(sum_width(s, &base, &|c| sp[&c], &|c| sb[&c]))
        })
    };
    let one = |_: char| 1.0;
    let zero = |_: char| 0.0;
    vec![
        Rule {
            name: "A. Bases = 1, spacing signs = 0",
            width: Box::new(move |s| sum_width(s, &one, &zero, &zero)),
        },
        Rule {
            name: "B. Bases = 1, spacing signs = 1",
            width: Box::new(move |s| sum_width(s, &one, &one, &zero)),
        },
        Rule {
            name: "C. Bases = 1, two-part vowels = 1, other signs = 0",
            width: Box::new(move |s| {
                sum_width(
                    s,
                    &one,
                    &|c| if TWO_PART.contains(&c) { 1.0 } else { 0.0 },
                    &zero,
                )
            }),
        },
        Rule {
            name: "D. Bases = 1, measured signs/subscripts, rounded up",
            width: measured(false, f32::ceil),
        },
        Rule {
            name: "E. Bases = 1, measured signs/subscripts, rounded down",
            width: measured(false, |x| (x - 0.5).ceil()),
        },
        Rule {
            name: "F. Bases = 1, measured signs/subscripts, half cells (×2)",
            width: measured(false, |x| 2.0 * x),
        },
        Rule {
            name: "G. Everything measured, rounded up",
            width: measured(true, f32::ceil),
        },
        Rule {
            name: "H. Everything measured, rounded to nearest (0.5 → up)",
            width: measured(true, |x| (x + 0.01).round()),
        },
        Rule {
            name: "I. Everything measured, half cells (×2)",
            width: measured(true, |x| 2.0 * x),
        },
    ]
}

fn report(all: &[Metrics], scanned: usize, syllables: &[String]) {
    let all: Vec<&Metrics> = all.iter().collect();
    let common: Vec<&Metrics> = all
        .iter()
        .copied()
        .filter(|m| COMMON_FONTS.contains(&m.name.as_str()))
        .collect();

    println!("# Khmer advance measurements\n");
    println!(
        "Generated by `tools/kte-measure`. Font faces scanned: {scanned}. \
         Unicode Khmer fonts measured: **{}**, of which **{}** are common system/UI fonts. \
         Legacy-encoded fonts and fonts with missing glyphs are skipped.\n",
        all.len(),
        common.len()
    );
    if all.is_empty() {
        return;
    }
    println!(
        "Values are in **consonant units**: the median advance of a Khmer consonant in the same font. \
         Each row shows the median across fonts and the 10th–90th percentile range.\n"
    );

    println!("## 1. How much each sign widens ក\n");
    println!("| Sign | Type | Median | p10–p90 |\n|---|---|---|---|");
    for (i, v) in SPACING.iter().enumerate() {
        let (lo, med, hi) = spread(&all, |m| m.spacing[i]);
        let kind = if TWO_PART.contains(v) {
            "Mc, two-part"
        } else {
            "Mc"
        };
        println!(
            "| ក{v} (U+{:04X}) | {kind} | {med:.2} | {lo:.2}–{hi:.2} |",
            *v as u32
        );
    }
    for (i, v) in NON_SPACING.iter().enumerate() {
        let (lo, med, hi) = spread(&all, |m| m.non_spacing[i]);
        println!(
            "| ក{v} (U+{:04X}) | Mn | {med:.2} | {lo:.2}–{hi:.2} |",
            *v as u32
        );
    }

    println!("\n## 2. How much each subscript widens ក (ក + coeng + consonant)\n");
    println!("| Subscript | Median | p10–p90 |\n|---|---|---|");
    let mut rows: Vec<(char, (f32, f32, f32))> = consonants()
        .enumerate()
        .map(|(i, c)| (c, spread(&all, |m| m.coeng[i])))
        .collect();
    rows.sort_by(|a, b| b.1.1.total_cmp(&a.1.1).then(a.0.cmp(&b.0)));
    for (c, (lo, med, hi)) in rows {
        println!("| ក្{c} (U+{:04X}) | {med:.2} | {lo:.2}–{hi:.2} |", c as u32);
    }

    println!("\n## 2b. Other Khmer characters (standalone advance)\n");
    println!("| Character | Fonts with glyph | Median | p10–p90 |\n|---|---|---|---|");
    for (i, c) in others().enumerate() {
        let mut v: Vec<f32> = all.iter().filter_map(|m| m.other[i]).collect();
        if v.is_empty() {
            println!("| {c} U+{:04X} | 0 | – | – |", c as u32);
            continue;
        }
        let n = v.len();
        let (lo, med, hi) = (
            percentile(&mut v, 0.1),
            percentile(&mut v, 0.5),
            percentile(&mut v, 0.9),
        );
        println!(
            "| {c} U+{:04X} | {n} | {med:.2} | {lo:.2}–{hi:.2} |",
            c as u32
        );
    }

    let derived = derive(&all);
    println!("\n## 3. Width table derived from the data (half-consonant steps)\n");
    println!(
        "Medians across all fonts, rounded to the nearest 0.5. Codepoints not listed: base = 1, Mn = 0, subscript = 0.\n"
    );
    println!("| Codepoint | Width |\n|---|---|");
    for (c, w) in derived.base.iter().filter(|(_, w)| **w != 1.0) {
        println!("| {c} U+{:04X} (base) | {w} |", *c as u32);
    }
    for (c, w) in &derived.spacing {
        println!("| ◌{c} U+{:04X} | {w} |", *c as u32);
    }
    for (c, w) in derived.subscript.iter().filter(|(_, w)| **w != 0.0) {
        println!("| ◌្{c} coeng + U+{:04X} | {w} |", *c as u32);
    }

    println!("\n## 4. Candidate rules, evaluated syllable by syllable\n");
    let mut unique: Vec<&String> = syllables.iter().collect();
    unique.sort();
    unique.dedup();
    println!(
        "Sample: {} syllables ({} distinct) from the test corpus and common sentences.\n",
        syllables.len(),
        unique.len()
    );
    println!(
        "For each syllable and font, **fit = measured advance ÷ rule width**. A terminal can scale its \
         Khmer font once, globally, so only the *variation* in fit matters: it is what shows up as \
         syllables squeezed together or gaps between them. The global scale is the median fit; \
         *Good* is the share of syllables whose fit is within ±20% of it.\n"
    );
    for (label, set) in [("All fonts", &all), ("Common system/UI fonts", &common)] {
        if set.is_empty() {
            continue;
        }
        println!("### {label} ({} fonts)\n", set.len());
        println!(
            "| Rule | Global scale | Good (±20%) | Squeezed (< −20%) | Gap (> +20%) | Cells per sample |\n\
             |---|---|---|---|---|---|"
        );
        for rule in rules(&derived) {
            let widths: Vec<f32> = syllables.iter().map(|s| (rule.width)(s)).collect();
            let mut fits = Vec::new();
            for m in set.iter() {
                for (i, w) in widths.iter().enumerate() {
                    if *w > 0.0 {
                        fits.push(m.syllables[i] / w);
                    }
                }
            }
            let scale = percentile(&mut fits, 0.5);
            let n = fits.len() as f32;
            let share = |f: &dyn Fn(f32) -> bool| {
                100.0 * fits.iter().filter(|x| f(**x / scale)).count() as f32 / n
            };
            println!(
                "| {} | {scale:.2} | {:.0}% | {:.0}% | {:.0}% | {} |",
                rule.name,
                share(&|r| (0.8..=1.2).contains(&r)),
                share(&|r| r > 1.2),
                share(&|r| r < 0.8),
                widths.iter().sum::<f32>()
            );
        }
        println!();
    }
    println!(
        "*Squeezed* means the syllable needs more space than the rule gives it, so it is drawn narrower \
         or overlaps; *Gap* means it gets more space than it needs.\n"
    );

    println!("## 5. Fonts measured\n");
    let names: Vec<String> = all
        .iter()
        .map(|m| {
            if COMMON_FONTS.contains(&m.name.as_str()) {
                format!("**{}**", m.name)
            } else {
                m.name.clone()
            }
        })
        .collect();
    println!("{}", names.join(", "));
    println!("\nCommon system/UI fonts are in bold.");
}
