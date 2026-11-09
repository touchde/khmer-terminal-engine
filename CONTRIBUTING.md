# Contributing to Khmer Terminal Engine

Thank you for helping make Khmer work properly in terminals. Contributions of every kind help: code, test cases, screenshots, font testing, documentation and reviews of rendering quality by native readers.

## Ways to contribute

- **Report a rendering problem.** Use the *Rendering issue* template. Include the terminal, OS, font, the exact text, and a screenshot.
- **Add test cases.** Add words or sentences that break in some terminal to `tests/corpus/`.
- **Review the spec.** Changes to [`spec/kte-width-spec.md`](spec/kte-width-spec.md) need input from both Khmer readers and terminal developers.
- **Write code.** Pick an issue labelled `good first issue` or `help wanted`.

## Development setup

```sh
git clone https://github.com/touchde/khmer-terminal-engine
cd khmer-terminal-engine
cargo build
cargo test
```

Before opening a pull request, run:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Pull request guidelines

- Keep each PR focused on one change.
- Add or update tests. Behaviour changes for Khmer text need corpus coverage.
- Update docs when you change behaviour or public API.
- Use [Conventional Commits](https://www.conventionalcommits.org/) for commit messages, e.g. `feat(unicode): add coeng-RO classification`, `fix(cell): …`, `docs: …`.
- Sign off your commits (`git commit -s`) to certify the [Developer Certificate of Origin](https://developercertificate.org/).

## Changing the width spec

The width rule is a contract with other projects, so changing it needs more care:

1. Open an issue labelled `spec` describing the problem, with examples and screenshots.
2. For significant changes, add an Architecture Decision Record in `docs/adr/`.
3. Update the spec, the conformance corpus and the implementation in the same PR.
4. Spec changes need approval from at least one maintainer and one native Khmer reader.

## Code conventions

- Rust edition 2024, formatted with `rustfmt`, warning-free under `clippy`.
- `unsafe` is denied everywhere except `kte-capi`. Every `unsafe` block there needs a `// SAFETY:` comment.
- Public items need doc comments.
- Never alter or normalize the user's text silently. KTE preserves the original codepoints.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).
