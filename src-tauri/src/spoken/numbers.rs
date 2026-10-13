//! Spoken numbers, written as digits where people would type them: "twenty
//! five" is 25, "march third twenty twenty six" is March 3, 2026, "five
//! percent" is 5%, "ten dollars" is $10 and "three thirty pm" is 3:30 PM.
//!
//! Small numbers in prose stay words ("one of them"), as do runs of number
//! words that don't make a single number ("one two three"), since they may be
//! read out digit by digit.

use super::{Word, capitalize, gap, spaced, splice, words};

const ONES: [&str; 10] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
];
const TEENS: [&str; 10] = [
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const TENS: [&str; 8] = [
    "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];
const ORDINALS: [&str; 19] = [
    "first",
    "second",
    "third",
    "fourth",
    "fifth",
    "sixth",
    "seventh",
    "eighth",
    "ninth",
    "tenth",
    "eleventh",
    "twelfth",
    "thirteenth",
    "fourteenth",
    "fifteenth",
    "sixteenth",
    "seventeenth",
    "eighteenth",
    "nineteenth",
];
const MONTHS: [&str; 12] = [
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
const AM_PM: [&str; 4] = ["am", "pm", "a.m", "p.m"];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Piece {
    /// 0 to 9.
    Digit(u64),
    /// 10 to 19.
    Teen(u64),
    /// 20, 30 … 90.
    Tens(u64),
    /// A hyphenated 21 to 99, like "twenty-five".
    Compound(u64),
    Hundred,
    /// Thousand, million or billion.
    Scale(u64),
    /// "Oh", as in "three oh five" or "twenty oh nine".
    Oh,
}

use Piece::*;

fn position(list: &[&str], word: &str) -> Option<u64> {
    list.iter().position(|w| *w == word).map(|i| i as u64)
}

fn piece(word: &str) -> Option<Piece> {
    if let Some((tens, ones)) = word.split_once('-') {
        return match (piece(tens), piece(ones)) {
            (Some(Tens(t)), Some(Digit(d))) if d > 0 => Some(Compound(t + d)),
            _ => None,
        };
    }
    if let Some(n) = position(&ONES, word) {
        return Some(Digit(n));
    }
    if let Some(n) = position(&TEENS, word) {
        return Some(Teen(10 + n));
    }
    if let Some(n) = position(&TENS, word) {
        return Some(Tens(20 + n * 10));
    }
    match word {
        "hundred" => Some(Hundred),
        "thousand" => Some(Scale(1_000)),
        "million" => Some(Scale(1_000_000)),
        "billion" => Some(Scale(1_000_000_000)),
        "oh" => Some(Oh),
        _ => None,
    }
}

/// Whether `word`, in lowercase, is part of a spoken number.
pub(super) fn is_number_word(word: &str) -> bool {
    piece(word).is_some_and(|p| p != Oh)
}

/// 1 for "first" up to 31 for "thirty-first".
fn ordinal(word: &str) -> Option<u64> {
    if let Some((tens, ones)) = word.split_once('-') {
        return match (piece(tens), ordinal(ones)) {
            (Some(Tens(t)), Some(o)) if o < 10 => Some(t + o),
            _ => None,
        };
    }
    match word {
        "twentieth" => Some(20),
        "thirtieth" => Some(30),
        _ => position(&ORDINALS, word).map(|i| i + 1),
    }
}

/// The value of a whole run of number words, or `None` when they don't make
/// one number, like "one two" or "twenty thirty".
fn value(pieces: &[Piece]) -> Option<u64> {
    let mut total = 0;
    let mut group = 0;
    let mut last: Option<Piece> = None;
    let mut last_scale = u64::MAX;
    for &p in pieces {
        let after_group_start = matches!(last, None | Some(Hundred | Scale(_)));
        match p {
            Digit(n) if after_group_start || (matches!(last, Some(Tens(_))) && n > 0) => group += n,
            Teen(n) | Tens(n) | Compound(n) if after_group_start => group += n,
            Hundred if (1..100).contains(&group) && last.is_some_and(|l| l != Hundred) => {
                group *= 100
            }
            Scale(s) if group > 0 && s < last_scale => {
                total += group * s;
                group = 0;
                last_scale = s;
            }
            _ => return None,
        }
        last = Some(p);
    }
    Some(total + group)
}

/// A year said in two halves: "nineteen eighty four", "twenty twenty six",
/// "twenty oh five".
fn year(pieces: &[Piece]) -> Option<u64> {
    let century = match pieces.first()? {
        Teen(n @ 18..=19) | Tens(n @ 20) => *n,
        _ => return None,
    };
    let rest = match pieces[1..] {
        [Teen(n) | Tens(n) | Compound(n)] => n,
        [Tens(t), Digit(d)] if d > 0 => t + d,
        [Oh, Digit(d)] if d > 0 => d,
        _ => return None,
    };
    Some(century * 100 + rest)
}

/// Number words starting at a word, as long as only spaces separate them. An
/// "and" can join them after "hundred" or "thousand".
struct Run {
    last: usize,
    pieces: Vec<Piece>,
}

fn run_at(text: &str, words: &[Word], first: usize) -> Option<Run> {
    let mut pieces = Vec::new();
    let mut last = first;
    let mut i = first;
    while let Some(word) = words.get(i) {
        if i > first && !spaced(text, &words[i - 1], word) {
            break;
        }
        if let Some(p) = piece(&word.lower()) {
            pieces.push(p);
            last = i;
            i += 1;
        } else if word.is("and")
            && matches!(pieces.last(), Some(Hundred | Scale(_)))
            && words
                .get(i + 1)
                .is_some_and(|next| spaced(text, word, next) && is_number_word(&next.lower()))
        {
            i += 1;
        } else {
            break;
        }
    }
    (pieces.first().is_some_and(|p| *p != Oh)).then_some(Run { last, pieces })
}

/// Digits as typed, with commas from 1,000 up, except for likely years.
fn digits(value: u64) -> String {
    let plain = value.to_string();
    if value < 1000 || (1900..=2100).contains(&value) {
        return plain;
    }
    let mut out = String::new();
    for (i, c) in plain.chars().enumerate() {
        if i > 0 && (plain.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// "5 million" rather than "5,000,000".
fn render(pieces: &[Piece], value: u64) -> String {
    match pieces.last() {
        Some(Scale(s)) if *s >= 1_000_000 && value.is_multiple_of(*s) => {
            let name = if *s == 1_000_000 {
                "million"
            } else {
                "billion"
            };
            format!("{} {name}", digits(value / s))
        }
        _ => digits(value),
    }
}

fn ordinal_suffix(n: u64) -> &'static str {
    match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    }
}

/// Digits already written by the transcriber: "3", "3:30", "2.5".
fn written_number(word: &Word) -> bool {
    word.text.starts_with(|c: char| c.is_ascii_digit())
        && word
            .text
            .chars()
            .all(|c| c.is_ascii_digit() || ".:,".contains(c))
}

/// A byte range, what replaces it, and the index of the first word after it.
type Edit = (usize, usize, String);

pub fn apply(text: &str) -> String {
    let words = words(text);
    let mut edits: Vec<Edit> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let Some((next, edit)) = date(text, &words, i).or_else(|| amount(text, &words, i)) else {
            i += 1;
            continue;
        };
        if let Some(edit) = edit
            && text[edit.0..edit.1] != edit.2
        {
            edits.push(edit);
        }
        i = next;
    }
    splice(text, &edits)
}

/// The word after `i`, if only spaces separate them.
fn next_word<'a>(text: &str, words: &[Word<'a>], i: usize) -> Option<Word<'a>> {
    words
        .get(i + 1)
        .filter(|next| spaced(text, &words[i], next))
        .copied()
}

