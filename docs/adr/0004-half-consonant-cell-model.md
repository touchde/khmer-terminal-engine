# 0004. Half-consonant cells (Wide model) as the default

- **Status:** Accepted
- **Date:** 2026-11-09

## Context

[Research 0001](../research/0001-khmer-advance-measurements.md) measured 160 Unicode Khmer fonts. Every Khmer codepoint's advance is a stable multiple of half a typical consonant: single vowels add 0.5, two-part vowels 1.0, consonants range from 0.5 (រ, វ) to 2 (ឈ, ណ), and 8 subscripts add 0.5.

Rules with one cell per consonant fit at most 72% of syllables within ±20%. A per-codepoint table in half-consonant cells fits 96% (100% for common system fonts).

## Decision

- Khmer widths are defined in **half-units** (half a typical consonant). The tables are in the [KTE Width Spec](../../spec/kte-width-spec.md).
- The default **Wide** cell model uses one cell per half-unit, so a typical consonant takes 2 cells, like a CJK character.
- A **Compact** model (one cell per consonant, rounded up per cluster) is also defined, for users who prefer density over even spacing.

## Consequences

- Khmer alignment in the grid is nearly exact, with no random gaps.
- Khmer is drawn larger than Latin text (about 1.3–1.9× depending on the spacing chosen) and needs extra line height.
- Applications and terminals must agree on the model. Detection is still open (spec §8).
- Switching the default later is cheap, because both models derive from the same half-unit table.
