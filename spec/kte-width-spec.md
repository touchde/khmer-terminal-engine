# KTE Width Spec

| | |
|---|---|
| **Version** | 0.1.0 |
| **Status** | Draft. Values may change before 1.0; implementations should state the version they follow. |
| **Editors** | Den TOUCH |
| **Discussion** | Issues labelled [`spec`](https://github.com/touchde/khmer-terminal-engine/labels/spec) |
| **Reference implementation** | [`kte-unicode`](../crates/kte-unicode) |
| **Evidence** | [Research 0001](../docs/research/0001-khmer-advance-measurements.md), [Research 0002](../docs/research/0002-windows-and-linux-fonts.md), [ADR 0004](../docs/adr/0004-half-consonant-cell-model.md) |

## 1. Purpose

This spec defines how text is split into clusters and how many terminal cells each Khmer cluster occupies. Terminals and applications that follow it agree on cursor positions, line wrapping and redraw regions for Khmer text.

## 2. Design constraints

1. **Codepoints only.** Width is computed from the codepoint sequence alone. It never depends on fonts, shaping results or rendering, because applications can't see the font.
2. **Whole cells.** Every cluster occupies an integer number of cells, ≥ 0.
3. **Stable.** From version 1.0, widths of assigned codepoints only change with a major version.
4. **Lossless.** Segmentation and width calculation never modify the text.
5. **Compatible.** Non-Khmer text keeps its usual terminal width (UAX #11 East Asian Width, as in common `wcwidth` implementations).

## 3. Terminology

The key words MUST, SHOULD and MAY are used as described in RFC 2119.

- **Cluster:** the unit stored in terminal cells and moved over by the cursor (§4).
- **Khmer cluster:** a cluster whose first character is in the Khmer block (U+1780–U+17FF) or the Khmer Symbols block (U+19E0–U+19FF).
- **Coeng:** U+17D2 KHMER SIGN COENG. Coeng + consonant is a **subscript**.
- **Base:** a consonant, independent vowel, digit, punctuation mark or symbol (§5.2).
- **Half-unit (h):** half the advance of a typical Khmer consonant. All Khmer widths are whole numbers of half-units.

## 4. Segmentation

1. Split the text into extended grapheme clusters (UAX #29).
2. While a cluster ends with COENG and the next cluster starts with a Khmer consonant (U+1780–U+17A2), join the two.

Rule 2 keeps a consonant and its subscripts in one cluster (e.g. `ស្ត្រី`) whichever Unicode version's grapheme rules are in use.

Concatenating the clusters MUST give back the original text.

## 5. Width of a Khmer cluster

### 5.1 Half-units

The width of a Khmer cluster in half-units is the sum of:

| Component | Half-units |
|---|---|
| Each base (§5.2) | from the base table |
| Each spacing sign (§5.3) | 1, or 2 for two-part vowels |
| Each subscript, COENG + consonant (§5.4) | 0, or 1 for side-drawn subscripts |
| COENG not followed by a consonant | 0 |
| Non-spacing signs (Mn): U+17B7–U+17BD, U+17C6, U+17C9–U+17D1, U+17D3, U+17DD | 0 |
| Inherent vowels U+17B4, U+17B5 | 0 |
| ZWSP, ZWNJ, ZWJ (U+200B–U+200D) | 0 |
| Any other character in the cluster | 2 × its UAX #11 width |

If the cluster's first character is not a base (a sign with nothing to attach to), add **2** for the placeholder (dotted circle) that renderers draw.

### 5.2 Base table

| Characters | Half-units |
|---|---|
| រ U+179A, វ U+179C | 1 |
| ៖ U+17D6, ៛ U+17DB | 1 |
| ឃ U+1783, ញ U+1789, ឍ U+178D, យ U+1799, ល U+179B, ស U+179F, ហ U+17A0, ឡ U+17A1, ឦ U+17A6 | 3 |
| ឈ U+1788, ណ U+178E | 4 |
| ៚ U+17DA | 4 |
| Lunar symbols U+19EA–U+19EF, U+19FA–U+19FF | 4 |
| ៘ U+17D8 | 6 |
| All other consonants (U+1780–U+17A2), independent vowels (U+17A3–U+17B3), punctuation (U+17D4–U+17DC), digits (U+17E0–U+17E9), numeric symbols (U+17F0–U+17F9) and lunar symbols (U+19E0–U+19FF) | 2 |

Khmer digits are all 2 half-units, even though ២ ៣ ៧ are slightly wider in most fonts, so that numbers line up in columns.

### 5.3 Spacing signs (Mc)

| Characters | Half-units |
|---|---|
| ឿ U+17BF, ៀ U+17C0, ោ U+17C4, ៅ U+17C5 (two-part vowels) | 2 |
| ា U+17B6, ើ U+17BE, េ U+17C1, ែ U+17C2, ៃ U+17C3, ះ U+17C7, ៈ U+17C8 | 1 |

### 5.4 Subscripts

| Subscript consonant (after COENG) | Half-units |
|---|---|
| ឃ U+1783, ឈ U+1788, ឍ U+178D, ប U+1794, យ U+1799, រ U+179A, ឞ U+179E, ស U+179F | 1 |
| All other consonants | 0 |

## 6. Cell models

Half-units are converted to cells by a **cell model**. Implementations MUST support the Wide model and MAY support the Compact model.

| Model | Cells for a Khmer cluster | Typical consonant |
|---|---|---|
| **Wide** (default) | *h* | 2 cells |
| **Compact** | ⌈*h* ÷ 2⌉ | 1 cell |

The Wide model matches measured font advances for 96% of syllables across 160 fonts (100% for common system fonts). The Compact model matches about 60–70%; the rest show half-cell gaps. See ADR 0004.

Non-Khmer clusters have the same width in both models: their UAX #11 width.

### 6.1 Examples

| Text | Clusters | Half-units | Wide | Compact |
|---|---|---|---|---|
| ក | ក | 2 | 2 | 1 |
| កា | កា | 2 + 1 = 3 | 3 | 2 |
| កោះ | កោះ | 2 + 2 + 1 = 5 | 5 | 3 |
| ស្ត្រី | ស្ត្រី | 3 + 0 + 1 + 0 = 4 | 4 | 2 |
| សួស្តី | សួ · ស្តី | 3 · 3 | 6 | 4 |
| ok ក | o · k · ␠ · ក | — · — · — · 2 | 5 | 4 |

## 7. Rendering guidance (informative)

- Shape each run with a complete OpenType shaper (e.g. HarfBuzz) and place each cluster's glyphs inside its cells. Center a cluster that is narrower than its cells; compress one that is wider.
- Choose the Khmer font size so that one typical consonant spans 2 cells (Wide) or 1 cell (Compact), or slightly less to leave even spacing.
- Do not clip glyphs vertically to the cell box: subscripts and above-vowels extend past normal line height.

## 8. Detection (open)

How applications discover that a terminal follows this spec, and which model it uses, is not yet defined. Candidates: a DEC private mode in the style of mode 2027, XTVERSION, or terminfo capabilities.

## 9. Conformance

A conforming implementation produces the clusters and widths listed in [`tests/corpus/khmer_corpus.json`](../tests/corpus/khmer_corpus.json) for every case. Expected values are generated by an independent reference implementation ([`tools/spec-ref`](../tools/spec-ref)) and checked against `kte-unicode` in CI.

## Changelog

- **0.1.0** (2026-11-09): First complete draft. Half-unit tables from Research 0001; Wide and Compact cell models; coeng joining in segmentation.
