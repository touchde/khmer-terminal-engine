# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Project structure: Cargo workspace, crate skeletons, standalone terminal stub.
- Strategy document, KTE Width Spec draft, initial ADRs, upstream tracker.
- Community files: license, contributing guide, code of conduct, security policy, governance.
- CI for Linux, macOS and Windows.
- `tools/kte-measure`: measures shaped Khmer advances across fonts.
- Research 0001: Khmer advance measurements across 160 fonts.
- KTE Width Spec 0.1.0: half-unit tables, Wide and Compact cell models, coeng-joining segmentation (ADR 0004).
- `kte-unicode`: classification, segmentation, widths; corpus conformance test.
- `kte-cell`: grid layout, wrapping, glyph fitting.
- `kte-shaper`: HarfBuzz shaping via `harfrust`, cluster grouping, font sizing.
- `kte-terminal`: VT/ANSI terminal model with multi-cell clusters, selection, scrollback, alternate screen, DSR and DECRQM (mode 2027) replies.
- `kte-core` facade; `kte-capi` C ABI with `include/kte.h`, C demo and WebAssembly smoke test.
- `tools/spec-ref`: independent Python implementation that generates corpus expectations.
- Test corpus expanded to 34 cases.
- KTE Width Spec 0.2.0: Legacy model (glibc and Apple variants) for compatibility with `wcwidth()`-based applications.
- `kte-term` MVP: window, shell on a pty, CPU rendering with HarfBuzz-shaped Khmer, keyboard and IME input, mouse selection, copy and paste (bracketed paste), `--snapshot` mode with live shell typing, macOS app bundle script.
- `kte-terminal`: application cursor keys (mode 1) and bracketed paste (mode 2004); Legacy-model cell segmentation.
- `kte-cell`: `fit_with_stretch`.
- `kte-term`: Khmer words keep one natural size ("grid in logic, flow in pixels" layout) instead of being squeezed syllable by syllable; larger default Khmer size in Legacy mode; glyphs stay visible under the block cursor; `--dump`.
- Research 0003 (shell compatibility); ADR 0005 (kte-term architecture).
- CI: kte-term snapshots on Linux, macOS and Windows, including typing Khmer into bash.
- Research 0002: the spec table fits Linux default fonts (98–100%) and Windows' Leelawadee UI (80–86%); `Measure fonts` workflow.
