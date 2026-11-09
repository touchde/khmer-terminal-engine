# 0002. Cell width is computed from codepoints only

- **Status:** Accepted (the value for spacing vowels is still open)
- **Date:** 2026-11-09

## Context

Applications (shells, editors, tmux) and the terminal each count cells independently. Applications can't see which font the terminal uses. Khmer glyph advances vary from font to font and are not whole multiples of the cell width.

## Decision

The width of Khmer text is a whole number of cells, computed only from its codepoints by the rule in the [KTE Width Spec](../../spec/kte-width-spec.md). Terminals fit the shaped glyphs into those cells.

Font measurements are used only to *choose* the rule (in Phase 0), never at runtime.

## Consequences

- Applications and terminals can agree without exchanging font information.
- Rendering is not pixel-identical to proportional layout; spacing between syllables is snapped to the grid.
- A proportional "reading mode" for plain output remains possible as a terminal-only feature, outside the spec.

## Open

Whether spacing vowels (Mc) count as 0 or 1 cell will be decided from font measurements over the corpus.
