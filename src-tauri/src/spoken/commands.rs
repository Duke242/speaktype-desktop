//! Voice commands: "scratch that" takes back the sentence just said, and
//! "new line" and "new paragraph" break the text.

use super::{Word, at_sentence_start, capitalize, spaced, words};

/// Words before "scratch that" that make it part of a sentence ("I'll scratch that").
const SCRATCH_AFTER: &[&str] = &[
    "i", "you", "we", "they", "he", "she", "to", "don't", "didn't", "can", "can't", "could",
    "will", "would", "should", "won't", "i'll", "we'll", "you'll", "they'll", "let's", "not",
    "never", "must", "might",
];

/// Words after "scratch that" that make it part of a sentence ("scratch that itch").
const SCRATCH_BEFORE: &[&str] = &[
    "off", "out", "itch", "from", "up", "away", "card", "cards", "ticket", "tickets", "part",
    "bit", "one", "surface",
];

/// Words before "new line" that make it a noun ("a new line of products").
const BREAK_AFTER: &[&str] = &[
    "a", "an", "the", "this", "that", "these", "those", "our", "your", "my", "his", "her", "their",
    "its", "each", "every", "another", "any", "no", "whole", "entire", "same",
];

/// Words after "new line" that make it a noun ("the new line is").
const BREAK_BEFORE: &[&str] = &[
    "of",
    "for",
    "is",
    "was",
    "are",
    "were",
    "has",
    "had",
    "will",
    "would",
    "should",
    "can",
    "could",
    "character",
    "characters",
    "break",
    "breaks",
    "item",
    "items",
    "rule",
    "rules",
    "number",
    "numbers",
    "code",
    "feed",
    "spacing",
    "height",
    "segment",
];

pub fn apply(text: &str) -> String {
    let mut text = text.to_string();
    // Each pass removes one command, so this ends.
    while let Some(next) = scratch_once(&text) {
        text = next;
    }
    breaks(&text)
}

fn is_scratch(text: &str, words: &[Word], i: usize) -> bool {
    let (Some(scratch), Some(that)) = (words.get(i), words.get(i + 1)) else {
        return false;
    };
    let in_sentence_before =
        i > 0 && spaced(text, &words[i - 1], scratch) && words[i - 1].is_any(SCRATCH_AFTER);
    let in_sentence_after = words
        .get(i + 2)
        .is_some_and(|next| spaced(text, that, next) && next.is_any(SCRATCH_BEFORE));
    scratch.is("scratch")
        && that.is("that")
        && spaced(text, scratch, that)
        && !in_sentence_before
        && !in_sentence_after
}

/// Removes the first "scratch that" and the sentence it takes back: the
/// unfinished one it interrupts, or the one just finished.
fn scratch_once(text: &str) -> Option<String> {
    let words = words(text);
    let i = (0..words.len()).find(|&i| is_scratch(text, &words, i))?;
    let (scratch, that) = (words[i], words[i + 1]);

    let said = text[..scratch.start]
        .trim_end_matches(|c: char| c.is_whitespace() || ",;:-—".contains(c))
        .trim_end_matches(['.', '!', '?']);
    let kept = text[..sentence_start(said)].trim_end_matches([' ', '\t']);
    let after =
        text[that.end..].trim_start_matches(|c: char| c.is_whitespace() || ",.;:!?".contains(c));

    Some(if after.is_empty() {
        kept.trim_end().to_string()
    } else if kept.is_empty() || kept.ends_with('\n') {
        format!("{kept}{}", capitalize(after))
    } else {
        format!("{kept} {}", capitalize(after))
    })
}

/// Where the last sentence in `text` starts.
fn sentence_start(text: &str) -> usize {
    text.char_indices()
        .rev()
        .find(|&(i, c)| {
            let rest = &text[i + c.len_utf8()..];
            c == '\n' || (".!?".contains(c) && rest.starts_with(char::is_whitespace))
        })
        .map_or(0, |(i, c)| i + c.len_utf8())
}

