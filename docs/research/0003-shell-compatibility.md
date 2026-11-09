# Research 0003: Shell compatibility

- **Date:** 2026-11-09
- **Phase:** 2 (standalone terminal)
- **Tool:** `kte-term --snapshot --command … --type …`, which runs a real shell on a pty, types into it, and renders the result
- **Question:** Can today's shells edit Khmer in a terminal that uses the KTE Width Spec?

## Findings

### 1. Shells measure with the system `wcwidth()`, and it differs by platform

| Codepoint | macOS 26 (Apple libc) | Linux (glibc 2.41) |
|---|---|---|
| Consonant (ក ស) | 1 | 1 |
| Spacing vowel (ា េ ោ ះ) | **0** | **1** |
| Non-spacing vowel (ិ ុ ំ ់) | 0 | 0 |
| COENG ្ | 0 | 0 |
| Khmer digit, ។ | 1 | 1 |

So `កោ` is 1 column to zsh on macOS and 2 on Linux, while the Wide model gives it 4 cells.

### 2. Shells place every character with width > 0 in a cell of its own

zsh's line editor redraws one cell at a time. When ី is typed after ស្ត, zsh moves back one column and reprints "តី" as its own cell. A terminal that has merged ស្ត into one cluster erases the whole cluster when it is partly overwritten, so ស disappears.

### 3. With the Wide model, line editing breaks

Typed Khmer shows gaps and missing letters, because zsh and the terminal disagree about columns. With the Legacy model and cell joining below, the same input renders correctly ([image](../images/live-zsh.png)).

### 4. zsh hides Khmer signs unless `COMBINING_CHARS` is set

By default zsh displays combining characters as `<17d2>` `<17bb>`. With `setopt COMBINING_CHARS` it prints them normally. The option is off in a default zsh and in the stock macOS configuration.

### 5. bash works

macOS's bash 3.2 (readline) edits Khmer correctly under the Legacy model with no configuration.

## Decisions

- Added the **Legacy model** to the spec (v0.2.0, §6.2), in glibc and Apple variants: per-codepoint `wcwidth()` widths, and one cell per character wider than zero.
- In the Legacy model the terminal stores cells exactly as the shell sees them, and **joins cells back into syllables only when rendering**. Storage follows the application; rendering follows Khmer.
- `kte-term` defaults to Legacy so shells work today. The Wide model remains the spec's target, for applications that adopt it (Phase 5).

## Follow-up (Phase 5)

- zsh: propose turning `COMBINING_CHARS` on by default, or documenting it for Khmer users.
- Shells and line editors: adopt KTE widths (or at least cluster-aware widths), so the Wide model can be used.
- Measure Windows console applications' widths.
