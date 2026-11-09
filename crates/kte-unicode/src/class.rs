//! Classification of Khmer codepoints.

/// U+17D2 KHMER SIGN COENG: turns the following consonant into a subscript.
pub const COENG: char = '\u{17D2}';

/// The role a character plays in Khmer text layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharClass {
    /// Consonant, U+1780..=U+17A2.
    Consonant,
    /// Independent vowel, U+17A3..=U+17B3 (U+17A3 and U+17A4 are deprecated).
    IndependentVowel,
    /// Inherent vowel, U+17B4 and U+17B5 (invisible; discouraged).
    InherentVowel,
    /// Dependent vowel or sign with General_Category Mc: it takes horizontal space.
    SpacingSign,
    /// Dependent vowel or sign with General_Category Mn: drawn above or below.
    NonSpacingSign,
    /// U+17D2 KHMER SIGN COENG.
    Coeng,
    /// Punctuation and currency, U+17D4..=U+17DC.
    Punctuation,
    /// Khmer digit, U+17E0..=U+17E9.
    Digit,
    /// Numeric symbols for divination lore, U+17F0..=U+17F9.
    NumericSymbol,
    /// Khmer Symbols block (lunar dates), U+19E0..=U+19FF.
    LunarSymbol,
    /// ZWSP, ZWNJ or ZWJ (U+200B..=U+200D): used inside Khmer text, never visible.
    ZeroWidth,
    /// Any other character, including unassigned Khmer codepoints.
    Other,
}

impl CharClass {
    /// Whether this class starts a Khmer syllable (and occupies space on its own).
    pub fn is_base(self) -> bool {
        matches!(
            self,
            CharClass::Consonant
                | CharClass::IndependentVowel
                | CharClass::Punctuation
                | CharClass::Digit
                | CharClass::NumericSymbol
                | CharClass::LunarSymbol
        )
    }
}

/// Classifies a character for Khmer layout.
pub fn char_class(c: char) -> CharClass {
    match c {
        '\u{1780}'..='\u{17A2}' => CharClass::Consonant,
        '\u{17A3}'..='\u{17B3}' => CharClass::IndependentVowel,
        '\u{17B4}' | '\u{17B5}' => CharClass::InherentVowel,
        '\u{17B6}' | '\u{17BE}'..='\u{17C5}' | '\u{17C7}' | '\u{17C8}' => CharClass::SpacingSign,
        '\u{17B7}'..='\u{17BD}'
        | '\u{17C6}'
        | '\u{17C9}'..='\u{17D1}'
        | '\u{17D3}'
        | '\u{17DD}' => CharClass::NonSpacingSign,
        COENG => CharClass::Coeng,
        '\u{17D4}'..='\u{17DC}' => CharClass::Punctuation,
        '\u{17E0}'..='\u{17E9}' => CharClass::Digit,
        '\u{17F0}'..='\u{17F9}' => CharClass::NumericSymbol,
        '\u{19E0}'..='\u{19FF}' => CharClass::LunarSymbol,
        '\u{200B}'..='\u{200D}' => CharClass::ZeroWidth,
        _ => CharClass::Other,
    }
}

/// Whether `c` is an assigned character in the Khmer (U+1780..=U+17FF) or
/// Khmer Symbols (U+19E0..=U+19FF) blocks.
pub fn is_khmer(c: char) -> bool {
    !matches!(char_class(c), CharClass::Other | CharClass::ZeroWidth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_khmer_block_codepoint_is_classified() {
        // Unassigned: U+17DE..=U+17DF, U+17EA..=U+17EF, U+17FA..=U+17FF.
        for c in '\u{1780}'..='\u{17FF}' {
            let unassigned = matches!(c, '\u{17DE}'..='\u{17DF}' | '\u{17EA}'..='\u{17EF}' | '\u{17FA}'..='\u{17FF}');
            assert_eq!(
                char_class(c) == CharClass::Other,
                unassigned,
                "U+{:04X}",
                c as u32
            );
        }
    }

    #[test]
    fn examples() {
        assert_eq!(char_class('ក'), CharClass::Consonant);
        assert_eq!(char_class('ឥ'), CharClass::IndependentVowel);
        assert_eq!(char_class('ា'), CharClass::SpacingSign);
        assert_eq!(char_class('ោ'), CharClass::SpacingSign);
        assert_eq!(char_class('ិ'), CharClass::NonSpacingSign);
        assert_eq!(char_class('ំ'), CharClass::NonSpacingSign);
        assert_eq!(char_class('ះ'), CharClass::SpacingSign);
        assert_eq!(char_class('្'), CharClass::Coeng);
        assert_eq!(char_class('។'), CharClass::Punctuation);
        assert_eq!(char_class('៥'), CharClass::Digit);
        assert_eq!(char_class('a'), CharClass::Other);
        assert!(is_khmer('ក') && !is_khmer('a') && !is_khmer('\u{200B}'));
    }
}