/// "March third twenty twenty six", "May 5th", "June twenty twenty".
fn date(text: &str, words: &[Word], i: usize) -> Option<(usize, Option<Edit>)> {
    let month = words[i];
    let lower = month.lower();
    if !MONTHS.contains(&lower.as_str()) {
        return None;
    }
    let day_word = next_word(text, words, i)?;
    // "may" and "march" are verbs too. Lowercase "march" only counts before a
    // spoken day like "third", lowercase "may" never.
    if matches!(lower.as_str(), "may" | "march") && !month.is_capitalized() {
        let ordinal_follows = ordinal(&day_word.lower()).is_some()
            || next_word(text, words, i + 1).is_some_and(|w| ordinal(&w.lower()).is_some());
        if lower == "may" || !ordinal_follows {
            return None;
        }
    }
    let name = capitalize(&lower);
    if let Some(run) = run_at(text, words, i + 1)
        && let Some(year) = year(&run.pieces)
    {
        let edit = (month.start, words[run.last].end, format!("{name} {year}"));
        return Some((run.last + 1, Some(edit)));
    }
    let (day, day_last) = day_at(text, words, i + 1)?;
    let year = words.get(day_last + 1).and_then(|next| {
        let between = gap(text, &words[day_last], next).trim();
        if between.is_empty() || between == "," {
            year_at(text, words, day_last + 1)
        } else {
            None
        }
    });
    let (out, last) = match year {
        Some((year, last)) => (format!("{name} {day}, {year}"), last),
        None => (format!("{name} {day}"), day_last),
    };
    Some((last + 1, Some((month.start, words[last].end, out))))
}

