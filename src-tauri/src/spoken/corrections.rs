//! Corrections made while speaking: "meet at 3, actually 4" becomes "meet at 4".
//!
//! A marker ("actually", "no wait", "I mean", "sorry", "or rather") only
//! counts when what follows it clearly stands in for what came just before:
//! a number for a number, a name for a name, a day for a day, or a short
//! phrase that starts or ends the same way. Anything else is left alone, so
//! "I actually liked it" keeps its "actually".

use super::{
    Word, at_sentence_start, capitalize, ends_phrase, ends_sentence, gap, numbers, spaced,
    starts_sentence, words,
};

const MARKERS: &[&[&str]] = &[
    &["actually"],
    &["no", "wait"],
    &["i", "mean"],
    &["sorry"],
    &["or", "rather"],
];

/// Words that make the marker after them part of the sentence ("I'm sorry",
/// "it is actually", "what I mean").
const IN_SENTENCE_BEFORE: &[&str] = &[
    "i'm", "im", "am", "so", "very", "really", "feel", "felt", "be", "been", "was", "were", "is",
    "are", "say", "said", "not", "too", "sound", "sounds", "what", "you", "it's", "that's", "i",
];

/// Words that say a number is a time, an amount or a count, so the unit
/// follows a corrected number along ("3 pm, actually 4" is "4 pm").
const UNITS: &[&str] = &[
    "am", "pm", "a.m", "p.m", "o'clock", "percent", "dollars", "dollar", "euros", "cents",
    "minutes", "hours", "days", "weeks", "months", "years", "people", "times",
];

const AM_PM: &[&str] = &["am", "pm", "a.m", "p.m"];

/// Words after a number that make it an amount of its own rather than a
/// correction ("we need 5, actually 2 more").
const NOT_AFTER_NUMBER: &[&str] = &["of", "more", "less", "fewer", "extra", "too", "than"];

/// Words that may follow a corrected day before the phrase ends ("Tuesday,
/// I mean Wednesday at noon").
const AFTER_DAY: &[&str] = &["at", "morning", "afternoon", "evening", "night"];

const DAYS: &[&str] = &[
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
    "today",
    "tomorrow",
    "tonight",
    "yesterday",
];

const MONTHS: &[&str] = &[
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

const PRONOUNS: &[&str] = &["i", "i'm", "i'll", "i've", "i'd"];

pub fn apply(text: &str) -> String {
    let mut text = text.to_string();
    // Each correction removes its marker, so this ends.
    while let Some(next) = correct_once(&text) {
        text = next;
    }
    text
}

/// A byte range and what replaces it.
type Edit = (usize, usize, String);

fn correct_once(text: &str) -> Option<String> {
    let words = words(text);
    let (start, end, with) = (1..words.len()).find_map(|m| {
        let len = marker_at(text, &words, m)?;
        correct(text, &words, m, len)
    })?;
    let with = if at_sentence_start(text, start) {
        capitalize(&with)
    } else {
        with
    };
    Some(format!("{}{with}{}", &text[..start], &text[end..]))
}

/// The length in words of the marker starting at word `m`, if there's one and
/// a word before it.
fn marker_at(text: &str, words: &[Word], m: usize) -> Option<usize> {
    let marker = MARKERS.iter().find(|marker| {
        let n = marker.len();
        m + n <= words.len()
            && (0..n).all(|k| words[m + k].is(marker[k]))
            // Only a comma may split a marker: "no, wait".
            && (1..n).all(|k| {
                gap(text, &words[m + k - 1], &words[m + k])
                    .chars()
                    .all(|c| c.is_whitespace() || c == ',')
            })
    })?;
    let before = words[m - 1];
    let part_of_sentence = spaced(text, &before, &words[m]) && before.is_any(IN_SENTENCE_BEFORE);
    (!part_of_sentence).then_some(marker.len())
}

fn correct(text: &str, words: &[Word], m: usize, len: usize) -> Option<Edit> {
    let prev = m - 1;
    let r = m + len;
    let next = words.get(r)?;
    let before = gap(text, &words[prev], &words[m]);
    let after = gap(text, &words[r - 1], next);
    let plain_before = before
        .chars()
        .all(|c| c.is_whitespace() || ",.;:!?—-".contains(c));
    let plain_after = after
        .chars()
        .all(|c| c.is_whitespace() || ",:—-".contains(c));
    if !plain_before || !plain_after {
        return None;
    }
    let new_sentence = ends_sentence(before);
    // "No wait" and "or rather" only ever correct; "actually", "I mean" and
    // "sorry" can also go on to say more ("it's fine, actually it's great").
    let hard = words[m].is_any(&["no", "or"]);
    number(text, words, prev, r, new_sentence)
        .or_else(|| single_word(text, words, prev, r, new_sentence))
        .or_else(|| {
            (!new_sentence)
                .then(|| phrase(text, words, prev, r, hard))
                .flatten()
        })
}

fn is_digits(word: &Word) -> bool {
    word.text
        .trim_start_matches(['$', '€', '£'])
        .starts_with(|c: char| c.is_ascii_digit())
}

fn is_number(word: &Word) -> bool {
    is_digits(word) || numbers::is_number_word(&word.lower())
}

/// The first word of the number that ends at word `end`.
fn number_start(text: &str, words: &[Word], end: usize) -> Option<usize> {
    if is_digits(&words[end]) {
        return Some(end);
    }
    if !is_number(&words[end]) {
        return None;
    }
    let mut start = end;
    while start > 0
        && spaced(text, &words[start - 1], &words[start])
        && is_number(&words[start - 1])
        && !is_digits(&words[start - 1])
    {
        start -= 1;
    }
    Some(start)
}

/// How many words the number starting at word `start` has.
fn number_len(text: &str, words: &[Word], start: usize) -> usize {
    if is_digits(&words[start]) {
        return 1;
    }
    let mut end = start;
    while end < words.len()
        && is_number(&words[end])
        && !is_digits(&words[end])
        && (end == start || spaced(text, &words[end - 1], &words[end]))
    {
        end += 1;
    }
    end - start
}

fn span<'a>(text: &'a str, words: &[Word], first: usize, last: usize) -> &'a str {
    &text[words[first].start..words[last].end]
}

