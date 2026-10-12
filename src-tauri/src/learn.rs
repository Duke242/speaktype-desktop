//! Learns the words a user keeps correcting.
//!
//! When a transcript is fixed by hand, in History or in the app it was typed
//! into, each short swap of a misheard word or phrase for one that sounds like
//! it becomes a dictionary entry. That entry fixes the word in later
//! transcripts, and `text::prompt` hints it to Whisper so it's heard right.
//!
//! Only swaps that look like mishearings are learned: a word or phrase of up to
//! three words replaced by something that sounds similar and isn't everyday
//! vocabulary. Rewording a sentence, or fixing "their" to "there", teaches nothing.
//!
//! On macOS and Windows, the field a dictation was typed into is watched for a
//! short while afterwards (`watch_for_corrections`), so fixing a word right
//! where it was typed teaches it too. Only that field is read, never a password
//! field, and nothing from it is kept beyond the learned words.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

use tauri::{AppHandle, Emitter, Manager};

use crate::{AppState, dictation::Event, platform, text::DictionaryEntry};

/// Longest misheard or corrected phrase, in words.
const MAX_WORDS: usize = 3;
/// Longer texts aren't compared, which keeps the comparison cheap.
const MAX_COMPARED_WORDS: usize = 600;
/// More swaps than this in one edit is rewriting, not correcting.
const MAX_CORRECTIONS: usize = 3;
/// How alike two spellings must sound, from 0 (nothing alike) to 1 (the same).
const MIN_SIMILARITY: f64 = 0.5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correction {
    pub heard: String,
    pub meant: String,
}

/// The mishearings fixed between what was `typed` and the `edited` text.
pub fn corrections(typed: &str, edited: &str) -> Vec<Correction> {
    let before: Vec<&str> = typed.split_whitespace().collect();
    let after: Vec<&str> = edited.split_whitespace().collect();
    if before.is_empty()
        || after.is_empty()
        || before.len() > MAX_COMPARED_WORDS
        || after.len() > MAX_COMPARED_WORDS
    {
        return Vec::new();
    }
    let changes = changed_runs(&before, &after);
    let changed_words: usize = changes.iter().map(|(heard, _)| heard.len()).sum();
    // Changing more than half the words is rewriting, not correcting.
    if changes.len() > MAX_CORRECTIONS * 2 || changed_words * 2 > before.len() {
        return Vec::new();
    }
    let found: Vec<Correction> = changes
        .into_iter()
        .filter_map(|(heard, meant)| correction(&heard, &meant))
        .collect();
    if found.len() > MAX_CORRECTIONS {
        return Vec::new();
    }
    found
}

/// Runs of words replaced by other words, found by aligning the two texts on
/// the words they share. Words only added or only removed aren't included.
fn changed_runs<'a>(before: &[&'a str], after: &[&'a str]) -> Vec<(Vec<&'a str>, Vec<&'a str>)> {
    let a: Vec<String> = before.iter().map(|w| key(w)).collect();
    let b: Vec<String> = after.iter().map(|w| key(w)).collect();
    // Longest common subsequence table, filled from the end.
    let mut lcs = vec![vec![0u16; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut runs = Vec::new();
    let (mut removed, mut added) = (Vec::new(), Vec::new());
    let (mut i, mut j) = (0, 0);
    let mut flush = |removed: &mut Vec<&'a str>, added: &mut Vec<&'a str>| {
        if !removed.is_empty() && !added.is_empty() {
            runs.push((std::mem::take(removed), std::mem::take(added)));
        }
        removed.clear();
        added.clear();
    };
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            flush(&mut removed, &mut added);
            i += 1;
            j += 1;
        } else if j < b.len() && (i == a.len() || lcs[i][j + 1] >= lcs[i + 1][j]) {
            added.push(after[j]);
            j += 1;
        } else {
            removed.push(before[i]);
            i += 1;
        }
    }
    flush(&mut removed, &mut added);
    runs
}

fn correction(heard: &[&str], meant: &[&str]) -> Option<Correction> {
    if heard.len() > MAX_WORDS || meant.len() > MAX_WORDS {
        return None;
    }
    let heard = phrase(heard);
    let meant = phrase(meant);
    if heard.is_empty() || meant.is_empty() || key(&heard) == key(&meant) {
        return None;
    }
    // Replacing an everyday word everywhere would do more harm than good, and
    // a correction to everyday words is grammar, not a misheard name.
    let single_common = !heard.contains(' ') && is_common(&heard);
    let meant_common = meant.split_whitespace().all(is_common);
    if single_common || meant_common || !meant.chars().any(char::is_alphabetic) {
        return None;
    }
    (similarity(&sound(&heard), &sound(&meant)) >= MIN_SIMILARITY)
        .then_some(Correction { heard, meant })
}

