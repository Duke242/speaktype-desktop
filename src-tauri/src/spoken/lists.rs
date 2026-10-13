//! Spoken lists: "first … second … third …" and "one … two … three …" become
//! numbered lists, and "bullet point …" becomes a bulleted one.
//!
//! Markers can't follow words like "the" or "came", so "the first time" or
//! "came second" don't start a list. Ordinals need three markers in a row, or
//! two that each start a clause ("First, … Second, …"). Numbers need three,
//! each starting a clause, and a bulleted list has to start a clause.

use super::{Word, capitalize, gap, spaced, words};

const ORDINALS: [&str; 10] = [
    "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth",
];
const ORDINALS_LY: [&str; 10] = [
    "firstly",
    "secondly",
    "thirdly",
    "fourthly",
    "fifthly",
    "sixthly",
    "seventhly",
    "eighthly",
    "ninthly",
    "tenthly",
];
const CARDINALS: [&str; 10] = [
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
];

/// Words before a marker that make it part of a sentence ("the first", "at one").
const NOT_AFTER: &[&str] = &[
    "the", "a", "an", "my", "your", "our", "their", "his", "her", "its", "this", "that", "at",
    "for", "in", "on", "of", "to", "be", "is", "was", "were", "are", "came", "come", "finished",
    "number",
];