/// A day of the month and its last word: "third", "twenty first", "3rd",
/// or a plain number when a year or the end of the phrase follows.
fn day_at(text: &str, words: &[Word], k: usize) -> Option<(u64, usize)> {
    let word = words[k];
    let lower = word.lower();
    let in_month = |day: u64| (1..=31).contains(&day);
    if let Some(day) = ordinal(&lower).filter(|d| in_month(*d)) {
        return Some((day, k));
    }
    if let Some(Tens(t @ (20 | 30))) = piece(&lower)
        && let Some(next) = next_word(text, words, k)
        && let Some(o) = ordinal(&next.lower()).filter(|o| *o < 10)
        && in_month(t + o)
    {
        return Some((t + o, k + 1));
    }
    let digits = lower.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let suffix = &lower[digits.len()..];
    if matches!(suffix, "" | "st" | "nd" | "rd" | "th")
        && let Ok(day) = digits.parse::<u64>()
    {
        return in_month(day).then_some((day, k));
    }
    // "March three" is only a date when it ends there or a year follows.
    let next_piece = next_word(text, words, k).and_then(|next| piece(&next.lower()));
    let (day, last) = match (piece(&lower)?, next_piece) {
        (Tens(20), Some(Digit(d @ 1..=9))) => (20 + d, k + 1),
        (Digit(d @ 1..=9), _) => (d, k),
        (Teen(n) | Tens(n) | Compound(n), _) if in_month(n) => (n, k),
        _ => return None,
    };
    let then_year = words
        .get(last + 1)
        .is_some_and(|_| year_at(text, words, last + 1).is_some());
    (then_year || super::ends_phrase(text, &words[last])).then_some((day, last))
}

/// A year and its last word: "2026", "twenty twenty six", "two thousand ten".
fn year_at(text: &str, words: &[Word], k: usize) -> Option<(u64, usize)> {
    let word = words[k];
    if word.text.len() == 4
        && let Ok(year) = word.text.parse::<u64>()
    {
        return (1000..=2999).contains(&year).then_some((year, k));
    }
    let run = run_at(text, words, k)?;
    let year = year(&run.pieces).or_else(|| {
        value(&run.pieces).filter(|v| run.pieces.len() >= 2 && (1000..=2999).contains(v))
    })?;
    Some((year, run.last))
}

/// Times, money, percentages and plain numbers.
fn amount(text: &str, words: &[Word], i: usize) -> Option<(usize, Option<Edit>)> {
    let first = words[i];
    if written_number(&first) {
        return Some((i + 1, written_amount(text, words, i)));
    }
    let run = run_at(text, words, i)?;
    let next = run.last + 1;
    let edit = time(text, words, &run)
        .or_else(|| unit_amount(text, words, &run))
        .or_else(|| ordinal_compound(text, words, &run))
        .map(|(edit, after)| (after, Some(edit)));
    if let Some((after, edit)) = edit {
        return Some((after, edit));
    }
    let number = value(&run.pieces)
        .filter(|v| run.pieces.len() >= 2 || *v >= 10)
        .map(|v| render(&run.pieces, v))
        .or_else(|| year(&run.pieces).map(|y| y.to_string()));
    let edit = number.map(|n| (first.start, words[run.last].end, n));
    Some((next, edit))
}

/// "3 pm", "3:30 p.m.", "5 percent" and "10 dollars" as the transcriber wrote them.
fn written_amount(text: &str, words: &[Word], i: usize) -> Option<Edit> {
    let number = words[i];
    let unit = next_word(text, words, i)?;
    let lower = unit.lower();
    if AM_PM.contains(&lower.as_str()) {
        let hour = number.text.split(':').next()?.parse::<u64>().ok()?;
        if !(1..=12).contains(&hour) {
            return None;
        }
        let (end, marker) = am_pm(text, words, i + 1);
        return Some((number.start, end, format!("{} {marker}", number.text)));
    }
    if number.text.contains([':', ',']) {
        return None;
    }
    let out = match lower.as_str() {
        "percent" => format!("{}%", number.text),
        "dollars" | "dollar" | "bucks" => format!("${}", number.text),
        "euros" | "euro" => format!("€{}", number.text),
        _ => return None,
    };
    Some((number.start, unit.end, out))
}

