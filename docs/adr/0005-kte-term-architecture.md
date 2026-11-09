# 0005. kte-term: CPU rendering and the Legacy model by default

- **Status:** Accepted
- **Date:** 2026-11-09

## Context

Phase 2 builds the standalone terminal. Two choices shape it:

1. **Rendering.** The strategy planned a GPU renderer (`wgpu`). A GPU pipeline (glyph atlas, shaders) is a large amount of work before the first Khmer character appears, and a terminal grid is small enough to draw on the CPU at interactive speeds.
2. **Default cell model.** [Research 0003](../research/0003-shell-compatibility.md) showed that today's shells measure text with the system `wcwidth()`. Under the Wide model their line editing breaks.

## Decision

- Render on the CPU: glyph outlines from `read-fonts` (via `harfrust`), rasterized with `tiny-skia`, cached per glyph and size, and presented with `softbuffer`. Draw backgrounds first and glyphs second, so Khmer marks that extend past a row are never covered.
- Keep a headless `--snapshot` mode that renders to PNG, optionally running a real shell and typing into it. It is used for development and in CI on all three platforms.
- Default `kte-term` to the **Legacy** model for the platform (Apple or glibc rules). Offer `--model wide` for the spec layout.
- **Grid in logic, flow in pixels.** Under Legacy, applications give Khmer fewer cells than it needs (on macOS, spacing vowels get none). Fitting each syllable into its own cells made letters alternate between big and small. Instead, each Khmer word is drawn at its natural width with one font size; when it needs more room than its cells, the rest of that line shifts right, and cells the application never wrote absorb the shift. A line that would run past the window has its Khmer compressed by one shared factor. Cursor, selection and mouse hit-testing follow the same pixel layout.

## Consequences

- Khmer works in today's shells immediately, at a consistent natural size.
- Columns to the right of Khmer text can shift in Legacy mode, which matters for tables and full-screen apps.
- The Wide model's better layout needs application support (Phase 5).
- A GPU renderer can replace the CPU one later behind the same `Renderer` interface if performance requires it.
