# KTE Strategy: Standalone Terminal and Upstream Support

This document covers the plan for Khmer Terminal Engine (KTE). It has two goals:

1. Build an **independent terminal** with full Khmer support. It also serves as the reference implementation.
2. Get Khmer support **upstream**: xterm.js, VS Code, and the native terminals on Windows, macOS and Linux.

---

## 1. The core problem: apps and the terminal must agree on width

Drawing Khmer is the easy part, since HarfBuzz already shapes it correctly. The hard part is the **terminal cell grid**.

A terminal is a grid of cells. Both sides of the PTY count cells independently:

- **Apps** (bash/readline, zsh, vim, Python, tmux) count cells with `wcwidth()` per codepoint.
- **The terminal** has its own width table, and places the cursor using it.

If the two counts differ by even one cell, cursor movement, backspace, line wrapping and redraws break. This causes the familiar "gaps" and cursor drift.

Khmer breaks per-codepoint `wcwidth` badly:

| Text | Codepoints | Typical `wcwidth` sum | Visual width |
|------|------------|-----------------------|--------------|
| ស្ត | ស + ្ (coeng, Mn = 0) + ត (Lo = 1) | 2 | ~1 (ត is a subscript) |
| កោ | ក + ោ (Mc = 1) | 2 | ~2 (the vowel wraps around ក) |
| កេ | ក + េ (Mc = 1) | 2 | ~2 (េ is drawn on the left) |
| ក្រ | ក + ្ + រ | 2 | ~1.3 (coeng-RO sits left/below) |

So a terminal can't fix Khmer on its own. A perfect terminal still breaks if zsh thinks `ស្ត` is 2 cells.
**The most important thing KTE produces is a width specification, not a renderer.**

---

## 2. Strategy: spec first, then implementations

```
            KTE Width Spec  ("how many cells is a Khmer cluster?")
                   │
   ┌───────────────┼────────────────────────┬──────────────────────────┐
 KTE core crates   Standalone KTE terminal   Upstream terminals         App-side libraries
 (Rust, C ABI,     (reference impl)          xterm.js → VS Code,        wcwidth (Python), unicode-width (Rust),
  WASM)                                      WezTerm, Ghostty, Windows  string-width (Node), go-runewidth,
                                             Terminal, VTE, Kitty,      readline / zsh / fish, tmux
                                             iTerm2
```

### 2.1 Width rule (draft v0)

The rule must be **deterministic from codepoints alone**. It must never depend on font measurements, because apps can't see the font.

Applied per Khmer orthographic syllable:

| Component | Cells |
|-----------|-------|
| Base consonant or independent vowel | 1 |
| Coeng (U+17D2) + consonant (subscript) | 0 |
| Non-spacing marks (Mn) | 0 |
| Spacing vowels (ា, ោ, េ, ែ, ៃ, …) | **open question: 1 or 0** |

The terminal shapes the whole syllable with HarfBuzz and fits the result into those N cells, scaling or overflowing as needed.

**Open question:** settle the spacing-vowel rule with data. Render the test corpus through HarfBuzz with common Khmer fonts (Noto Sans Khmer, Kantumruy Pro, Khmer OS) and compare real advances to the cell width.

### 2.2 Existing work to build on

- **Mode 2027** (grapheme-cluster mode, proposed by Contour, adopted by Ghostty and others). The terminal tells apps "I measure by cluster." Khmer syllable width can ride on this mechanism.
- **Windows Terminal** has a grapheme-cluster text-measurement mode.
- **xterm.js** has a pluggable `terminal.unicode` provider API, plus the unicode11 and graphemes addons.
- **Kitty text-sizing protocol**: lets an app state widths explicitly. A possible escape hatch.
- **ucs-detect** (Jeff Quast): tests terminals on UDHR language samples, Khmer included. Use it as before/after evidence in every upstream issue and PR.

---

## 3. Standalone KTE terminal (reference implementation)

### 3.1 Stack (Rust, cross-platform)

