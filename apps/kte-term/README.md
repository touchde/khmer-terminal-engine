# kte-term

The standalone KTE terminal emulator and reference implementation of the
[KTE Width Spec](../../spec/kte-width-spec.md). **Status: not yet implemented.**

Planned stack: `winit` (window), `wgpu` (rendering), HarfBuzz (`harfrust`) via `kte-shaper`,
`portable-pty` (PTY, including ConPTY on Windows). See [docs/strategy.md](../../docs/strategy.md).