/// Words joined with single spaces, without the punctuation at either end.
fn phrase(words: &[&str]) -> String {
    words
        .join(" ")
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_string()
}

/// A word as compared when aligning: lowercase, without surrounding punctuation.
fn key(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

/// A rough spelling of how text sounds, so "Cooper netties" can match
/// "Kubernetes": letters only, hard c/q as k, ph as f, z as s, no doubled letters.
fn sound(text: &str) -> Vec<char> {
    let lower: String = text
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();
    let lower = lower.replace("ph", "f").replace("ck", "k");
    let mut out: Vec<char> = Vec::with_capacity(lower.len());
    for c in lower.chars() {
        let c = match c {
            'c' | 'q' => 'k',
            'z' => 's',
            _ => c,
        };
        if out.last() != Some(&c) {
            out.push(c);
        }
    }
    out
}

/// 1 minus the edit distance relative to the longer of the two.
fn similarity(a: &[char], b: &[char]) -> f64 {
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1.0;
    }
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitute = diagonal + usize::from(ca != cb);
            diagonal = row[j + 1];
            row[j + 1] = substitute.min(row[j] + 1).min(row[j + 1] + 1);
        }
    }
    1.0 - row[b.len()] as f64 / longest as f64
}

fn is_common(word: &str) -> bool {
    let word = key(word);
    COMMON_WORDS.split_whitespace().any(|common| common == word)
}

/// Adds a dictionary entry for each correction not already covered, and
/// returns the ones added.
pub fn add_to_dictionary(
    dictionary: &mut Vec<DictionaryEntry>,
    corrections: &[Correction],
) -> Vec<DictionaryEntry> {
    let mut added = Vec::new();
    for correction in corrections {
        let known = dictionary
            .iter()
            .any(|entry| entry.trigger.trim().eq_ignore_ascii_case(&correction.heard));
        if known {
            continue;
        }
        let entry = DictionaryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            trigger: correction.heard.clone(),
            replacement: correction.meant.clone(),
            is_enabled: true,
            match_whole_word: true,
            learned: true,
        };
        dictionary.push(entry.clone());
        added.push(entry);
    }
    added
}

/// Learns from a transcript the user edited, if learning is switched on, and
/// saves the new entries. Returns them so the caller can offer to undo them.
pub fn learn(app: &AppHandle, typed: &str, edited: &str) -> Vec<DictionaryEntry> {
    let state = app.state::<AppState>();
    let mut settings = state.settings();
    if !settings.learn_corrections {
        return Vec::new();
    }
    let found = corrections(typed, edited);
    let added = add_to_dictionary(&mut settings.dictionary, &found);
    if added.is_empty() {
        return added;
    }
    if let Err(e) = state.replace_settings(settings) {
        eprintln!("[learn] couldn't save learned words: {e}");
        return Vec::new();
    }
    let _ = app.emit("settings-changed", ());
    added
}

/// Learns from a correction made in another app. The pill says what was
/// learned, as the main window is likely hidden; if it's open, it offers Undo.
pub fn learn_from_app(app: &AppHandle, typed: &str, edited: &str) {
    let added = learn(app, typed, edited);
    if !added.is_empty() {
        let _ = app.emit("words-learned", &added);
        let words = added.into_iter().map(|e| e.replacement).collect();
        app.state::<AppState>()
            .controller
            .send(Event::Learned { words });
    }
}

/// How long after typing the field is watched, and how often it's read.
const WATCH_FOR: Duration = Duration::from_secs(45);
const WATCH_EVERY: Duration = Duration::from_millis(800);
/// Time for pasted text to land before the field is first read.
const SETTLE: Duration = Duration::from_millis(900);
/// Fields holding more text than this, like long documents, aren't watched.
const MAX_FIELD_BYTES: usize = 200_000;
/// How much of the text around the typed text is used to find it again.
const ANCHOR_CHARS: usize = 40;

/// Bumped by each watch, so a newer dictation ends the previous watch.
static WATCH: AtomicU64 = AtomicU64::new(0);