/// Where an "am" or "pm" word ends, taking its full stop along mid-sentence,
/// and how it's written: "AM" or "PM".
fn am_pm(text: &str, words: &[Word], k: usize) -> (usize, &'static str) {
    let word = words[k];
    let marker = if word.lower().starts_with('a') {
        "AM"
    } else {
        "PM"
    };
    let dotted = word.text.contains('.') && text[word.end..].starts_with('.');
    let sentence_goes_on = words
        .get(k + 1)
        .is_some_and(|next| !next.is_capitalized() && !gap(text, &word, next).contains('\n'));
    let end = if dotted && sentence_goes_on {
        word.end + 1
    } else {
        word.end
    };
    (end, marker)
}

/// "three thirty pm", "twelve oh five a.m.", "four o'clock".
fn time(text: &str, words: &[Word], run: &Run) -> Option<(Edit, usize)> {
    let unit = next_word(text, words, run.last)?;
    let lower = unit.lower();
    let (hour, rest) = run.pieces.split_first()?;
    let hour = match *hour {
        Digit(h @ 1..=9) => h,
        Teen(h @ 10..=12) => h,
        _ => return None,
    };
    let minutes = match rest {
        [] => None,
        [Oh, Digit(d @ 1..=9)] => Some(*d),
        [Teen(m) | Compound(m)] if *m < 60 => Some(*m),
        [Tens(t @ 20..=50)] => Some(*t),
        [Tens(t @ 20..=50), Digit(d @ 1..=9)] => Some(t + d),
        _ => return None,
    };
    let first = first_word_start(words, run);
    let clock = match minutes {
        Some(m) => format!("{hour}:{m:02}"),
        None => hour.to_string(),
    };
    if lower == "o'clock" && minutes.is_none() {
        return Some(((first, unit.end, format!("{hour} o'clock")), run.last + 2));
    }
    if !AM_PM.contains(&lower.as_str()) {
        return None;
    }
    let (end, marker) = am_pm(text, words, run.last + 1);
    Some(((first, end, format!("{clock} {marker}")), run.last + 2))
}

/// Where a run starts. Runs skip "and", so this looks back from its last word.
fn first_word_start(words: &[Word], run: &Run) -> usize {
    let mut k = run.last;
    let mut pieces = run.pieces.len();
    while pieces > 1 {
        k -= 1;
        if !words[k].is("and") {
            pieces -= 1;
        }
    }
    words[k].start
}

/// "five percent", "two point five percent", "ten dollars and fifty cents",
/// "five million dollars".
fn unit_amount(text: &str, words: &[Word], run: &Run) -> Option<(Edit, usize)> {
    let whole = value(&run.pieces)?;
    let mut k = run.last;
    // Decimals only count before a unit: "at one point five people" isn't 1.5.
    let mut decimals = String::new();
    if let Some(point) = next_word(text, words, k)
        && point.is("point")
    {
        let mut j = k + 1;
        while let Some(next) = next_word(text, words, j)
            && let Some(Digit(d)) = piece(&next.lower())
        {
            decimals.push_str(&d.to_string());
            j += 1;
        }
        if decimals.is_empty() {
            return None;
        }
        k = j;
    }
    let number = if decimals.is_empty() {
        render(&run.pieces, whole)
    } else {
        format!("{}.{decimals}", digits(whole))
    };
    let unit = next_word(text, words, k)?;
    let first = first_word_start(words, run);
    let (out, last) = match unit.lower().as_str() {
        "percent" => (format!("{number}%"), k + 1),
        "per" if next_word(text, words, k + 1).is_some_and(|w| w.is("cent")) => {
            (format!("{number}%"), k + 2)
        }
        "dollars" | "dollar" | "bucks" => {
            let cents = if decimals.is_empty() {
                cents_after(text, words, k + 1)
            } else {
                None
            };
            match cents {
                Some((cents, last)) => (format!("${number}.{cents:02}"), last),
                None => (format!("${number}"), k + 1),
            }
        }
        "euros" | "euro" => (format!("€{number}"), k + 1),
        _ => return None,
    };
    Some(((first, words[last].end, out), last + 1))
}