/// The command starting at word `i`, as its length in words and the break it makes.
fn break_at(text: &str, words: &[Word], i: usize) -> Option<(usize, &'static str)> {
    let word = words[i];
    let (len, kind) = if word.is("newline") {
        (1, "\n")
    } else if let Some(next) = words.get(i + 1)
        && word.is("new")
        && spaced(text, &word, next)
    {
        match next.lower().as_str() {
            "line" => (2, "\n"),
            "paragraph" => (2, "\n\n"),
            _ => return None,
        }
    } else {
        return None;
    };
    let noun_before =
        i > 0 && spaced(text, &words[i - 1], &word) && words[i - 1].is_any(BREAK_AFTER);
    let last = words[i + len - 1];
    let noun_after = words
        .get(i + len)
        .is_some_and(|next| spaced(text, &last, next) && next.is_any(BREAK_BEFORE));
    (!noun_before && !noun_after).then_some((len, kind))
}

/// Turns "new line" and "new paragraph" into line breaks. A comma before the
/// command and punctuation after it go; a full stop before it stays.
fn breaks(text: &str) -> String {
    let words = words(text);
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let mut capitalize_next = false;
    let mut i = 0;
    while i < words.len() {
        let Some((len, kind)) = break_at(text, &words, i) else {
            i += 1;
            continue;
        };
        let start = text[..words[i].start]
            .trim_end_matches(|c: char| c.is_whitespace() || ",;".contains(c))
            .len()
            .max(copied);
        let rest = &text[words[i + len - 1].end..];
        let skipped = rest.len()
            - rest
                .trim_start_matches(|c: char| c.is_whitespace() || ",.;:!?".contains(c))
                .len();
        push(&mut out, &text[copied..start], capitalize_next);
        out.push_str(kind);
        copied = words[i + len - 1].end + skipped;
        capitalize_next = true;
        i += len;
    }
    push(&mut out, &text[copied..], capitalize_next);
    out
}

fn push(out: &mut String, text: &str, capitalized: bool) {
    if capitalized && at_sentence_start(out, out.len()) {
        out.push_str(&capitalize(text));
    } else {
        out.push_str(text);
    }
}

#[cfg(test)]
mod tests {
    use super::apply;

    #[test]
    fn scratch_that_removes_the_sentence_it_interrupts() {
        let cases = [
            (
                "Send it today, scratch that, send it tomorrow.",
                "Send it tomorrow.",
            ),
            (
                "Send it today. Scratch that. Send it tomorrow.",
                "Send it tomorrow.",
            ),
            (
                "I'll call you. Let's meet at noon scratch that let's meet at one.",
                "I'll call you. Let's meet at one.",
            ),
            ("Hello there. Scratch that.", ""),
            (
                "Thanks for coming. Bad idea, scratch that",
                "Thanks for coming.",
            ),
            ("Scratch that. Hello", "Hello"),
            (
                "First line\nsecond line scratch that third line",
                "First line\nThird line",
            ),
            ("One. Two. Scratch that. Scratch that. Three.", "Three."),
            ("Is it 3.5? No, scratch that, it's 4.", "Is it 3.5? It's 4."),
        ];
        for (input, expected) in cases {
            assert_eq!(apply(input), expected, "input: {input:?}");
        }
    }

    #[test]
    fn scratch_that_in_a_sentence_is_left_alone() {
        for input in [
            "I'll scratch that off the list.",
            "Don't scratch that, it's new.",
            "You need to scratch that itch.",
            "We can scratch that from the plan.",
            "Scratch the surface.",
            "That scratch looks bad.",
        ] {
            assert_eq!(apply(input), input, "input: {input:?}");
        }
    }

    #[test]
    fn new_line_and_new_paragraph_break_the_text() {
        let cases = [
            ("Hello new line how are you", "Hello\nHow are you"),
            (
                "Dear Sam, new paragraph. Thanks for the notes.",
                "Dear Sam\n\nThanks for the notes.",
            ),
            ("Done. New paragraph. Next steps.", "Done.\n\nNext steps."),
            ("Eggs new line milk new line", "Eggs\nMilk\n"),
            (
                "Add a new line item for travel",
                "Add a new line item for travel",
            ),
            ("Hi, new line, new line bye", "Hi\n\nBye"),
            ("Newline then this", "\nThen this"),
            ("Sign here new line", "Sign here\n"),
        ];
        for (input, expected) in cases {
            assert_eq!(apply(input), expected, "input: {input:?}");
        }
    }

    #[test]
    fn new_line_as_a_noun_is_left_alone() {
        for input in [
            "We're launching a new line of shoes.",
            "Start a new paragraph here.",
            "The new line is broken.",
            "Insert a newline character.",
            "Their new line for kids sold out.",
            "Our new paragraph reads better.",
            "Add the new line break",
        ] {
            assert_eq!(apply(input), input, "input: {input:?}");
        }
    }
}
