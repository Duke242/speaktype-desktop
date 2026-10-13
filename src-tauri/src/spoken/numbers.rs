//! Spoken number words.

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

/// Whether `word`, in lowercase, is part of a spoken number.
pub(super) fn is_number_word(word: &str) -> bool {
    if let Some((tens, ones)) = word.split_once('-') {
        return TENS.contains(&tens) && ONES[1..].contains(&ones);
    }
    ONES.contains(&word)
        || TEENS.contains(&word)
        || TENS.contains(&word)
        || matches!(word, "hundred" | "thousand" | "million" | "billion")
}

#[cfg(test)]
mod tests {
    use super::is_number_word;

    #[test]
    fn number_words() {
        for word in [
            "zero",
            "twelve",
            "forty",
            "twenty-five",
            "hundred",
            "million",
        ] {
            assert!(is_number_word(word), "{word}");
        }
        for word in ["oh", "twenty-zero", "five-twenty", "first", "none"] {
            assert!(!is_number_word(word), "{word}");
        }
    }
}
