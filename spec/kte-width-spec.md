# KTE Width Spec

| | |
|---|---|
| **Version** | 0.1-draft |
| **Status** | Draft. Not stable; do not implement in other projects yet. |
| **Editors** | Den TOUCH |
| **Discussion** | Issues labelled [`spec`](https://github.com/touchde/khmer-terminal-engine/labels/spec) |

## 1. Purpose

This spec defines how many terminal cells a sequence of Khmer text occupies. Terminals and applications that follow it agree on cursor positions, line wrapping and redraw regions for Khmer text.

## 2. Design constraints

1. **Codepoints only.** Width is computed from the codepoint sequence alone. It must not depend on fonts, shaping results or rendering, because applications can't see the font.
2. **Whole cells.** Every cluster occupies an integer number of cells, ≥ 0.
3. **Stable.** Once v1.0 is published, widths for already-assigned codepoints only change with a major version.
4. **Lossless.** Width calculation never modifies the text.
5. **Compatible.** Non-Khmer text keeps the widths defined by Unicode East Asian Width and established `wcwidth` practice.

## 3. Terminology

The key words MUST, SHOULD and MAY are used as described in RFC 2119.

- **Cluster:** an extended grapheme cluster (UAX #29), extended with the Khmer syllable rules in §4.
- **Coeng:** U+17D2 KHMER SIGN COENG, which turns the next consonant into a subscript.
- **Spacing vowel:** a Khmer dependent vowel with General_Category Mc.

## 4. Segmentation

> **TODO (Phase 0):** Define Khmer syllable boundaries. Specify how they relate to UAX #29 extended grapheme clusters. In particular, check whether each Unicode version keeps coeng + consonant in one cluster or splits it, and define the KTE behaviour either way.

## 5. Width rule

> **Draft rule. Values marked open are undecided.**

| Component | Cells |
|-----------|-------|
| Base consonant (U+1780–U+17A2) or independent vowel (U+17A3–U+17B3) | 1 |
| Coeng (U+17D2) + following consonant | 0 |
| Non-spacing marks (Mn) | 0 |
| Spacing vowels (Mc) | **open: 0 or 1** |
| Khmer digits, punctuation and symbols (U+17D4–U+17DC, U+17E0–U+17E9, U+19E0–U+19FF) | 1 |
| ZWSP (U+200B), ZWJ, ZWNJ | 0 |

The width of a cluster is the sum of its components' widths. A cluster with only zero-width components that starts a line (no base) SHOULD take 1 cell, the same as an isolated combining mark.

### 5.1 Open questions

1. **Spacing vowels.** Count them as 1 or 0? Decide from measurements of common fonts (Noto Sans Khmer, Noto Serif Khmer, Kantumruy Pro, Khmer OS) across the corpus. Tracked in [ADR 0002](../docs/adr/0002-width-from-codepoints.md).
2. **Coeng-RO** (្រ) is drawn before the base. Does it need extra width?
3. **Detection.** How do applications find out that a terminal follows this spec: mode 2027, a new DEC private mode, or XTVERSION/terminfo?

## 6. Rendering guidance (informative)

Terminals SHOULD shape each cluster as a whole (e.g. with HarfBuzz) and fit the result into the cluster's cells. They SHOULD NOT clip glyphs to the cell box vertically, because subscripts and above-vowels extend past normal line height.

## 7. Conformance

A conforming implementation passes every test in [`tests/corpus/`](../tests/corpus/) marked `"spec": "width"`.
