# Khmer Terminal Engine (KTE)

[![CI](https://github.com/touchde/khmer-terminal-engine/actions/workflows/ci.yml/badge.svg)](https://github.com/touchde/khmer-terminal-engine/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Make Khmer Unicode work correctly everywhere people use terminals.**

```text
សួស្តី ពិភពលោក!
ការអភិវឌ្ឍកម្មវិធីជាភាសាខ្មែរ
```

> **Status: pre-alpha.** The project structure is in place. Phase 0 (the width spec and test corpus) is in progress. Nothing is usable yet.

## The problem

Terminals lay text out on a grid of cells. Applications such as shells, editors and `tmux` count cells per codepoint with `wcwidth()`. The terminal keeps its own count. Khmer subscripts (coeng), pre-base vowels and split vowels make those two counts disagree. The result is gaps between letters, clipped subscripts, a drifting cursor and broken backspace.

You can't fix this inside one terminal. KTE's approach:

1. **A written standard.** The [KTE Width Spec](spec/kte-width-spec.md) defines how many cells a Khmer cluster occupies, using codepoints only.
2. **Reusable libraries** that implement the spec: Rust crates, a C ABI and WebAssembly.
3. **A standalone terminal** (`kte-term`) that serves as the reference implementation.
4. **Upstream contributions** to xterm.js, VS Code, WezTerm, Ghostty, Windows Terminal, VTE, Kitty, iTerm2 and to width libraries, shells and tmux.

See [docs/strategy.md](docs/strategy.md) for the full analysis and roadmap.

## Repository layout

| Path | Contents |
|------|----------|
| `spec/` | KTE Width Spec, the normative standard |
| `crates/kte-unicode` | Khmer classification, syllable segmentation, width rule |
| `crates/kte-shaper` | HarfBuzz shaping and cluster metrics |
| `crates/kte-cell` | Mapping shaped clusters onto terminal cells |
| `crates/kte-terminal` | VT/ANSI parser, grid, cursor, selection, wrapping |
| `crates/kte-core` | Stable facade API |
| `crates/kte-capi` | C ABI for native terminals |
| `apps/kte-term` | Standalone terminal emulator |
| `integrations/xtermjs` | xterm.js addon (TypeScript) |
| `tests/corpus` | Khmer test corpus |
| `docs/` | Strategy, architecture decisions (ADRs), upstream tracker |

## Building

Requires a stable Rust toolchain (MSRV 1.85).

```sh
cargo build
cargo test
cargo run -p kte-term
```

## Roadmap

| Phase | Deliverable |
|-------|-------------|
| 0 | KTE Width Spec v0 + conformance corpus |
| 1 | Core crates (Rust, C ABI, WASM) |
| 2 | `kte-term` MVP (macOS first, then Windows and Linux) |
| 3 | xterm.js addon → VS Code |
| 4 | Native terminals (WezTerm → Ghostty → Windows Terminal → others) |
| 5 | Width libraries, shells, tmux |

Progress on each upstream project is tracked in [docs/upstream](docs/upstream/README.md).

## Contributing

Contributions are welcome, especially from native Khmer readers who can judge rendering quality. Read [CONTRIBUTING.md](CONTRIBUTING.md) first. This project follows the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

[MIT](LICENSE)
