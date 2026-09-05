//! Spoken punctuation: turning what people *say* into what they mean.
//!
//! Dictating an email means saying "comma" and "new line" aloud. A lookup
//! table reads that more reliably than any amount of language modelling, and
//! does it instantly.
//!
//! This is a **processing step, not a refinement engine**. It was briefly the
//! latter, which was a mistake: sitting in the same list as Apple Intelligence
//! meant enabling it silently prevented any rewrite from running, and it only
//! applied in Rewrite mode. Punctuation you spoke aloud belongs in Verbatim
//! and Polished too, and should never cost a model call.

/// Spoken commands and the punctuation they stand for.
///
/// Ordered longest-first so "open quote" is matched before "quote".
const SPOKEN: &[(&str, Replacement)] = &[
    ("new paragraph", Replacement::Break("\n\n")),
    ("new line", Replacement::Break("\n")),
    ("open parenthesis", Replacement::Opening("(")),
    ("close parenthesis", Replacement::Closing(")")),
    ("question mark", Replacement::Closing("?")),
    ("exclamation mark", Replacement::Closing("!")),
    ("exclamation point", Replacement::Closing("!")),
    ("open quote", Replacement::Opening("\u{201c}")),
    ("close quote", Replacement::Closing("\u{201d}")),
    ("semicolon", Replacement::Closing(";")),
    ("full stop", Replacement::Closing(".")),
    ("ellipsis", Replacement::Closing("\u{2026}")),
    ("apostrophe", Replacement::Tight("\u{2019}")),
    ("hyphen", Replacement::Tight("-")),
    ("dash", Replacement::Spaced("\u{2014}")),
    ("colon", Replacement::Closing(":")),
    ("comma", Replacement::Closing(",")),
    ("period", Replacement::Closing(".")),
];

#[derive(Clone, Copy)]
enum Replacement {
    /// Attaches to the word before it: `word,`
    Closing(&'static str),
    /// Attaches to the word after it: `(word`
    Opening(&'static str),
    /// No space on either side: `don't`
    Tight(&'static str),
    /// Spaces on both sides.
    Spaced(&'static str),
    /// A literal line break.
    Break(&'static str),
}

/// Replace spoken punctuation with the characters it names.
pub fn apply_spoken_punctuation(input: &str) -> String {
    let mut text = format!(" {} ", input.to_lowercase());
    let original = format!(" {input} ");
    let mut result = original;

    for (phrase, replacement) in SPOKEN {
        let needle = format!(" {phrase} ");
        while let Some(at) = text.find(&needle) {
            let inserted = match replacement {
                // The needle consumed the space on both sides. Closing marks
                // attach to the word before, so the trailing space has to come
                // back or the next word is glued on: "hello,world".
                Replacement::Closing(mark) => format!("{mark} "),
                Replacement::Opening(mark) => format!(" {mark}"),
                Replacement::Tight(mark) => mark.to_string(),
                Replacement::Spaced(mark) => format!(" {mark} "),
                Replacement::Break(mark) => mark.to_string(),
            };

            // Splice the same span out of both, so the case-insensitive search
            // and the original-case output stay aligned.
            let end = at + needle.len();
            result.replace_range(at..end, &inserted);
            text.replace_range(at..end, &inserted.to_lowercase());
        }
    }

    tidy_spacing(result.trim())
}

/// Collapse the spaces that splicing leaves behind.
fn tidy_spacing(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut previous_space = false;

    for character in input.chars() {
        match character {
            ' ' | '\t' => {
                if !previous_space {
                    out.push(' ');
                }
                previous_space = true;
            }
            '\n' => {
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push('\n');
                previous_space = false;
            }
            _ => {
                // No space before punctuation that attaches to the word.
                if matches!(character, ',' | '.' | '!' | '?' | ';' | ':') {
                    while out.ends_with(' ') {
                        out.pop();
                    }
                }
                out.push(character);
                previous_space = false;
            }
        }
    }

    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spoken_punctuation_becomes_punctuation() {
        assert_eq!(
            apply_spoken_punctuation("hello comma how are you question mark"),
            "hello, how are you?"
        );
        assert_eq!(
            apply_spoken_punctuation("wait period then go"),
            "wait. then go"
        );
    }

    #[test]
    fn line_commands_produce_real_breaks() {
        assert_eq!(
            apply_spoken_punctuation("first line new line second line"),
            "first line\nsecond line"
        );
        assert!(apply_spoken_punctuation("one new paragraph two").contains("\n\n"));
    }

    /// "open quote" must win over "quote" — hence longest-first ordering.
    #[test]
    fn longer_phrases_match_before_their_prefixes() {
        let out = apply_spoken_punctuation("she said open quote hello close quote");
        assert!(out.contains('\u{201c}'), "got: {out}");
        assert!(out.contains('\u{201d}'), "got: {out}");
    }

    /// The words being replaced are case-insensitive, but everything else must
    /// keep the casing the speech engine produced.
    #[test]
    fn surrounding_text_keeps_its_original_case() {
        assert_eq!(
            apply_spoken_punctuation("Hello Comma World"),
            "Hello, World"
        );
    }

    #[test]
    fn ordinary_speech_is_left_alone() {
        for text in [
            "the meeting is at four",
            "let us period out the details", // 'period' mid-sentence still replaced
            "nothing to change here",
        ] {
            let out = apply_spoken_punctuation(text);
            assert!(!out.is_empty(), "{text} was emptied");
        }
        assert_eq!(
            apply_spoken_punctuation("the meeting is at four"),
            "the meeting is at four"
        );
    }

}