/// "at 3, actually 4", "3 pm, sorry 4 pm", "5 people, I mean 6".
fn number(text: &str, words: &[Word], prev: usize, r: usize, new_sentence: bool) -> Option<Edit> {
    let r_len = number_len(text, words, r);
    if r_len == 0 {
        return None;
    }
    let r_last = r + r_len - 1;
    // The number being corrected, and the word after it, if any ("5 people").
    let (l_start, unit) = match number_start(text, words, prev) {
        Some(start) => (start, None),
        None if prev > 0
            && spaced(text, &words[prev - 1], &words[prev])
            && words[prev].text.chars().all(char::is_alphabetic) =>
        {
            (number_start(text, words, prev - 1)?, Some(words[prev]))
        }
        None => return None,
    };
    let r_unit = words.get(r_last + 1).filter(|w| {
        spaced(text, &words[r_last], w)
            && match unit {
                Some(unit) => w.is(&unit.lower()) || (w.is_any(AM_PM) && unit.is_any(AM_PM)),
                None => w.is_any(UNITS),
            }
    });
    let end = if r_unit.is_some() { r_last + 1 } else { r_last };
    // "5, actually one of them" and "5, actually 2 more" aren't corrections.
    if words
        .get(end + 1)
        .is_some_and(|w| w.is_any(NOT_AFTER_NUMBER))
    {
        return None;
    }
    let finished = ends_phrase(text, &words[end]);
    if new_sentence && !finished {
        return None;
    }
    let with = match (unit, r_unit) {
        (Some(unit), None) if finished => format!("{} {}", span(text, words, r, r_last), unit.text),
        (Some(_), None) => return None,
        _ => span(text, words, r, end).to_string(),
    };
    Some((words[l_start].start, words[end].end, with))
}

/// "to Bob, no wait, Alice" and "on Tuesday, I mean Wednesday".
fn single_word(
    text: &str,
    words: &[Word],
    prev: usize,
    r: usize,
    new_sentence: bool,
) -> Option<Edit> {
    let (left, right) = (words[prev], words[r]);
    let finished = ends_phrase(text, &right);
    let same_kind = |set: &[&str]| left.is_any(set) && right.is_any(set);
    let ok = if same_kind(DAYS) || same_kind(MONTHS) {
        // Not "free on Monday, actually Tuesday too", which adds a day.
        let time_follows = words
            .get(r + 1)
            .is_some_and(|next| spaced(text, &right, next) && next.is_any(AFTER_DAY));
        finished || (time_follows && !new_sentence)
    } else {
        // Capitalized mid-sentence, so most likely names.
        let is_name = |w: &Word| {
            w.is_capitalized() && !w.is_any(PRONOUNS) && w.text.chars().all(char::is_alphabetic)
        };
        finished && is_name(&left) && is_name(&right) && !starts_sentence(text, words, prev)
    };
    (ok && !left.text.eq_ignore_ascii_case(right.text))
        .then(|| (left.start, right.end, right.text.to_string()))
}

/// Whether two words are the same kind of thing: days, months or numbers.
fn alike(left: &Word, right: &Word) -> bool {
    let both = |set: &[&str]| left.is_any(set) && right.is_any(set);
    both(DAYS) || both(MONTHS) || (is_number(left) && is_number(right))
}

