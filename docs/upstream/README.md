# Upstream tracker

Status of Khmer support work in other projects. Update this table when an issue or PR is opened or changes state.

Always attach ucs-detect results and screenshots to upstream issues, and link the [KTE Width Spec](../../spec/kte-width-spec.md).

## Terminals

| Project | Language | License | Approach | Issue | PR | Status |
|---------|----------|---------|----------|-------|----|--------|
| xterm.js | TypeScript | MIT | Unicode provider addon; then cluster-level WebGL rendering | — | — | Not started |
| VS Code | TypeScript | MIT | Follows xterm.js; expose setting | — | — | Blocked on xterm.js |
| WezTerm | Rust | MIT | Width rule via KTE crates | — | — | Not started |
| Ghostty | Zig | MIT | Width rule under mode 2027; fix glyph clipping | — | — | Not started |
| Windows Terminal | C++ | MIT | Width table in grapheme mode | — | — | Not started |
| VTE (GNOME Terminal) | C/C++ | LGPL | Width table + rendering | — | — | Not started |
| Kitty | C/Python | GPL-3.0 | Width rule; text-sizing protocol | — | — | Not started |
| iTerm2 | Obj-C/Swift | GPL-2.0 | Width rule + Core Text rendering | — | — | Not started |
| Konsole | C++ | GPL | Width table | — | — | Not started |
| Terminal.app / conhost | — | Closed | Feedback reports only | — | n/a | — |

## Width libraries, shells and multiplexers

| Project | Language | Issue | PR | Status |
|---------|----------|-------|----|--------|
| wcwidth (Python) | Python | — | — | Not started |
| unicode-width | Rust | — | — | Not started |
| string-width | JavaScript | — | — | Not started |
| go-runewidth | Go | — | — | Not started |
| tmux | C | — | — | Not started |
| zsh | C | — | — | Not started |
| bash / readline | C | — | — | Not started |
| fish | Rust | — | — | Not started |

## Baseline measurements

| Terminal | Version | OS | ucs-detect Khmer result | Date |
|----------|---------|----|-------------------------|------|
| | | | | |