/// "and fifty cents" after a dollar amount.
fn cents_after(text: &str, words: &[Word], k: usize) -> Option<(u64, usize)> {
    next_word(text, words, k).filter(|w| w.is("and"))?;
    let run = run_at(text, words, k + 2).filter(|_| spaced(text, &words[k + 1], &words[k + 2]))?;
    let cents = value(&run.pieces).filter(|c| *c < 100)?;
    next_word(text, words, run.last)
        .filter(|w| w.is_any(&["cents", "cent"]))
        .map(|_| (cents, run.last + 1))
}

/// "twenty first" as in "the twenty first century".
fn ordinal_compound(text: &str, words: &[Word], run: &Run) -> Option<(Edit, usize)> {
    let [Tens(t)] = run.pieces[..] else {
        return None;
    };
    let next = next_word(text, words, run.last)?;
    let o = ordinal(&next.lower()).filter(|o| *o < 10)?;
    let n = t + o;
    let edit = (
        words[run.last].start,
        next.end,
        format!("{n}{}", ordinal_suffix(n)),
    );
    Some((edit, run.last + 2))
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
    fn numbers_become_digits() {
        check(&[
            ("twenty five", "25"),
            ("I counted twenty-five birds.", "I counted 25 birds."),
            ("Twenty five people came.", "25 people came."),
            ("about fifteen minutes", "about 15 minutes"),
            ("one hundred and five", "105"),
            ("three hundred twenty", "320"),
            ("two thousand three hundred", "2,300"),
            ("forty thousand", "40,000"),
            ("five million people", "5 million people"),
            ("one million two hundred thousand", "1,200,000"),
            ("in nineteen eighty four", "in 1984"),
            ("back in twenty twenty", "back in 2020"),
            ("by twenty oh five", "by 2005"),
            ("the twenty first century", "the 21st century"),
            ("top ten", "top 10"),
        ]);
    }

    #[test]
    fn small_numbers_and_digit_runs_stay_words() {
        for input in [
            "one of them",
            "I have two kids and three cats.",
            "one two three",
            "five five five one two three four",
            "nine eleven",
            "Oh, that's fine.",
            "a hundred times",
            "hundreds of people",
            "at one point five of us left",
            "I'll be there at three thirty.",
            "someone and everyone",
            "It's a one-off.",
            "We march forward.",
            "You may first ask.",
            "Wait a second.",
            "zero",
        ] {
            assert_eq!(apply(input), input, "input: {input:?}");
        }
    }

    #[test]
    fn dates_are_written_out() {
        check(&[
            ("march third twenty twenty six", "March 3, 2026"),
            ("we march twenty miles", "we march 20 miles"),
            ("March third twenty twenty six", "March 3, 2026"),
            ("on March third, twenty twenty six.", "on March 3, 2026."),
            ("due june twenty first", "due June 21"),
            ("July 4th", "July 4"),
            ("August twenty-fifth 2025", "August 25, 2025"),
            ("December thirty first", "December 31"),
            ("May fifth", "May 5"),
            ("since June twenty twenty", "since June 2020"),
            ("on April three.", "on April 3."),
            ("January two thousand ten", "January 2010"),
            ("in April three people left", "in April three people left"),
            ("October 2026", "October 2026"),
        ]);
    }

    #[test]
    fn times_money_and_percentages() {
        check(&[
            ("three thirty pm", "3:30 PM"),
            (
                "Meet at three thirty p.m. tomorrow.",
                "Meet at 3:30 PM tomorrow.",
            ),
            ("Meet at three p.m.", "Meet at 3 PM."),
            (
                "Meet at three p.m. Bring snacks.",
                "Meet at 3 PM. Bring snacks.",
            ),
            ("twelve oh five am", "12:05 AM"),
            ("ten fifteen a.m.", "10:15 AM."),
            ("at four o'clock", "at 4 o'clock"),
            ("at 3 pm", "at 3 PM"),
            ("at 3:30 p.m. today", "at 3:30 PM today"),
            ("five percent", "5%"),
            ("two point five percent", "2.5%"),
            ("fifty per cent", "50%"),
            ("ten dollars", "$10"),
            ("one dollar", "$1"),
            ("ten dollars and fifty cents", "$10.50"),
            ("five million dollars", "$5 million"),
            ("twenty euros", "€20"),
            ("costs 10 dollars", "costs $10"),
            ("a 5 percent raise", "a 5% raise"),
            ("fifty cents", "50 cents"),
        ]);
    }
}