/// After `typed` was put into another app, watches the focused field for a
/// while and learns from any corrections made to it. Does nothing if learning
/// is off or the field can't be read, as on Linux.
pub fn watch_for_corrections(app: &AppHandle, typed: &str) {
    let typed = typed.trim();
    if typed.is_empty() || !app.state::<AppState>().settings().learn_corrections {
        return;
    }
    let generation = WATCH.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    let typed = typed.to_string();
    let spawned = thread::Builder::new()
        .name("correction-watch".into())
        .spawn(move || {
            if let Some(edited) = watch(&typed, generation) {
                learn_from_app(&app, &typed, &edited);
            }
        });
    if let Err(e) = spawned {
        eprintln!("[learn] couldn't watch for corrections: {e}");
    }
}

/// The typed text as it reads when the user moves on: when the field loses
/// focus, a newer dictation starts, or the watch times out.
fn watch(typed: &str, generation: u64) -> Option<String> {
    thread::sleep(SETTLE);
    let field = platform::FocusedField::current()?;
    let mut latest = field.text().filter(|t| t.len() <= MAX_FIELD_BYTES)?;
    let span = Span::find(&latest, typed)?;
    let started = Instant::now();
    while started.elapsed() < WATCH_FOR && WATCH.load(Ordering::SeqCst) == generation {
        thread::sleep(WATCH_EVERY);
        if !field.is_focused() {
            break;
        }
        match field.text() {
            Some(text) if text.len() <= MAX_FIELD_BYTES => latest = text,
            _ => break,
        }
    }
    span.edited(&latest).map(str::to_string)
}

/// Where typed text sits in a field, remembered by the text either side so it
/// can be found again after the user edits it or the text around it.
#[derive(Debug)]
struct Span {
    before: String,
    after: String,
    /// Byte offset of the typed text, used to pick between repeated anchors.
    start: usize,
    len: usize,
}

impl Span {
    /// The last place `typed` appears in `text`, the likeliest to be the one just typed.
    fn find(text: &str, typed: &str) -> Option<Self> {
        let start = text.rfind(typed)?;
        let end = start + typed.len();
        Some(Self {
            before: last_chars(&text[..start], ANCHOR_CHARS).to_string(),
            after: first_chars(&text[end..], ANCHOR_CHARS).to_string(),
            start,
            len: typed.len(),
        })
    }

    /// The typed text as it reads in `text` now, if it can still be found and
    /// hasn't grown into something much longer.
    fn edited<'a>(&self, text: &'a str) -> Option<&'a str> {
        let start = if self.before.is_empty() {
            0
        } else {
            let near = self.start.saturating_sub(self.before.len());
            text.match_indices(self.before.as_str())
                .map(|(at, _)| at)
                .min_by_key(|at| at.abs_diff(near))?
                + self.before.len()
        };
        let end = if self.after.is_empty() {
            text.len()
        } else {
            start + text[start..].find(self.after.as_str())?
        };
        let edited = &text[start..end];
        (edited.len() <= self.len * 3 + 200).then_some(edited)
    }
}

fn last_chars(text: &str, count: usize) -> &str {
    text.char_indices()
        .rev()
        .nth(count.saturating_sub(1))
        .map_or(text, |(at, _)| &text[at..])
}

fn first_chars(text: &str, count: usize) -> &str {
    text.char_indices()
        .nth(count)
        .map_or(text, |(at, _)| &text[..at])
}

/// Roughly the 350 most frequent English words.
const COMMON_WORDS: &str = "
a about above after again against all almost alone along already also although always am among an and
another any anyone anything are around as ask asked at away back be became because become been before
began behind being below best better between big both bring brought but buy by call came can cannot car
case change child children city close come could country course cut day did different do does doing
done don't down during each early end enough even ever every eye face fact far feel few find first
for found four free friend from full gave get give go going good got great group had hand hard has have
having he head hear heard help her here hers herself high him himself his home hour house how however i
idea if important in into is it its itself just keep kind knew know large last late later lead least
leave left less let life light like line little live long look made make man many may me mean men
might mind more most mother move much must my myself name near need never new next night no not
nothing now number of off often old on once one only open or order other our ours ourselves out over
own part people perhaps place play point put question quite rather read real really right room run
said same saw say school see seem seen set she should show side since small so some something sometimes
soon sound start still stop such sure take talk tell than that the their theirs them themselves then
there these they thing things think this those though thought three through time to today together told
too took toward try turn two under until up upon us use used very want was water way we well went were
what when where whether which while white who whole whom whose why will with within without word work
world would write year yes yet you young your yours yourself yourselves its it's i'm you're we're
they're that's there's what's let's isn't aren't wasn't weren't won't wouldn't can't couldn't
shouldn't didn't doesn't haven't hasn't hadn't okay ok yeah hey hi hello thanks thank please sorry
";