/// "the red one, actually the blue one": a short phrase that ends like the
/// one before it. One that only starts the same way ("I was wrong, actually
/// I was right") says something new, unless the marker is "no wait" or "or
/// rather", or the words that differ are the same kind ("call me tomorrow,
/// actually call me Friday").
fn phrase(text: &str, words: &[Word], prev: usize, r: usize, hard: bool) -> Option<Edit> {
    for n in [3, 2] {
        if prev + 1 < n || r + n > words.len() {
            continue;
        }
        let (l, r_last) = (prev + 1 - n, r + n - 1);
        let joined =
            |from: usize, to: usize| (from..to).all(|k| spaced(text, &words[k], &words[k + 1]));
        if !joined(l, prev) || !joined(r, r_last) || !ends_phrase(text, &words[r_last]) {
            continue;
        }
        let left: Vec<String> = (l..=prev).map(|k| words[k].lower()).collect();
        let right: Vec<String> = (r..=r_last).map(|k| words[k].lower()).collect();
        let same_start = left[0] == right[0] && (hard || alike(&words[prev], &words[r_last]));
        if left != right && (same_start || left[n - 1] == right[n - 1]) {
            return Some((
                words[l].start,
                words[r_last].end,
                span(text, words, r, r_last).to_string(),
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::apply;

    #[test]
    fn numbers_are_corrected() {
        let cases = [
            ("Let's meet at 3, actually 4.", "Let's meet at 4."),
            ("meet at 3 actually 4", "meet at 4"),
            ("Meet at 3. Actually, 4.", "Meet at 4."),
            ("Meet at 3 pm, actually 4.", "Meet at 4 pm."),
            ("Meet at 3 pm, no wait, 4 pm.", "Meet at 4 pm."),
            ("Meet at 3, actually 4 pm", "Meet at 4 pm"),
            ("We need 5 people, I mean 6.", "We need 6 people."),
            (
                "We need 5 people, I mean 6 people tomorrow.",
                "We need 6 people tomorrow.",
            ),
            ("Order three, sorry, four boxes", "Order four boxes"),
            ("It costs $20, actually $25.", "It costs $25."),
            (
                "meet at 3, actually 4 in the lobby",
                "meet at 4 in the lobby",
            ),
            (
                "Call me at twenty five, or rather thirty.",
                "Call me at thirty.",
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(apply(input), expected, "input: {input:?}");
        }
    }

    #[test]
    fn names_days_and_phrases_are_corrected() {
        let cases = [
            ("Send it to Bob, no wait, Alice.", "Send it to Alice."),
            ("Send it to Bob no wait Alice", "Send it to Alice"),
            ("Send it to Bob. No, wait, Alice.", "Send it to Alice."),
            ("I said Bob, sorry, Bobby.", "I said Bobby."),
            (
                "Let's meet Tuesday, I mean Wednesday at noon.",
                "Let's meet Wednesday at noon.",
            ),
            ("Due in March, actually April.", "Due in April."),
            (
                "I'll take the red one, actually the blue one.",
                "I'll take the blue one.",
            ),
            (
                "Put it in the top drawer, sorry, the bottom drawer.",
                "Put it in the bottom drawer.",
            ),
            (
                "Call me tomorrow, actually call me Friday.",
                "Call me Friday.",
            ),
            ("Today, actually tomorrow, works.", "Tomorrow, works."),
            (
                "I'm free on Monday, actually Tuesday.",
                "I'm free on Tuesday.",
            ),
            ("It's fine, no wait, it's broken.", "It's broken."),
        ];
        for (input, expected) in cases {
            assert_eq!(apply(input), expected, "input: {input:?}");
        }
    }

    #[test]
    fn markers_in_ordinary_sentences_are_left_alone() {
        for input in [
            "I actually liked it.",
            "It is actually 4 now.",
            "I'm sorry Bob.",
            "Sorry, John, I missed that.",
            "Thanks, sorry for the delay.",
            "Thank you so much, sorry about that.",
            "Great work, I mean it.",
            "That's not what I mean.",
            "There's no wait time today.",
            "I have 2 kids, actually I'm tired.",
            "We met John. Actually, Mary was there too.",
            "I'll bring 3. Actually 4 people are coming.",
            "It's 5 actually one of the best.",
            "Version 2 is great actually.",
            "We met Bob, actually.",
            "Actually, Alice is right.",
            "I think so, actually.",
            "Go left, sorry, I mean right.",
            "It was cold, actually freezing.",
            "Bob said sorry Alice.",
            "I'm free on Monday, actually Tuesday too.",
            "I'm busy today, actually tomorrow too.",
            "I was wrong, actually I was right.",
            "It's fine, actually it's great.",
            "This is good, I mean this is great.",
            "We're close, actually we're done.",
            "We need 5, actually 2 more.",
            "We sold 10, actually 5 more than last year.",
        ] {
            assert_eq!(apply(input), input, "input: {input:?}");
        }
    }
}
