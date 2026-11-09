#!/usr/bin/env python3
"""Independent reference implementation of the KTE Width Spec (v0.1.0).

Written directly from spec/kte-width-spec.md, without looking at the Rust code,
so that the two implementations can check each other. It fills in the
`expected` field of every case in tests/corpus/khmer_corpus.json.

Usage:
    python3 tools/spec-ref/kte_spec_ref.py          # rewrite expected values
    python3 tools/spec-ref/kte_spec_ref.py --check  # fail if any value would change

Segmentation here supports Khmer, Latin (with combining marks), CJK and ZWSP.
Corpus text outside that range (e.g. emoji) is rejected rather than guessed.
"""

import json
import math
import sys
import unicodedata
from pathlib import Path

CORPUS = Path(__file__).resolve().parents[2] / "tests" / "corpus" / "khmer_corpus.json"
COENG = "្"

# --- Spec §5.2: base table ---------------------------------------------------
BASE_HALF = {}
for c in "រវ៖៛":
    BASE_HALF[c] = 1
for c in "ឃញឍយលសហឡឦ":
    BASE_HALF[c] = 3
for c in "ឈណ៚":
    BASE_HALF[c] = 4
for cp in list(range(0x19EA, 0x19F0)) + list(range(0x19FA, 0x1A00)):
    BASE_HALF[chr(cp)] = 4
BASE_HALF["៘"] = 6

# --- Spec §5.3: spacing signs -------------------------------------------------
SPACING_HALF = {c: 1 for c in "ាើេែៃះៈ"}
SPACING_HALF.update({c: 2 for c in "ឿៀោៅ"})

# --- Spec §5.4: subscripts ----------------------------------------------------
SUBSCRIPT_HALF = {c: 1 for c in "ឃឈឍបយរឞស"}

NON_SPACING = set(chr(cp) for cp in list(range(0x17B7, 0x17BE)) + [0x17C6] + list(range(0x17C9, 0x17D2)) + [0x17D3, 0x17DD])
INHERENT = {"឴", "឵"}
ZW_JOINERS = {"‌", "‍"}  # ZWNJ, ZWJ: grapheme Extend
ZWSP = "​"  # grapheme Control: its own cluster


def is_consonant(c):
    return "ក" <= c <= "អ"


def is_base(c):
    cp = ord(c)
    return (
        0x1780 <= cp <= 0x17B3
        or 0x17D4 <= cp <= 0x17DC
        or 0x17E0 <= cp <= 0x17E9
        or 0x17F0 <= cp <= 0x17F9
        or 0x19E0 <= cp <= 0x19FF
    )


def is_khmer(c):
    cp = ord(c)
    return (0x1780 <= cp <= 0x17FF or 0x19E0 <= cp <= 0x19FF) and unicodedata.category(c) != "Cn"


def attaches(c):
    """Characters that extend the preceding cluster (UAX #29 Extend/SpacingMark subset)."""
    return c in NON_SPACING or c in INHERENT or c in SPACING_HALF or c == COENG or c in ZW_JOINERS or unicodedata.category(c) in ("Mn", "Me")


def supported(c):
    if is_khmer(c) or c == ZWSP or c in ZW_JOINERS:
        return True
    if c.isascii() and c.isprintable():
        return True
    if unicodedata.category(c) in ("Mn", "Me"):
        return True
    if unicodedata.east_asian_width(c) in ("W", "F") and unicodedata.category(c).startswith("L"):
        return True
    return unicodedata.category(c) in ("Ll", "Lu", "Lo") and ord(c) < 0x0300


def segment(text):
    """Spec §4: grapheme clusters, then join COENG + consonant across clusters."""
    clusters = []
    for c in text:
        if not supported(c):
            raise ValueError(f"reference segmentation does not support U+{ord(c):04X}")
        if clusters and c != ZWSP and clusters[-1] != ZWSP and (attaches(c) or (clusters[-1].endswith(COENG) and is_consonant(c))):
            clusters[-1] += c
        else:
            clusters.append(c)
    return clusters


def uax11(c):
    if c in (ZWSP,) or c in ZW_JOINERS or unicodedata.category(c) in ("Mn", "Me"):
        return 0
    return 2 if unicodedata.east_asian_width(c) in ("W", "F") else 1


def half_units(cluster):
    """Spec §5.1. None for non-Khmer clusters."""
    if not is_khmer(cluster[0]):
        return None
    total = 0 if is_base(cluster[0]) else 2
    i = 0
    while i < len(cluster):
        c = cluster[i]
        if is_base(c):
            total += BASE_HALF.get(c, 2)
        elif c in SPACING_HALF:
            total += SPACING_HALF[c]
        elif c == COENG:
            if i + 1 < len(cluster) and is_consonant(cluster[i + 1]):
                total += SUBSCRIPT_HALF.get(cluster[i + 1], 0)
                i += 1
        elif c in NON_SPACING or c in INHERENT or c in ZW_JOINERS:
            pass
        else:
            total += 2 * uax11(c)
        i += 1
    return total


def width(cluster, model):
    h = half_units(cluster)
    if h is None:
        return sum(uax11(c) for c in cluster)
    return h if model == "wide" else math.ceil(h / 2)


def expected(text):
    clusters = segment(text)
    assert "".join(clusters) == text
    return {
        "clusters": clusters,
        "half_units": [half_units(c) for c in clusters],
        "wide": sum(width(c, "wide") for c in clusters),
        "compact": sum(width(c, "compact") for c in clusters),
    }


def main():
    check = "--check" in sys.argv
    cases = json.loads(CORPUS.read_text(encoding="utf-8"))
    changed = []
    for case in cases:
        new = expected(case["text"])
        if case.get("expected") != new:
            changed.append(case["id"])
            case["expected"] = new
    if check:
        if changed:
            print("expected values out of date for:", ", ".join(changed))
            sys.exit(1)
        print(f"ok: {len(cases)} cases up to date")
        return
    CORPUS.write_text(json.dumps(cases, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"updated {len(changed)} of {len(cases)} cases")


if __name__ == "__main__":
    main()
