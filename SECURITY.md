# Security Policy

## Supported versions

KTE is pre-release. Only the latest commit on `main` receives security fixes.

## Reporting a vulnerability

Please **do not** open a public issue for security problems.

Report privately through GitHub's
[private vulnerability reporting](https://github.com/touchde/khmer-terminal-engine/security/advisories/new).
We aim to acknowledge reports within 7 days.

Terminal emulators parse untrusted input. Reports about these areas are especially welcome:

- escape-sequence parsing (crashes, hangs, unbounded memory use)
- malformed or adversarial Unicode input (e.g. very long combining sequences)
- memory safety in the C ABI (`kte-capi`)
