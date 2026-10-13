//! Spoken code, written as code: in code editors and terminals, "main dot rs"
//! becomes "main.rs", "user underscore id" becomes "user_id", "camel case get
//! user name" becomes "getUserName", and in terminals "dash dash verbose"
//! becomes "--verbose".
//!
//! Only runs for apps `tone` knows are code editors or terminals, where these
//! words are far more likely to be code than prose.

/// Words that name a case, said before "case".
const CASES: &[(&str, Case)] = &[
    ("camel", Case::Camel),
    ("pascal", Case::Pascal),
    ("snake", Case::Snake),
    ("kebab", Case::Kebab),
    ("constant", Case::Constant),
];

/// The most words one "camel case …" takes.
const MAX_CASE_WORDS: usize = 6;

/// Words that end a "camel case …" name, as they're rarely part of one.
const CASE_STOPS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "but", "for", "from", "in", "into", "is", "of", "on",
    "or", "so", "that", "the", "then", "to", "was", "which", "with",
];

/// Words after which "dot" or "slash" starts a new name rather than joining
/// the one before, as in "rename it to dot env".
const LEADING_WORDS: &[&str] = &[
    "a", "an", "and", "at", "called", "create", "delete", "edit", "for", "from", "in", "into",
    "is", "it", "its", "my", "named", "of", "on", "open", "or", "our", "rename", "the", "this",
    "to", "with", "your",
];

/// File extensions as they're said or misheard, and how they're written.
const EXTENSIONS: &[(&str, &str)] = &[
    ("jason", "json"),
    ("pie", "py"),
    ("text", "txt"),
    ("markdown", "md"),
    ("yammel", "yaml"),
    ("yamel", "yaml"),
    ("tommel", "toml"),
    ("tomel", "toml"),
];

/// Extensions written in lowercase even when the transcript capitalized them.
const KNOWN_EXTENSIONS: &[&str] = &[
    "c", "cc", "cpp", "cs", "css", "csv", "dart", "env", "go", "h", "hpp", "html", "java", "js",
    "json", "jsx", "kt", "lock", "log", "lua", "md", "mjs", "php", "py", "rb", "rs", "scss", "sh",
    "sql", "svelte", "swift", "toml", "ts", "tsx", "txt", "vue", "xml", "yaml", "yml", "zig",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Case {
    Camel,
    Pascal,
    Snake,
    Kebab,
    Constant,
}

impl Case {
    fn join(self, words: &[String]) -> String {
        let capitalized = |word: &String| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                .unwrap_or_default()
        };
        match self {
            Case::Camel => words
                .iter()
                .enumerate()
                .map(|(i, word)| {
                    if i == 0 {
                        word.clone()
                    } else {
                        capitalized(word)
                    }
                })
                .collect(),
            Case::Pascal => words.iter().map(capitalized).collect(),
            Case::Snake => words.join("_"),
            Case::Kebab => words.join("-"),
            Case::Constant => words.join("_").to_uppercase(),
        }
    }
}

/// One word of the transcript, split from the punctuation around it.
#[derive(Debug, Clone, Copy)]
struct Token<'a> {
    lead: &'a str,
    core: &'a str,
    trail: &'a str,
}

impl<'a> Token<'a> {
    fn new(word: &'a str) -> Self {
        let start = word
            .find(|c: char| c.is_alphanumeric())
            .unwrap_or(word.len());
        let end = word[start..]
            .rfind(|c: char| c.is_alphanumeric())
            .map_or(start, |i| {
                start + i + word[start + i..].chars().next().map_or(0, char::len_utf8)
            });
        Self {
            lead: &word[..start],
            core: &word[start..end],
            trail: &word[end..],
        }
    }

    fn lower(&self) -> String {
        self.core.to_lowercase()
    }

    /// A plain word, with no punctuation on either side.
    fn is_bare(&self) -> bool {
        self.lead.is_empty() && self.trail.is_empty() && !self.core.is_empty()
    }
}