#[cfg(test)]
mod tests {
    use super::*;

    fn found(typed: &str, edited: &str) -> Vec<(String, String)> {
        corrections(typed, edited)
            .into_iter()
            .map(|c| (c.heard, c.meant))
            .collect()
    }

    #[test]
    fn learns_misheard_names_and_terms() {
        assert_eq!(
            found(
                "Karen said the Cooper netties deploy failed.",
                "Karan said the Kubernetes deploy failed."
            ),
            [
                ("Karen".into(), "Karan".into()),
                ("Cooper netties".into(), "Kubernetes".into())
            ]
        );
        assert_eq!(
            found("ping jon on slack", "ping Jon on Slack"),
            Vec::<(String, String)>::new(),
            "capitalization alone isn't a mishearing"
        );
    }

    #[test]
    fn ignores_grammar_rewrites_and_unrelated_words() {
        let none = Vec::<(String, String)>::new();
        assert_eq!(found("I put it over their", "I put it over there"), none);
        assert_eq!(
            found("send the report today", "send the summary today"),
            none,
            "summary doesn't sound like report"
        );
        assert_eq!(
            found(
                "let's meet tomorrow to talk about the plan",
                "can we push the planning call to Friday instead"
            ),
            none,
            "rewritten sentences teach nothing"
        );
        assert_eq!(
            found("the deploy failed", "the deploy failed and I fixed it"),
            none,
            "added words are not corrections"
        );
        assert_eq!(found("", "anything"), none);
    }

    #[test]
    fn sound_alike_handles_spelling_differences() {
        assert!(similarity(&sound("Fone"), &sound("phone")) >= 0.99);
        assert!(similarity(&sound("cooper netties"), &sound("Kubernetes")) >= MIN_SIMILARITY);
        assert!(similarity(&sound("report"), &sound("summary")) < MIN_SIMILARITY);
        assert_eq!(similarity(&[], &[]), 1.0);
    }

    #[test]
    fn typed_text_is_found_again_after_edits_in_and_around_it() {
        let typed = "Karen said the deploy failed.";
        let field = format!("Hi team,\n\n{typed}\n\nThanks");
        let span = Span::find(&field, typed).unwrap();

        // Corrected in place.
        let now = field.replace("Karen", "Karan");
        assert_eq!(span.edited(&now), Some("Karan said the deploy failed."));
        // More written above it, and the sign-off changed further down.
        let now = format!("Quick update.\n{}", now.replace("Thanks", "Thanks!"));
        assert_eq!(span.edited(&now), Some("Karan said the deploy failed."));
        // The typed text was deleted along with the greeting: nothing to compare.
        assert_eq!(span.edited("Thanks"), None);
    }

    #[test]
    fn text_typed_at_the_start_or_end_of_a_field_is_found() {
        let span = Span::find("Karen said hi", "Karen said hi").unwrap();
        assert_eq!(span.edited("Karan said hi"), Some("Karan said hi"));
        let span = Span::find("Note: Karen said hi", "Karen said hi").unwrap();
        assert_eq!(
            span.edited("Note: Karan said hi, and more"),
            Some("Karan said hi, and more")
        );
        assert!(Span::find("something else", "Karen said hi").is_none());
        assert_eq!(last_chars("héllo", 3), "llo");
        assert_eq!(first_chars("héllo", 2), "hé");
        assert_eq!(first_chars("hi", 5), "hi");
    }

    #[test]
    fn new_entries_are_added_once_and_marked_learned() {
        let mut dictionary = Vec::new();
        let fixes = [Correction {
            heard: "Karen".into(),
            meant: "Karan".into(),
        }];
        let added = add_to_dictionary(&mut dictionary, &fixes);
        assert_eq!(added.len(), 1);
        assert!(added[0].learned && added[0].is_enabled && added[0].match_whole_word);
        assert_eq!(
            (added[0].trigger.as_str(), added[0].replacement.as_str()),
            ("Karen", "Karan")
        );
        assert!(add_to_dictionary(&mut dictionary, &fixes).is_empty());
        assert_eq!(dictionary.len(), 1);
    }
}
