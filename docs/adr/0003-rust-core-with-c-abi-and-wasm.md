# 0003. Rust core with C ABI and WebAssembly bindings

- **Status:** Accepted
- **Date:** 2026-11-09

## Context

Target projects use many languages: TypeScript (xterm.js, VS Code), Rust (WezTerm), Zig (Ghostty), C++ (Windows Terminal, VTE, Konsole), C/Python (Kitty), Objective-C/Swift (iTerm2).

## Decision

Implement the core in Rust. Expose it through a C ABI (`kte-capi`, with a generated `kte.h`) for native terminals and through WebAssembly for xterm.js.

The width rule is also published as plain data tables, so projects that won't take a dependency can port it directly.

## Consequences

- One implementation, tested once, used everywhere.
- Some upstream projects will prefer a native port of the tables over a Rust dependency; the spec and corpus make that possible.