/// The text being written, with a note of whether the next piece joins the
/// last one without a space.
#[derive(Default)]
struct Output {
    text: String,
    glue: bool,
    /// The last word written, lowercase, and whether punctuation followed it.
    last: Option<(String, bool)>,
}

impl Output {
    fn push(&mut self, piece: &str) {
        if !self.text.is_empty() && !self.glue {
            self.text.push(' ');
        }
        self.text.push_str(piece);
        self.glue = false;
    }

    fn word(&mut self, token: &Token, core: &str) {
        self.push(&format!("{}{core}{}", token.lead, token.trail));
        self.last = Some((token.lower(), !token.trail.is_empty()));
    }

    /// Writes `symbol` joined to the next word, and to the last one unless
    /// that ended in punctuation or reads as English leading into a name.
    fn joiner(&mut self, symbol: &str, join_last: bool) {
        let joins = self.glue
            || join_last
                && self.last.as_ref().is_some_and(|(word, punctuated)| {
                    !punctuated && !LEADING_WORDS.contains(&word.as_str())
                });
        if joins {
            self.text.push_str(symbol);
        } else {
            self.push(symbol);
        }
        self.glue = true;
        self.last = None;
    }
}

/// The case named by the words at `i` ("camel case" or "camelcase"), and how
/// many words that took.
fn case_at(tokens: &[Token], i: usize) -> Option<(Case, usize)> {
    let word = tokens[i].lower();
    let named = |name: &str| CASES.iter().find(|(n, _)| *n == name).map(|(_, c)| *c);
    if let Some(case) = word.strip_suffix("case").and_then(named) {
        return tokens[i].is_bare().then_some((case, 1));
    }
    let case = named(&word)?;
    let next = tokens.get(i + 1)?;
    (tokens[i].is_bare() && next.is_bare() && next.lower() == "case").then_some((case, 2))
}

fn is_operator(word: &str) -> bool {
    matches!(word, "dot" | "underscore" | "slash" | "dash" | "double")
}

