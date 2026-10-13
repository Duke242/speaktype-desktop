//! Rule-based edits for the way people talk, applied after transcription:
//! voice commands ("scratch that", "new line").
//!
//! English only, and cautious on purpose: when a rule isn't sure what was
//! meant, the words stay as they were said.

mod commands;

pub use commands::apply as apply_commands;

/// A word in a transcript and where it sits.
#[derive(Debug, Clone, Copy)]
struct Word<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

impl Word<'_> {
    /// Lowercase, with curly apostrophes made straight.
    fn lower(&self) -> String {
        self.text.to_lowercase().replace('’', "'")
    }

    fn is(&self, word: &str) -> bool {
        self.lower() == word
    }

    fn is_any(&self, words: &[&str]) -> bool {
        words.contains(&self.lower().as_str())
    }
}

/// Splits text into words. Apostrophes, hyphens and dots inside a word stay
/// part of it ("don't", "twenty-five", "p.m"), as do digit separators ("3:30",
/// "1,000"), a currency sign before digits and a percent sign after them.
fn words(text: &str) -> Vec<Word<'_>> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let next_char = |i: usize| chars.get(i).map(|&(_, c)| c);
    let mut words = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let (start, c) = chars[i];
        let currency =
            matches!(c, '$' | '€' | '£') && next_char(i + 1).is_some_and(|n| n.is_ascii_digit());
        if !c.is_alphanumeric() && !currency {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while let Some(c) = next_char(j) {
            let prev = chars[j - 1].1;
            let next = next_char(j + 1);
            let joins = c.is_alphanumeric()
                || match c {
                    '\'' | '’' | '-' | '.' => {
                        prev.is_alphanumeric() && next.is_some_and(char::is_alphanumeric)
                    }
                    ',' | ':' => prev.is_ascii_digit() && next.is_some_and(|n| n.is_ascii_digit()),
                    '%' => prev.is_ascii_digit(),
                    _ => false,
                };
            if !joins {
                break;
            }
            j += 1;
        }
        let end = chars.get(j).map_or(text.len(), |&(b, _)| b);
        words.push(Word {
            text: &text[start..end],
            start,
            end,
        });
        i = j;
    }
    words
}

/// The text between two words.
fn gap<'a>(text: &'a str, before: &Word, after: &Word) -> &'a str {
    &text[before.end..after.start]
}

/// Whether two neighbouring words are separated only by spaces.
fn spaced(text: &str, before: &Word, after: &Word) -> bool {
    gap(text, before, after).chars().all(char::is_whitespace)
}

/// Whether something written at `at` starts a sentence.
fn at_sentence_start(text: &str, at: usize) -> bool {
    let before = text[..at].trim_end_matches([' ', '\t']);
    before.is_empty() || before.ends_with(['.', '!', '?', '\n'])
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(text: &str) -> Vec<&str> {
        words(text).iter().map(|w| w.text).collect()
    }

    #[test]
    fn words_keep_contractions_numbers_and_abbreviations_together() {
        assert_eq!(
            texts("Don't pay $1,000.50 at 3:30 p.m., twenty-five% off!"),
            [
                "Don't",
                "pay",
                "$1,000.50",
                "at",
                "3:30",
                "p.m",
                "twenty-five",
                "off"
            ]
        );
        assert_eq!(texts("5% of it’s done"), ["5%", "of", "it’s", "done"]);
        assert_eq!(texts("end. Next - one"), ["end", "Next", "one"]);
        assert_eq!(texts(""), Vec::<&str>::new());
        assert_eq!(texts("café naïve"), ["café", "naïve"]);
    }

    #[test]
    fn sentence_helpers() {
        assert!(at_sentence_start("", 0) && at_sentence_start("Hi. ", 4));
        assert!(!at_sentence_start("Hi ", 3));
        assert_eq!(capitalize("éa"), "Éa");
    }
}
