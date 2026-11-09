# Architecture

How the KTE crates fit together, and how text flows from an application to pixels.

## Crates

```
                 kte-core  (facade: the API most users import)
                    │
   ┌────────────┬───┴────────┬──────────────┐
kte-terminal  kte-shaper   kte-cell      kte-unicode
 VT parser,    harfrust     layout,       classes, clusters,
 grid, cursor, shaping,     wrap, fit     half-unit widths
 selection     font size        │              ▲
   │    │          │            └──────────────┤
   │    └──────────┼───────────────────────────┤
   └─ kte-cell ────┘                           │
                                     (everything depends on it)

kte-capi ── C ABI over kte-core (native terminals; WebAssembly)
kte-term ── the standalone terminal app (Phase 2)
```

| Crate | Depends on | Responsibility |
|-------|-----------|----------------|
| `kte-unicode` | `unicode-segmentation`, `unicode-width` | The spec: what a cluster is and how wide it is. No fonts. |
| `kte-cell` | `kte-unicode` | Grid geometry: columns, wrapping, fitting glyphs into cells. |
| `kte-shaper` | `kte-unicode`, `kte-cell`, `harfrust` | Turns Khmer text into positioned glyphs per cluster. |
| `kte-terminal` | `kte-unicode`, `kte-cell`, `vte` | Terminal state: what is in each cell, where the cursor is. |
| `kte-core` | all of the above | Stable re-exports. |
| `kte-capi` | `kte-core` | `extern "C"` functions; the only crate with `unsafe`. |

## Data flow in a terminal

```
application ──bytes──▶ PTY ──▶ Terminal::feed ──▶ grid of cells
                                    │                  │
             replies (DSR, DECRQM) ◀┘                  │ per row: clusters + widths
                                                       ▼
                               Shaper::shape_clusters (Khmer runs)
                                                       │ glyphs per cluster
                                                       ▼
                                    kte_cell::fit ──▶ renderer (GPU)
```

1. **Parse.** `Terminal::feed` runs the byte stream through `vte`. Printable characters go to `print_char`.
2. **Cluster.** A new codepoint either extends the cluster just printed (when the combined text is still one KTE cluster) or starts a new one. Extending can change the cluster's width; the cluster is re-placed, and moves to the next line if it no longer fits.
3. **Store.** A cluster `N` cells wide occupies one cell holding its text, followed by `N - 1` continuation cells. Overwriting any part of a cluster erases all of it, so the grid never holds half a syllable.
4. **Shape.** The renderer groups each row into runs per font, shapes Khmer runs with `Shaper::shape_clusters`, and gets glyphs grouped by cluster.
5. **Fit.** `kte_cell::fit` centers or compresses each cluster's glyphs within its cells. Glyphs are not clipped vertically.
6. **Select.** `Terminal::selection_text` snaps to whole clusters and returns the stored text, so copy/paste preserves the original codepoints.

## Testing layers

| Layer | What it checks |
|-------|----------------|
| `tools/spec-ref` + `tests/corpus` | An independent Python implementation of the spec generates expected clusters and widths. |
| `kte-unicode/tests/corpus.rs` | The Rust implementation matches every corpus case. |
| Unit tests in each crate | Segmentation, widths, wrapping, terminal behavior, C ABI. |
| `kte-shaper` tests | Spec widths track real advances in an installed Khmer font. |
| `examples/c/demo.c` | The C header and static library link and work from C. |
| `tests/wasm/smoke.mjs` | The WebAssembly build works from JavaScript. |
