# Khmer test corpus

Test cases shared by every KTE implementation and by upstream ports of the width rule.

## Format

`khmer_corpus.json` is a list of cases:

| Field | Meaning |
|-------|---------|
| `id` | Stable unique identifier, e.g. `word-0003` |
| `category` | `consonant`, `independent-vowel`, `dependent-vowel`, `subscript`, `coeng`, `diacritic`, `punctuation`, `numeral`, `symbol`, `mixed`, `word`, `sentence`, `paragraph` |
| `text` | The exact Unicode text (never normalized) |
| `spec` | Spec areas this case tests, e.g. `["width", "segmentation"]` |
| `expected` | Expected results. `null` until the spec value is decided |
| `note` | Optional explanation |

## Adding cases

Add text that breaks somewhere, and note where in `note` (terminal, font). Native readers: please check that each `text` is spelled correctly.