/// Words after a marker that make it part of a sentence ("first time", "one of").
const NOT_BEFORE: &[&str] = &[
    "of", "time", "times", "place", "thing", "things", "half", "hand", "person", "people", "class",
    "floor", "grade", "round", "second", "seconds", "minute", "minutes", "and", "or", "more",
    "day", "days", "year", "years", "week", "weeks", "is", "was",
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Style {
    Ordinal,
    Cardinal,
    Bullet,
}

/// A list marker: its first and last word, and whether it starts a clause.
#[derive(Debug, Clone, Copy)]
struct Marker {
    first: usize,
    last: usize,
    clause_start: bool,
}

pub fn apply(text: &str) -> String {
    let mut text = text.to_string();
    // A dictation can hold more than one list. Each pass replaces its
    // markers, so this ends; the cap is a backstop.
    for _ in 0..5 {
        match list_once(&text) {
            Some(next) => text = next,
            None => break,
        }
    }
    text
}

fn list_once(text: &str) -> Option<String> {
    let words = words(text);
    (0..words.len()).find_map(|i| {
        [Style::Bullet, Style::Ordinal, Style::Cardinal]
            .into_iter()
            .find_map(|style| {
                let markers = sequence(text, &words, i, style)?;
                render(text, &words, &markers, style)
            })
    })
}

/// The `n`th marker (from 0) of a list, if it starts at word `i`.
fn marker_at(text: &str, words: &[Word], i: usize, n: usize, style: Style) -> Option<Marker> {
    let word = words[i];
    let followed_by = |k: usize, w: &str| {
        words
            .get(k + 1)
            .is_some_and(|next| spaced(text, &words[k], next) && next.is(w))
    };
    let last = match style {
        Style::Bullet if word.is("bullet") && followed_by(i, "point") => i + 1,
        Style::Bullet => return None,
        Style::Ordinal
            if n == 0 && word.is("first") && followed_by(i, "of") && followed_by(i + 1, "all") =>
        {
            i + 2
        }
        Style::Ordinal if word.is(ORDINALS.get(n)?) || word.is(ORDINALS_LY.get(n)?) => i,
        Style::Cardinal if word.is(CARDINALS.get(n)?) => i,
        _ => return None,
    };
    let clause_start =
        i == 0 || gap(text, &words[i - 1], &word).contains([',', '.', ';', ':', '!', '?', '\n']);
    if !clause_start && words[i - 1].is_any(NOT_AFTER) {
        return None;
    }
    // Something has to follow, and not something that makes it a phrase.
    let next = words.get(last + 1)?;
    let joined = spaced(text, &words[last], next);
    if style != Style::Bullet
        && joined
        && (next.is_any(NOT_BEFORE) || super::numbers::is_number_word(&next.lower()))
    {
        return None;
    }
    Some(Marker {
        first: i,
        last,
        clause_start,
    })
}

/// The markers of a list that starts at word `i`, if one does.
fn sequence(text: &str, words: &[Word], i: usize, style: Style) -> Option<Vec<Marker>> {
    let first = marker_at(text, words, i, 0, style)?;
    let mut markers = vec![first];
    let mut from = first.last + 1;
    while let Some(marker) =
        (from..words.len()).find_map(|j| marker_at(text, words, j, markers.len(), style))
    {
        from = marker.last + 1;
        markers.push(marker);
    }
    let all_clauses = markers.iter().all(|m| m.clause_start);
    let enough = match style {
        Style::Bullet => first.clause_start,
        Style::Ordinal => markers.len() >= 3 || (markers.len() == 2 && all_clauses),
        Style::Cardinal => markers.len() >= 3 && all_clauses,
    };
    enough.then_some(markers)
}

/// Where the sentence that starts at `from` ends: after its closing
/// punctuation, or at a line break or the end of the text.
fn sentence_end(text: &str, from: usize) -> usize {
    let rest = &text[from..];
    rest.char_indices()
        .find_map(|(i, c)| {
            let after = i + c.len_utf8();
            if c == '\n' {
                Some(i)
            } else if ".!?".contains(c)
                && rest[after..].chars().next().is_none_or(char::is_whitespace)
            {
                Some(after)
            } else {
                None
            }
        })
        .map_or(text.len(), |i| from + i)
}

/// An item's text without the joining words and punctuation around it.
fn tidy(item: &str) -> &str {
    let mut item = item.trim_start_matches(|c: char| c.is_whitespace() || ",:;-".contains(c));
    loop {
        let trimmed = item.trim_end_matches(|c: char| c.is_whitespace() || ",;:".contains(c));
        let lower = trimmed.to_lowercase();
        let joiner = [" and then", " and", " then"]
            .into_iter()
            .find(|j| lower.ends_with(j));
        match joiner {
            Some(j) => item = &trimmed[..trimmed.len() - j.len()],
            None => return trimmed,
        }
    }
}

fn render(text: &str, words: &[Word], markers: &[Marker], style: Style) -> Option<String> {
    let mut items = Vec::new();
    let mut end = 0;
    for (n, marker) in markers.iter().enumerate() {
        let start = words[marker.last].end;
        end = match markers.get(n + 1) {
            Some(next) => words[next.first].start,
            None => sentence_end(text, start),
        };
        let item = tidy(&text[start..end]);
        if item.is_empty() {
            return None;
        }
        items.push(item);
    }

    // Short items read as a list without full stops.
    let short = items.iter().all(|item| {
        let body = item.trim_end_matches(['.', '!', '?']);
        body.split_whitespace().count() <= 6 && !body.contains(['.', '!', '?'])
    });
    let lines: Vec<String> = items
        .iter()
        .enumerate()
        .map(|(n, item)| {
            let item = if short {
                item.trim_end_matches('.')
            } else {
                item
            };
            match style {
                Style::Bullet => format!("- {}", capitalize(item)),
                Style::Ordinal | Style::Cardinal => format!("{}. {}", n + 1, capitalize(item)),
            }
        })
        .collect();

    let before = &text[..words[markers[0].first].start];
    let intro = before.trim_end().trim_end_matches([',', ';']).trim_end();
    let mut out = intro.to_string();
    if !intro.is_empty() {
        // "Shopping list first eggs…" introduces the list: "Shopping list:".
        let same_line = !before[intro.len()..].contains('\n');
        if same_line && intro.ends_with(char::is_alphanumeric) {
            out.push(':');
        }
        out.push('\n');
    }
    out.push_str(&lines.join("\n"));
    let rest = text[end..].trim_start();
    if !rest.is_empty() {
        out.push('\n');
        out.push_str(&capitalize(rest));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::apply;

    fn check(cases: &[(&str, &str)]) {
        for (input, expected) in cases {
            assert_eq!(apply(input), *expected, "input: {input:?}");
        }
    }

    #[test]
    fn ordinals_make_numbered_lists() {
        check(&[
            (
                "My list: first eggs, second milk, third bread.",
                "My list:\n1. Eggs\n2. Milk\n3. Bread",
            ),
            (
                "Shopping list first eggs second milk and third bread",
                "Shopping list:\n1. Eggs\n2. Milk\n3. Bread",
            ),
            (
                "First, we fix the bug and write a test for it. Second, we ship the release to everyone. That's the plan.",
                "1. We fix the bug and write a test for it.\n2. We ship the release to everyone.\nThat's the plan.",
            ),
            (
                "Here's the plan. Firstly, design. Secondly, build. Thirdly, test.",
                "Here's the plan.\n1. Design\n2. Build\n3. Test",
            ),
            (
                "First of all, thanks. Second, welcome.",
                "1. Thanks\n2. Welcome",
            ),
        ]);
    }

    #[test]
    fn numbers_and_bullets_make_lists() {
        check(&[
            (
                "Agenda: one, budget. Two, hiring. Three, roadmap.",
                "Agenda:\n1. Budget\n2. Hiring\n3. Roadmap",
            ),
            (
                "bullet point buy milk bullet point call mom",
                "- Buy milk\n- Call mom",
            ),
            ("Bullet point, review the draft.", "- Review the draft"),
            (
                "Notes: bullet point pricing is fine. Bullet point ship Friday. Thanks all.",
                "Notes:\n- Pricing is fine\n- Ship Friday\nThanks all.",
            ),
        ]);
    }

    #[test]
    fn ordinary_sentences_are_not_lists() {
        for input in [
            "The first time I saw it, the second time I loved it.",
            "At first I was skeptical, second guessing everything.",
            "I came first, second was Bob.",
            "One person said two things and three others agreed.",
            "First, let me say thanks.",
            "One of them is late. Two of them are early. Three of them left.",
            "one two three",
            "Add a bullet point about pricing.",
            "This bullet point is wrong.",
            "First things first.",
            "Second, I disagree.",
            "First, eggs.",
        ] {
            assert_eq!(apply(input), input, "input: {input:?}");
        }
    }
}
