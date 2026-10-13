//! Snippets: a phrase you say that turns into text you've saved, like "my
//! calendar link" into the link itself.
//!
//! A trigger matches as a whole phrase, in any case, even when the
//! transcriber put a comma inside it or punctuation after it. When the whole
//! dictation is the trigger, just the snippet is pasted, without the full stop.

use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snippet {
    pub id: String,
    pub trigger: String,
    pub expansion: String,
    #[serde(default = "yes")]
    pub is_enabled: bool,
}

fn yes() -> bool {
    true
}

/// The trigger's words, without punctuation around them.
fn trigger_words(trigger: &str) -> Vec<&str> {
    trigger
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| ",.;:!?\"".contains(c)))
        .filter(|w| !w.is_empty())
        .collect()
}

/// A pattern for a trigger, allowing any spacing or a comma between its words.
fn pattern(words: &[&str]) -> String {
    let body = words
        .iter()
        .map(|w| regex::escape(w))
        .collect::<Vec<_>>()
        .join(r"[,;:]?\s+");
    let word_char = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
    let start = if word_char(words.first().and_then(|w| w.chars().next())) {
        r"\b"
    } else {
        ""
    };
    let end = if word_char(words.last().and_then(|w| w.chars().last())) {
        r"\b"
    } else {
        ""
    };
    format!("{start}{body}{end}")
}

/// Expands enabled snippets in one pass, so an expansion is never expanded
/// again. Longer triggers win: "my work email" before "my email".
///
/// `spoken` gives the trigger as the rest of the pipeline would have written
/// it, so "my zoom room two" still matches after "two" became "2".
pub fn expand(text: &str, snippets: &[Snippet], spoken: impl Fn(&str) -> String) -> String {
    let mut active: Vec<(String, &Snippet)> = snippets
        .iter()
        .filter(|s| s.is_enabled)
        .filter_map(|s| {
            let words = trigger_words(&s.trigger);
            if words.is_empty() {
                return None;
            }
            let written = spoken(&words.join(" "));
            let written = trigger_words(&written);
            let mut alternatives = vec![pattern(&words)];
            if !written.is_empty() && written != words {
                alternatives.push(pattern(&written));
            }
            Some((alternatives.join("|"), s))
        })
        .collect();
    if text.is_empty() || active.is_empty() {
        return text.to_string();
    }
    active.sort_by_key(|(_, s)| std::cmp::Reverse(s.trigger.trim().len()));

    let groups: Vec<String> = active
        .iter()
        .enumerate()
        .map(|(i, (p, _))| format!("(?P<s{i}>{p})"))
        .collect();
    let any = groups.join("|");
    let expansion = |caps: &Captures| {
        active
            .iter()
            .enumerate()
            .find(|(i, _)| caps.name(&format!("s{i}")).is_some())
            .map_or("", |(_, (_, s))| s.expansion.as_str())
            .to_string()
    };

    // The whole dictation is the trigger.
    if let Ok(whole) = Regex::new(&format!(r"(?i)^\s*(?:{any})[\s.!?,;:]*$"))
        && let Some(caps) = whole.captures(text)
    {
        return expansion(&caps);
    }
    match Regex::new(&format!("(?i){any}")) {
        Ok(re) => re
            .replace_all(text, |caps: &Captures| expansion(caps))
            .into_owned(),
        Err(e) => {
            eprintln!("[snippets] couldn't build the pattern: {e}");
            text.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snippet(trigger: &str, expansion: &str) -> Snippet {
        Snippet {
            id: trigger.into(),
            trigger: trigger.into(),
            expansion: expansion.into(),
            is_enabled: true,
        }
    }

    fn same(text: &str) -> String {
        text.to_string()
    }

    #[test]
    fn whole_dictation_becomes_the_snippet() {
        let snippets = [snippet("my calendar link", "https://cal.com/karan")];
        for input in [
            "my calendar link",
            "My calendar link.",
            "  MY CALENDAR LINK!  ",
            "My calendar, link.",
        ] {
            assert_eq!(
                expand(input, &snippets, same),
                "https://cal.com/karan",
                "input: {input:?}"
            );
        }
    }

    #[test]
    fn triggers_inside_a_sentence_keep_the_punctuation_around_them() {
        let snippets = [snippet("my calendar link", "https://cal.com/karan")];
        assert_eq!(
            expand(
                "Book a time with my calendar link, thanks.",
                &snippets,
                same
            ),
            "Book a time with https://cal.com/karan, thanks."
        );
        assert_eq!(
            expand("Here's my Calendar link.", &snippets, same),
            "Here's https://cal.com/karan."
        );
    }

    #[test]
    fn only_whole_phrases_match() {
        let snippets = [snippet("addr", "1 Main St")];
        assert_eq!(expand("my address", &snippets, same), "my address");
        assert_eq!(
            expand("my addr please", &snippets, same),
            "my 1 Main St please"
        );
        let snippets = [snippet("my calendar link", "x")];
        assert_eq!(
            expand("my calendar linked", &snippets, same),
            "my calendar linked"
        );
    }

    #[test]
    fn longest_trigger_wins_and_expansions_are_not_expanded_again() {
        let snippets = [
            snippet("my email", "me@example.com"),
            snippet("my work email", "me@work.com"),
            snippet("sig", "Thanks, my email"),
        ];
        assert_eq!(
            expand("Send it to my work email or my email", &snippets, same),
            "Send it to me@work.com or me@example.com"
        );
        assert_eq!(expand("sig", &snippets, same), "Thanks, my email");
    }

    #[test]
    fn disabled_and_empty_snippets_are_skipped() {
        let mut off = snippet("my email", "me@example.com");
        off.is_enabled = false;
        let snippets = [off, snippet("  ", "x"), snippet("...", "y")];
        assert_eq!(expand("my email", &snippets, same), "my email");
        assert_eq!(expand("", &[snippet("a", "b")], same), "");
    }

    #[test]
    fn expansions_are_inserted_literally() {
        let snippets = [snippet("my price", "$1 and ${name}\nline two")];
        assert_eq!(
            expand("The total is my price", &snippets, same),
            "The total is $1 and ${name}\nline two"
        );
    }

    #[test]
    fn triggers_also_match_as_the_pipeline_writes_them() {
        let snippets = [snippet("room twenty five", "https://zoom.us/j/25")];
        let spoken = |t: &str| t.replace("twenty five", "25");
        assert_eq!(
            expand("Join room 25.", &snippets, spoken),
            "Join https://zoom.us/j/25."
        );
        assert_eq!(expand("Room 25", &snippets, spoken), "https://zoom.us/j/25");
        assert_eq!(
            expand("Join room twenty five.", &snippets, spoken),
            "Join https://zoom.us/j/25."
        );
    }
}