| Layer | Choice |
|-------|--------|
| VT/ANSI parser | `vte` crate, or our own |
| Grid | Cells hold grapheme clusters; a cluster can span N cells (per the spec) |
| Segmentation | `unicode-segmentation` + Khmer syllable rules (`kte-unicode`) |
| Shaping | `harfrust` (the HarfBuzz project's Rust port; `rustybuzz` is unmaintained), or C HarfBuzz via FFI; shape whole runs, then map clusters to cells (`kte-shaper`, `kte-cell`) |
| Fonts | Fallback chain: Latin monospace + Noto Sans Khmer / Kantumruy Pro (true monospace Khmer fonts barely exist) |
| Rendering | `wgpu` with a glyph atlas keyed by **shaped cluster**, not codepoint |
| Window | `winit` |
| PTY | `portable-pty` (covers ConPTY on Windows) |

### 3.2 Khmer-specific rendering issues

- **Vertical overflow.** Subscripts and above-vowels go past normal line height. Use a taller line height (1.3–1.5), or let glyphs draw into neighboring rows instead of clipping them to the cell. Clipping to the cell is why GPU terminals cut off subscripts.
- **Pre-base reordering** (េ ែ ៃ, coeng-RO). The cursor and selection follow logical order while the glyphs are drawn reordered.
- **Cursor, backspace and selection** move by whole syllable. Whether backspace deletes the whole cluster or only the last mark is a policy decision to document.
- **Copy/paste** must return the exact original codepoints (core principle: preserve the original Unicode text).

---

## 4. Upstream contribution map

| Target | License / language | Path | Difficulty |
|--------|--------------------|------|------------|
| **xterm.js** | MIT, TypeScript | 1) Khmer unicode-provider addon (easy to land). 2) The WebGL renderer rasterizes glyphs per cell, which breaks shaping. The real fix is cluster-level rasterization, a large PR that needs maintainer buy-in first. | Medium → High |
| **VS Code** | MIT, TypeScript | Uses xterm.js (WebGL by default). Mostly free once xterm.js lands, plus a setting / unicode-version option. | Low (after xterm.js) |
| **WezTerm** | MIT, Rust | Already uses HarfBuzz; can reuse KTE crates directly. **Best first native target.** | Medium |
| **Ghostty** | MIT, Zig | Supports mode 2027. Contribute the Khmer width rule plus a fix for any renderer clipping. | Medium |
| **Windows Terminal** | MIT, C++ | Grapheme mode + DirectWrite shaping. File issues with ucs-detect evidence, then PR the width table. | Medium |
| **VTE** (GNOME Terminal, etc.) | LGPL, C/C++ | Width tables + Pango/HarfBuzz rendering. Slow review cycle. | High |
| **Kitty** | GPL, C/Python | HarfBuzz shaping + text-sizing protocol. | Medium |
| **iTerm2** (macOS) | GPL, ObjC/Swift | Core Text. Open to PRs. | Medium |
| **Terminal.app / conhost** | Closed source | No PRs possible; feedback reports only. | — |
| **Alacritty** | Apache-2.0, Rust | Maintainers have long declined text shaping. Low priority. | — |

### 4.1 How to get PRs merged

1. Open an **issue first** with screenshots and ucs-detect numbers.
2. Link the **KTE Width Spec** so the change is backed by a written standard, not just "our project's approach."
3. Keep PRs **small** and **behind an option** at the start.
4. Engage maintainers before building large renderer changes.

### 4.2 App-side libraries (required for end-to-end correctness)

Upstream terminal work only fully works once apps count widths the same way:

- Python `wcwidth`
- Rust `unicode-width`
- Node `string-width`
- Go `go-runewidth`
- Shells: readline (bash), zsh, fish
- **tmux**: high priority, because it re-measures and re-renders everything itself

---

## 5. Phases

| Phase | Deliverable |
|-------|-------------|
| **0. Spec + corpus** | KTE Width Spec v0, conformance test suite, data-driven decision on spacing vowels |
| **1. Core crates** | `kte-unicode`, `kte-shaper`, `kte-cell`, `kte-terminal`, `kte-core`, exposed via Rust, C ABI, WASM |
| **2. Standalone terminal MVP** | macOS first (development machine); wgpu keeps it portable to Windows and Linux |
| **3. xterm.js + VS Code** | Unicode provider addon, then renderer work, then VS Code integration |
| **4. Native terminals** | WezTerm → Ghostty → Windows Terminal → VTE / Kitty / iTerm2 |
| **5. Apps + shells + tmux** | Width-table PRs to libraries, shells and tmux |