/// Formats spoken file names, identifiers and, in a `terminal`, flags.
pub fn format(text: &str, terminal: bool) -> String {
    let tokens: Vec<Token> = text.split_whitespace().map(Token::new).collect();
    let mut out = Output::default();
    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i];
        let word = token.lower();
        let next = tokens
            .get(i + 1)
            .filter(|next| next.lead.is_empty() && !next.core.is_empty());

        if let Some((case, used)) = case_at(&tokens, i) {
            let mut words = Vec::new();
            let mut trail = "";
            let mut j = i + used;
            while let Some(t) = tokens.get(j) {
                let w = t.lower();
                if words.len() == MAX_CASE_WORDS
                    || !t.lead.is_empty()
                    || t.core.is_empty()
                    || CASE_STOPS.contains(&w.as_str())
                    || is_operator(&w)
                    || case_at(&tokens, j).is_some()
                {
                    break;
                }
                words.push(w.chars().filter(|c| c.is_alphanumeric()).collect());
                j += 1;
                if !t.trail.is_empty() {
                    trail = t.trail;
                    break;
                }
            }
            if !words.is_empty() {
                let name = case.join(&words);
                let joined = Token {
                    lead: token.lead,
                    core: &name,
                    trail,
                };
                out.word(&joined, &name);
                i = j;
                continue;
            }
        }

        // Operators only count between words: "a polka dot." stays prose.
        if token.is_bare()
            && let Some(next) = next
        {
            let next_word = next.lower();
            match word.as_str() {
                "dot" => {
                    out.joiner(".", true);
                    i += 1;
                    // "dot t s x" is spelled out: join the letters.
                    let mut letters = String::new();
                    let mut j = i;
                    while let Some(t) = tokens.get(j)
                        && t.core.chars().count() == 1
                        && t.lead.is_empty()
                        && letters.len() < 4
                    {
                        letters.push_str(&t.lower());
                        j += 1;
                        if !t.trail.is_empty() {
                            break;
                        }
                    }
                    if letters.len() > 1 {
                        let trail = tokens[j - 1].trail;
                        out.push(&format!("{letters}{trail}"));
                        out.last = Some((letters, !trail.is_empty()));
                        i = j;
                    } else {
                        let ext = EXTENSIONS
                            .iter()
                            .find(|(said, _)| *said == next_word)
                            .map(|(_, ext)| *ext)
                            .or_else(|| KNOWN_EXTENSIONS.iter().copied().find(|e| *e == next_word));
                        out.word(next, ext.unwrap_or(next.core));
                        i += 1;
                    }
                    continue;
                }
                "underscore" => {
                    out.joiner("_", true);
                    i += 1;
                    continue;
                }
                "double" if next_word == "underscore" && next.is_bare() => {
                    out.joiner("__", true);
                    i += 2;
                    continue;
                }
                "slash" => {
                    out.joiner("/", true);
                    i += 1;
                    continue;
                }
                // Flags stand apart from the word before: "ls dash la" is "ls -la".
                "dash" if terminal => {
                    out.joiner("-", false);
                    i += 1;
                    continue;
                }
                "double" if terminal && next_word == "dash" && next.is_bare() => {
                    out.joiner("--", false);
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }

        out.word(&token, token.core);
        i += 1;
    }
    out.text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(text: &str) -> String {
        format(text, false)
    }

    fn shell(text: &str) -> String {
        format(text, true)
    }

    #[test]
    fn file_names() {
        assert_eq!(code("open main dot rs"), "open main.rs");
        assert_eq!(code("Main dot RS."), "Main.rs.");
        assert_eq!(code("config dot jason"), "config.json");
        assert_eq!(code("setup dot pie"), "setup.py");
        assert_eq!(code("rename it to dot env"), "rename it to .env");
        assert_eq!(code("all the dot ts files"), "all the .ts files");
        assert_eq!(code("index dot t s x"), "index.tsx");
        assert_eq!(code("os dot path dot join"), "os.path.join");
        assert_eq!(code("src slash lib slash api dot ts"), "src/lib/api.ts");
        assert_eq!(code("main.rs is fine"), "main.rs is fine");
    }

    #[test]
    fn identifiers() {
        assert_eq!(code("user underscore id"), "user_id");
        assert_eq!(
            code("set max underscore retries to 3"),
            "set max_retries to 3"
        );
        assert_eq!(code("double underscore init double underscore"), "__init__");
        assert_eq!(code("camel case get user name"), "getUserName");
        assert_eq!(code("Call camel case get user name."), "Call getUserName.");
        assert_eq!(code("pascal case user profile card"), "UserProfileCard");
        assert_eq!(code("snake case total item count"), "total_item_count");
        assert_eq!(code("kebab case main nav bar"), "main-nav-bar");
        assert_eq!(code("constant case max retries"), "MAX_RETRIES");
        assert_eq!(code("camelcase fetch data"), "fetchData");
    }

    #[test]
    fn a_name_ends_at_a_small_word_or_punctuation() {
        assert_eq!(
            code("rename camel case get user to camel case fetch user"),
            "rename getUser to fetchUser"
        );
        assert_eq!(code("snake case user id, then save"), "user_id, then save");
        assert_eq!(code("camel case is"), "camel case is");
        assert_eq!(code("in this case"), "in this case");
    }

    #[test]
    fn flags_only_in_terminals() {
        assert_eq!(shell("ls dash la"), "ls -la");
        assert_eq!(
            shell("cargo test dash dash release"),
            "cargo test --release"
        );
        assert_eq!(
            shell("run it with double dash verbose"),
            "run it with --verbose"
        );
        assert_eq!(shell("git commit dash m"), "git commit -m");
        assert_eq!(code("a dash of salt"), "a dash of salt");
    }

    #[test]
    fn prose_is_left_alone() {
        assert_eq!(code("a polka dot."), "a polka dot.");
        assert_eq!(code("Fix the login bug."), "Fix the login bug.");
        assert_eq!(code(""), "");
        assert_eq!(code("  spaced   out  "), "spaced out");
        assert_eq!(
            code("“quoted” text, (and) more"),
            "“quoted” text, (and) more"
        );
    }
}
