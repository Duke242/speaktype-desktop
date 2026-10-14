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
//! A name that can't safely replace what was heard, because that's an everyday
//! word ("more" fixed to "Moa") or sounds nothing like it, is learned as itself:
//! the entry writes it with the right capitals and hints it to Whisper.
//!
//! Capitals are learned too: "github" fixed to "GitHub", or "new york" to
//! "New York", becomes an entry that writes it that way from then on. Only
//! capitals added mid-sentence count; one added to the first word of a
//! sentence is grammar, and so is changing the case of everyday words.
//!
//! On macOS and Windows, the field a dictation was typed into is watched for a
//! couple of minutes afterwards (`watch_for_corrections`), so fixing a word
//! right where it was typed teaches it too. Only that field is read, never a
//! password field, and nothing from it is kept beyond the learned words.

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
    let (changes, recased) = changed_runs(&before, &after);
    let changed_words: usize = changes
        .iter()
        .map(|run| run.heard.len())
        .chain(recased.iter().map(|run| run.heard.len()))
        .sum();
    // Changing more than half the words is rewriting, not correcting. A dictation
    // of a few words, like a name on its own, can be corrected entirely.
    let rewritten = before.len() > MAX_WORDS && changed_words * 2 > before.len();
    if changes.len() + recased.len() > MAX_CORRECTIONS * 2 || rewritten {
        return Vec::new();
    }
    let found: Vec<Correction> = changes
        .into_iter()
        .filter_map(|run| correction(&before, &run))
        .chain(recased.iter().filter_map(|run| recasing(&before, run)))
        .collect();
    if found.len() > MAX_CORRECTIONS {
        return Vec::new();
    }
    found
}

/// Neighbouring words kept but written with different capitals.
#[derive(Debug)]
struct Recased<'a> {
    /// Where the run starts in the typed words.
    start: usize,
    heard: Vec<&'a str>,
    meant: Vec<&'a str>,
}

/// Typed words replaced by other words.
#[derive(Debug)]
struct Replaced<'a> {
    /// Where the run starts in the typed words.
    start: usize,
    heard: Vec<&'a str>,
    meant: Vec<&'a str>,
}

type Runs<'a> = (Vec<Replaced<'a>>, Vec<Recased<'a>>);

