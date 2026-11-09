# Research

Experiments and measurements behind KTE decisions. Each record states its question, method, data and limitations, so the result can be reproduced and challenged.

| # | Topic | Result |
|---|-------|--------|
| [0001](0001-khmer-advance-measurements.md) | Khmer advance measurements across 160 fonts | Widths are stable multiples of half a consonant; a half-consonant cell fits 96–100% of syllables |
| [0002](0002-windows-and-linux-fonts.md) | Spec table vs. Windows and Linux fonts | Linux fonts fit 98–100%; Windows' Leelawadee UI fits 80–86% (wide consonants drawn narrower) |
| [0003](0003-shell-compatibility.md) | Can today's shells edit Khmer? | Only if the terminal matches the system `wcwidth()` cell by cell; macOS and glibc differ on spacing vowels; zsh needs `COMBINING_CHARS` |