/// Runs of words replaced by other words, found by aligning the two texts on
/// the words they share, and runs of shared words whose capitals changed.
/// Words only added or only removed aren't included.
fn changed_runs<'a>(before: &[&'a str], after: &[&'a str]) -> Runs<'a> {
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
    let mut recased: Vec<Recased<'a>> = Vec::new();
    let (mut removed, mut added) = (Vec::new(), Vec::new());
    let (mut i, mut j) = (0, 0);
    let mut flush = |removed: &mut Vec<&'a str>, added: &mut Vec<&'a str>, end: usize| {
        if !removed.is_empty() && !added.is_empty() {
            runs.push(Replaced {
                start: end - removed.len(),
                heard: std::mem::take(removed),
                meant: std::mem::take(added),
            });
        }
        removed.clear();
        added.clear();
    };
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            flush(&mut removed, &mut added, i);
            if phrase(&[before[i]]) != phrase(&[after[j]]) {
                match recased.last_mut() {
                    Some(run) if run.start + run.heard.len() == i => {
                        run.heard.push(before[i]);
                        run.meant.push(after[j]);
                    }
                    _ => recased.push(Recased {
                        start: i,
                        heard: vec![before[i]],
                        meant: vec![after[j]],
                    }),
                }
            }
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
    flush(&mut removed, &mut added, i);
    // A run of recased words ends where a word kept its case or was changed.
    let mut joined: Vec<Recased<'a>> = Vec::new();
    for run in recased {
        match joined.last_mut() {
            Some(last)
                if last.start + last.heard.len() == run.start
                    && last.heard.len() + run.heard.len() <= MAX_WORDS => {}
            _ => {
                joined.push(run);
                continue;
            }
        }
        if let Some(last) = joined.last_mut() {
            last.heard.extend(run.heard);
            last.meant.extend(run.meant);
        }
    }
    (runs, joined)
}

fn correction(typed: &[&str], run: &Replaced) -> Option<Correction> {
    if run.heard.len() > MAX_WORDS || run.meant.len() > MAX_WORDS {
        return None;
    }
    let heard = phrase(&run.heard);
    let meant = phrase(&run.meant);
    if heard.is_empty() || meant.is_empty() || key(&heard) == key(&meant) {
        return None;
    }
    // A correction to everyday words is grammar, not a misheard name.
    if meant.split_whitespace().all(is_common)
        || is_inflection(&key(&heard), &key(&meant))
        || !meant.chars().any(char::is_alphabetic)
    {
        return None;
    }
    // Replacing an everyday word everywhere would do more harm than good, and
    // so would replacing something that sounds nothing like the fix.
    let single_common = !heard.contains(' ') && is_common(&heard);
    if !single_common && similarity(&sound(&heard), &sound(&meant)) >= MIN_SIMILARITY {
        return Some(Correction { heard, meant });
    }
    // A name is still worth knowing, learned as itself. At the start of a
    // sentence, a capital the heard word already had says nothing.
    let sentence_start = run.start == 0 || ends_sentence(typed[run.start - 1]);
    let capital_given = sentence_start && heard.chars().next().is_some_and(char::is_uppercase);
    is_name(&meant, capital_given).then(|| Correction {
        heard: meant.clone(),
        meant,
    })
}

/// Whether `text` has capitals of its own, beyond its first one when that
/// was given by grammar.
fn is_name(text: &str, first_given: bool) -> bool {
    let capitals = text.chars().filter(|c| c.is_uppercase()).count();
    let first = text.chars().next().is_some_and(char::is_uppercase);
    capitals > usize::from(first_given && first)
}

/// Whether a sentence ends with this word.
fn ends_sentence(word: &str) -> bool {
    word.trim_end_matches(['"', '\'', ')', '”', '’'])
        .ends_with(['.', '!', '?', ':'])
}

/// A run of words written with new capitals, if it's worth learning: capitals
/// added to a name or term mid-sentence, not to the first word of a sentence,
/// and not to everyday words.
fn recasing(typed: &[&str], run: &Recased) -> Option<Correction> {
    if run.heard.len() > MAX_WORDS {
        return None;
    }
    let heard = phrase(&run.heard);
    let meant = phrase(&run.meant);
    if heard.is_empty() || heard == meant || !meant.chars().any(char::is_uppercase) {
        return None;
    }
    if meant.split_whitespace().all(is_common) {
        return None;
    }
    let sentence_start = run.start == 0 || ends_sentence(typed[run.start - 1]);
    if sentence_start && meant == capitalize(&heard) {
        return None;
    }
    Some(Correction { heard, meant })
}

/// `text` with its first letter in capitals.
fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// Whether one word is the other with an ending added, like "meeting" and
/// "meetings" or "fix" and "fixed". That's grammar for this sentence, and
/// replacing the word everywhere would break others.
fn is_inflection(a: &str, b: &str) -> bool {
    const ENDINGS: &[&str] = &["s", "es", "'s", "d", "ed", "ing", "er", "est", "ly"];
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    short.chars().count() >= 3
        && long
            .strip_prefix(short)
            .is_some_and(|ending| ENDINGS.contains(&ending))
}

/// Words joined with single spaces, without the punctuation at either end.
fn phrase(words: &[&str]) -> String {
    words
        .join(" ")
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_string()
}

/// A word as compared when aligning: lowercase, without surrounding
/// punctuation, and with curly apostrophes straightened, as editors curl them.
fn key(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
        .replace(['‘', '’'], "'")
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
/// returns the ones added. A correction that reverses a learned entry replaces
/// it, as the latest correction wins; one that reverses the user's own entry is
/// skipped, as theirs does.
pub fn add_to_dictionary(
    dictionary: &mut Vec<DictionaryEntry>,
    corrections: &[Correction],
) -> Vec<DictionaryEntry> {
    let mut added = Vec::new();
    for correction in corrections {
        let same = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b);
        let reverses = |entry: &DictionaryEntry| {
            same(&entry.trigger, &correction.meant) && same(&entry.replacement, &correction.heard)
        };
        // New capitals for a word already learned: the latest wins.
        if let Some(entry) = dictionary.iter_mut().find(|entry| {
            entry.learned
                && same(&entry.trigger, &correction.heard)
                && same(&entry.replacement, &correction.meant)
                && entry.replacement.trim() != correction.meant
        }) {
            entry.replacement = correction.meant.clone();
            added.push(entry.clone());
            continue;
        }
        // A name learned as itself is already known if anything writes it,
        // and gives way to a later fix of that name.
        let hint = correction.heard == correction.meant;
        let learned_hint = |entry: &DictionaryEntry| {
            entry.learned
                && entry.trigger.trim() == entry.replacement.trim()
                && same(&entry.trigger, &correction.heard)
        };
        let known = dictionary.iter().any(|entry| {
            (same(&entry.trigger, &correction.heard) && (hint || !learned_hint(entry)))
                || (hint && same(&entry.replacement, &correction.meant))
                || (!entry.learned && reverses(entry))
        });
        if known {
            continue;
        }
        dictionary.retain(|entry| !(entry.learned && reverses(entry)) && !learned_hint(entry));
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
    let found = corrections(typed, edited);
    if found.is_empty() {
        return Vec::new();
    }
    let updated = app.state::<AppState>().update_settings(|settings| {
        if settings.learn_corrections {
            add_to_dictionary(&mut settings.dictionary, &found)
        } else {
            Vec::new()
        }
    });
    match updated {
        Ok(added) => {
            if !added.is_empty() {
                let _ = app.emit("settings-changed", ());
            }
            added
        }
        Err(e) => {
            eprintln!("[learn] couldn't save learned words: {e}");
            Vec::new()
        }
    }
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
const WATCH_FOR: Duration = Duration::from_secs(120);
const WATCH_EVERY: Duration = Duration::from_millis(800);
/// Time for pasted text to land before the field is first read.
const SETTLE: Duration = Duration::from_millis(900);
/// Chrome and Electron apps build their accessibility tree when first asked,
/// so the field is read a few more times, this far apart, before giving up.
const FIELD_RETRIES: u32 = 4;
const FIELD_RETRY_EVERY: Duration = Duration::from_millis(300);
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

/// The typed text as last seen settled: unchanged across two reads, so a word
/// half retyped isn't learned. Watching goes on when the field loses focus, as
/// clicking around a web page moves focus without leaving it. It ends when a
/// newer dictation starts, time runs out, or the typed text is gone, as when a
/// message is sent; then the last version seen counts, as it was sent as meant,
/// and nothing typed in the field afterwards is compared.
fn watch(typed: &str, generation: u64) -> Option<String> {
    thread::sleep(SETTLE);
    let (field, text) = read_field()?;
    let span = Span::find(&text, typed)?;
    let mut previous = span.edited(&text)?.to_string();
    let mut settled = None;
    let started = Instant::now();
    while started.elapsed() < WATCH_FOR && WATCH.load(Ordering::SeqCst) == generation {
        thread::sleep(WATCH_EVERY);
        let Some(text) = field.text().filter(|t| t.len() <= MAX_FIELD_BYTES) else {
            break;
        };
        match span
            .edited(&text)
            .filter(|edited| !edited.trim().is_empty())
        {
            Some(edited) if edited == previous => settled = Some(previous.clone()),
            Some(edited) => previous = edited.to_string(),
            None => return Some(previous),
        }
    }
    settled
}

/// The focused field and its text, given a moment to become readable.
fn read_field() -> Option<(platform::FocusedField, String)> {
    for attempt in 0..FIELD_RETRIES {
        if attempt > 0 {
            thread::sleep(FIELD_RETRY_EVERY);
        }
        if let Some(field) = platform::FocusedField::current()
            && let Some(text) = field.text()
        {
            return (text.len() <= MAX_FIELD_BYTES).then_some((field, text));
        }
    }
    None
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
    /// The last place `typed` appears in `text`, the likeliest to be the one
    /// just typed. Spaces, line breaks and quotes may differ, as editors change
    /// them when text is pasted.
    fn find(text: &str, typed: &str) -> Option<Self> {
        let (loose_text, offsets) = loosened(text);
        let (loose_typed, _) = loosened(typed.trim());
        if loose_typed.is_empty() {
            return None;
        }
        let at = loose_text.rfind(&loose_typed)?;
        let start = offsets[at];
        let end = offsets
            .get(at + loose_typed.len())
            .copied()
            .unwrap_or(text.len());
        Some(Self {
            before: last_chars(&text[..start], ANCHOR_CHARS).to_string(),
            after: first_chars(&text[end..], ANCHOR_CHARS).to_string(),
            start,
            len: end - start,
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

/// `text` with each run of whitespace as one space and curly quotes as
/// straight ones, and for each of its bytes, where it came from in `text`.
fn loosened(text: &str) -> (String, Vec<usize>) {
    let mut out = String::with_capacity(text.len());
    let mut offsets = Vec::with_capacity(text.len());
    let mut in_space = false;
    for (at, c) in text.char_indices() {
        let c = match c {
            c if c.is_whitespace() => ' ',
            '‘' | '’' => '\'',
            '“' | '”' => '"',
            c => c,
        };
        if c == ' ' && in_space {
            continue;
        }
        in_space = c == ' ';
        out.push(c);
        offsets.extend(std::iter::repeat_n(at, c.len_utf8()));
    }
    (out, offsets)
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
        // A name dictated on its own is the commonest thing to correct.
        assert_eq!(
            found("Karen.", "Karan."),
            [("Karen".into(), "Karan".into())]
        );
        assert_eq!(
            found("Cooper netties", "Kubernetes"),
            [("Cooper netties".into(), "Kubernetes".into())]
        );
    }

    #[test]
    fn learns_names_and_terms_given_capitals() {
        let none = Vec::<(String, String)>::new();
        assert_eq!(
            found("ping jon on slack", "ping Jon on Slack"),
            [
                ("jon".into(), "Jon".into()),
                ("slack".into(), "Slack".into())
            ]
        );
        assert_eq!(
            found(
                "I pushed it to github, then the iphone build.",
                "I pushed it to GitHub, then the iPhone build."
            ),
            [
                ("github".into(), "GitHub".into()),
                ("iphone".into(), "iPhone".into())
            ]
        );
        assert_eq!(
            found("call the api", "call the API"),
            [("api".into(), "API".into())]
        );
        // Neighbouring words make one entry.
        assert_eq!(
            found(
                "we flew to new york on monday",
                "we flew to New York on Monday"
            ),
            [
                ("new york".into(), "New York".into()),
                ("monday".into(), "Monday".into())
            ]
        );
        // A name on its own, or at the start, with more than the first letter changed.
        assert_eq!(
            found("github", "GitHub"),
            [("github".into(), "GitHub".into())]
        );
        assert_eq!(
            found("Github is down", "GitHub is down"),
            [("Github".into(), "GitHub".into())]
        );
        // The first word of a sentence is grammar.
        assert_eq!(
            found("thanks. karan will call", "thanks. Karan will call"),
            none
        );
        assert_eq!(found("karan will call", "Karan will call"), none);
        // Everyday words, and capitals taken away, teach nothing.
        assert_eq!(found("go to the house", "go to The House"), none);
        assert_eq!(found("I use Bash daily", "I use bash daily"), none);
        // Title-casing a whole sentence is rewriting.
        assert_eq!(
            found(
                "quarterly roadmap review notes",
                "Quarterly Roadmap Review Notes"
            ),
            none
        );
    }

    #[test]
    fn names_that_cant_replace_what_was_heard_are_learned_as_themselves() {
        let none = Vec::<(String, String)>::new();
        // "more" is everyday, so it isn't replaced; "Moa" is still learned.
        assert_eq!(
            found("I told more about it", "I told Moa about it"),
            [("Moa".into(), "Moa".into())]
        );
        assert_eq!(found("more", "Moa"), [("Moa".into(), "Moa".into())]);
        // A name that sounds nothing like what was heard.
        assert_eq!(
            found("ask Jim about the launch", "ask Siobhan about the launch"),
            [("Siobhan".into(), "Siobhan".into())]
        );
        // A capital that only starts a sentence doesn't make a name.
        assert_eq!(found("Thanks!", "Cheers!"), none);
        assert_eq!(
            found(
                "it went well. More is coming soon",
                "it went well. Lots is coming soon"
            ),
            none
        );
    }

    #[test]
    fn curly_apostrophes_are_not_corrections() {
        assert!(found("don't ping Karen", "don’t ping Karen").is_empty());
    }

    #[test]
    fn names_learned_as_themselves_are_added_once() {
        let fix = |heard: &str, meant: &str| Correction {
            heard: heard.into(),
            meant: meant.into(),
        };
        let mut dictionary = Vec::new();
        add_to_dictionary(&mut dictionary, &[fix("mower", "Moa")]);
        assert!(add_to_dictionary(&mut dictionary, &[fix("Moa", "Moa")]).is_empty());
        assert_eq!(dictionary.len(), 1);

        // A later fix of a name learned as itself replaces it.
        let mut dictionary = Vec::new();
        add_to_dictionary(&mut dictionary, &[fix("Karen", "Karen")]);
        assert_eq!(
            add_to_dictionary(&mut dictionary, &[fix("Karen", "Karan")]).len(),
            1
        );
        assert_eq!(dictionary.len(), 1);
        assert_eq!(dictionary[0].replacement, "Karan");
    }

    #[test]
    fn new_capitals_replace_learned_ones() {
        let fix = |heard: &str, meant: &str| Correction {
            heard: heard.into(),
            meant: meant.into(),
        };
        let mut dictionary = Vec::new();
        add_to_dictionary(&mut dictionary, &[fix("github", "Github")]);
        let added = add_to_dictionary(&mut dictionary, &[fix("github", "GitHub")]);
        assert_eq!(added.len(), 1);
        assert_eq!(dictionary.len(), 1);
        assert_eq!(dictionary[0].replacement, "GitHub");
        assert!(add_to_dictionary(&mut dictionary, &[fix("github", "GitHub")]).is_empty());
    }

    #[test]
    fn word_endings_are_grammar_not_mishearings() {
        let none = Vec::<(String, String)>::new();
        assert_eq!(
            found("we fix the meeting notes", "we fixed the meetings notes"),
            none
        );
        assert_eq!(found("the roadmaps slide", "the roadmap slide"), none);
        assert!(is_inflection("meeting", "meetings") && is_inflection("fixed", "fix"));
        assert!(!is_inflection("sara", "sarah") && !is_inflection("go", "goes"));
    }

    #[test]
    fn the_latest_correction_replaces_a_learned_one_but_not_the_users_own() {
        let fix = |heard: &str, meant: &str| Correction {
            heard: heard.into(),
            meant: meant.into(),
        };
        let mut dictionary = Vec::new();
        add_to_dictionary(&mut dictionary, &[fix("Karen", "Karan")]);
        let added = add_to_dictionary(&mut dictionary, &[fix("Karan", "Karen")]);
        assert_eq!(added.len(), 1);
        assert_eq!(dictionary.len(), 1, "the reversed learned entry is gone");
        assert_eq!(dictionary[0].trigger, "Karan");

        dictionary[0].learned = false;
        assert!(add_to_dictionary(&mut dictionary, &[fix("Karen", "Karan")]).is_empty());
        assert_eq!(dictionary.len(), 1, "the user's own entry wins");
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
        // Editors turn spaces into non-breaking ones and straighten or curl quotes.
        let span = Span::find("Hi\u{a0}there,\n\nKaren said “hi”", "Karen said \"hi\"").unwrap();
        assert_eq!(
            span.edited("Hi\u{a0}there,\n\nKaran said “hi”"),
            Some("Karan said “hi”")
        );
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
