//! Command mode: select text in any app, hold the command shortcut and say
//! what to do with it, like "make this friendlier" or "translate to French".
//! The selection is copied, the instruction transcribed, and the AI cleanup
//! model rewrites the selection, which is pasted over it.
//!
//! It has its own shortcut rather than guessing from what was said: a
//! dictation that happens to start with "make" or "fix" should never replace
//! the text around it.

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::{
    AppState,
    cleanup::{Cleaner, Message, Prompt},
    dictation::{Controller, Event},
    paste::Paster,
    pipeline::{self, Error, Stage},
    platform,
    tone::AppCategory,
};

/// Longer selections are refused: they'd outgrow the model's context, and
/// take too long to rewrite.
pub const MAX_SELECTION_WORDS: usize = 1200;

const NOT_SET_UP: &str = "Set up AI cleanup to use commands";
const NO_SAFE_COPY: &str = "Commands can't run on Wayland yet";
const NO_SELECTION: &str = "Select some text first";
const TOO_LONG: &str = "Selection is too long";
const FAILED: &str = "Couldn't edit the text";
const UNCHANGED: &str = "The model left the text as it was. Try rewording.";

const SYSTEM_PROMPT: &str = "\
You are a text editor. You receive some text between <selection> tags and an instruction \
someone dictated between <instruction> tags. Rewrite the text following the instruction.
- Change only what the instruction asks for. Keep everything else, including line breaks, \
formatting and language, unless the instruction says otherwise.
- If the instruction asks to write something new from the text, like a reply or a summary, \
write that instead.
- If the instruction is vague or unusual, like \"make it sound like a pirate\" or \"more \
punchy\", go with its most likely meaning. Don't return the text as it was because you're \
unsure what was meant.
Reply with the resulting text only: no explanations, quotes, tags or comments around it.";

/// (selection, instruction, result)
const EXAMPLES: &[(&str, &str, &str)] = &[
    (
        "hey can u send me the report by tmrw",
        "make this more formal",
        "Could you please send me the report by tomorrow?",
    ),
    (
        "I am writing to inform you that I will be unable to attend the meeting.",
        "make it casual",
        "Hey, just a heads up, I can't make the meeting.",
    ),
    (
        "The meeting is on Monday.\nBring your laptop.",
        "translate to Spanish",
        "La reunión es el lunes.\nTrae tu portátil.",
    ),
];

fn wrap(selection: &str, instruction: &str) -> String {
    format!("<selection>\n{selection}\n</selection>\n<instruction>\n{instruction}\n</instruction>")
}

/// The conversation that asks for `selection` to be edited by `instruction`.
pub fn messages(selection: &str, instruction: &str) -> Vec<Message> {
    let mut messages = vec![Message::new("system", SYSTEM_PROMPT)];
    for (text, said, edited) in EXAMPLES {
        messages.push(Message::new("user", wrap(text, said)));
        messages.push(Message::new("assistant", *edited));
    }
    messages.push(Message::new("user", wrap(selection, instruction)));
    messages
}

pub fn prompt(selection: &str, instruction: &str) -> Prompt {
    let words = selection.split_whitespace().count() as u64;
    Prompt {
        messages: messages(selection, instruction),
        // Room for a rewrite a good deal longer than the selection.
        max_tokens: (words * 4 + 128).min(4096) as u32,
        timeout: std::time::Duration::from_millis(30_000 + words * 150)
            .min(std::time::Duration::from_secs(120)),
    }
}

/// The model's reply as the text to paste, or `None` if there's nothing
/// usable in it. Drops thinking, tags, a code fence or quotes the selection
/// didn't have, and a chatty first line like "Here's the rewritten text:".
pub fn sanitize(selection: &str, reply: &str) -> Option<String> {
    let mut text = reply.trim();
    if let Some(start) = text.find("<think>") {
        let end = text[start..].find("</think>")? + start + "</think>".len();
        let rest = format!("{}{}", &text[..start], &text[end..]);
        return sanitize(selection, &rest);
    }
    for tag in ["selection", "result", "text"] {
        let (open, close) = (format!("<{tag}>"), format!("</{tag}>"));
        if let Some(inner) = text.strip_prefix(open.as_str()) {
            text = inner.strip_suffix(close.as_str()).unwrap_or(inner).trim();
        }
    }
    if text.starts_with("```") && !selection.trim_start().starts_with("```") {
        let body = text.trim_start_matches('`');
        // The fence's first line may name a language.
        let body = body.split_once('\n').map_or("", |(_, rest)| rest);
        text = body.trim_end().trim_end_matches('`').trim();
    }
    if let Some((first, rest)) = text.split_once('\n') {
        let lower = first.trim().to_lowercase();
        let chatty = ["here is", "here's", "sure", "certainly", "okay", "ok,"]
            .iter()
            .any(|p| lower.starts_with(p));
        if chatty && lower.ends_with(':') {
            text = rest.trim();
        }
    }
    let quoted = |s: &str| {
        s.chars().count() >= 2
            && ((s.starts_with('"') && s.ends_with('"'))
                || (s.starts_with('“') && s.ends_with('”')))
    };
    if quoted(text) && !quoted(selection.trim()) {
        let mut chars = text.chars();
        chars.next();
        chars.next_back();
        text = chars.as_str().trim();
    }
    (!text.is_empty()).then(|| text.to_string())
}

/// Registers the command shortcut, replacing `previous`. An empty shortcut
/// turns command mode off. Single modifier keys aren't offered: the listener
/// for those holds one key, the dictation hotkey.
pub fn register_hotkey(
    app: &AppHandle,
    controller: &Controller,
    hotkey: &str,
    previous: Option<&str>,
) -> Result<(), String> {
    let shortcuts = app.global_shortcut();
    if let Some(previous) = previous.map(str::trim).filter(|p| !p.is_empty()) {
        let _ = shortcuts.unregister(previous);
    }
    let hotkey = hotkey.trim();
    if hotkey.is_empty() {
        return Ok(());
    }
    if platform::MODIFIER_HOTKEYS.contains(&hotkey) {
        return Err("The command shortcut needs a regular key, like Ctrl+Shift+E".into());
    }
    let controller = controller.clone();
    shortcuts
        .on_shortcut(hotkey, move |_, _, event| {
            controller.send(match event.state {
                ShortcutState::Pressed => Event::CommandHotkeyDown,
                ShortcutState::Released => Event::CommandHotkeyUp,
            });
        })
        .map_err(|e| format!("Couldn't register {hotkey}: {e}"))
}

/// Why command mode can't run, if it can't: it needs AI cleanup on with a
/// model ready, and a copy shortcut that's safe to press in the focused app.
pub fn unavailable(app: &AppHandle) -> Option<&'static str> {
    if !platform::can_copy_selection() {
        return Some(NO_SAFE_COPY);
    }
    let state = app.state::<AppState>();
    (!Cleaner::is_set_up(&state.settings().cleanup, &state.models)).then_some(NOT_SET_UP)
}

/// With nothing selected, VS Code and JetBrains IDEs copy the whole current
/// line, line break included. Taken as the selection, the edit would be
/// pasted next to the line rather than over it, so in code editors a copy
/// ending in a line break counts as no selection. Selecting whole lines down
/// to the next one is refused too; selecting up to the end of the last line
/// isn't.
fn is_whole_line_copy(category: AppCategory, copied: &str) -> bool {
    category == AppCategory::Code && copied.ends_with('\n')
}

/// Runs on a worker thread once the instruction is recorded. Returns the
/// edited text to paste over the selection, which is still selected.
pub fn run(
    app: &AppHandle,
    paster: &Paster,
    samples: &[f32],
    duration_secs: f64,
    category: AppCategory,
    on_stage: impl Fn(Stage),
) -> Result<String, Error> {
    let selection = paster
        .copy_selection(category == AppCategory::Terminal)
        .map_err(|e| Error::Command(FAILED, Some(e)))?
        .filter(|copied| !is_whole_line_copy(category, copied))
        .ok_or(Error::Command(NO_SELECTION, None))?;
    if selection.split_whitespace().count() > MAX_SELECTION_WORDS {
        return Err(Error::Command(TOO_LONG, None));
    }
    let (instruction, model_name) = pipeline::transcribe_only(app, samples, &on_stage)?;

    let state = app.state::<AppState>();
    let settings = state.settings();
    if !Cleaner::is_set_up(&settings.cleanup, &state.models) {
        return Err(Error::Command(NOT_SET_UP, None));
    }
    on_stage(Stage::Editing);
    let complete = |prompt: &Prompt| {
        let reply = state
            .cleaner
            .complete(&settings.cleanup, &state.models, prompt)
            .map_err(|e| Error::Command(FAILED, Some(e)))?;
        Ok::<_, Error>((
            reply.clone(),
            sanitize(&selection, &reply).ok_or_else(|| {
                Error::Command(FAILED, Some("The model's reply was empty".into()))
            })?,
        ))
    };
    let mut prompt = prompt(&selection, &instruction);
    let (reply, mut edited) = complete(&prompt)?;
    // Small models sometimes hand back the selection when they don't know
    // what to do. Ask once more, then say so rather than pasting it unchanged.
    if is_unchanged(&selection, &edited) {
        prompt.messages.push(Message::new("assistant", reply));
        prompt
            .messages
            .push(Message::new("user", retry_message(&instruction)));
        edited = complete(&prompt)?.1;
        if is_unchanged(&selection, &edited) {
            return Err(Error::Command(UNCHANGED, None));
        }
    }
    pipeline::save(
        app,
        samples,
        duration_secs,
        &history_text(&instruction, &edited),
        model_name,
        None,
    );
    Ok(edited)
}

/// Whether `edited` is the selection again, give or take spacing.
fn is_unchanged(selection: &str, edited: &str) -> bool {
    selection.split_whitespace().eq(edited.split_whitespace())
}

fn retry_message(instruction: &str) -> String {
    format!(
        "That's the same text as before. Rewrite it so it follows the instruction \
         \"{instruction}\", taking the most likely meaning. Reply with the resulting text only."
    )
}

/// What History shows for a command: what was said, then the result.
fn history_text(instruction: &str, edited: &str) -> String {
    format!("Command: {instruction}\n\n{edited}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_selection_and_instruction_go_last_and_tagged() {
        let messages = messages("hi there", "make it louder");
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages.len(), 2 + EXAMPLES.len() * 2);
        let last = messages.last().unwrap();
        assert_eq!(last.role, "user");
        assert_eq!(
            last.content,
            "<selection>\nhi there\n</selection>\n<instruction>\nmake it louder\n</instruction>"
        );
    }

    #[test]
    fn budgets_grow_with_the_selection() {
        let short = prompt("one two", "fix it");
        assert_eq!(short.max_tokens, 136);
        assert_eq!(short.timeout, std::time::Duration::from_millis(30_300));
        let long = prompt(&"word ".repeat(5000), "fix it");
        assert_eq!(long.max_tokens, 4096);
        assert_eq!(long.timeout, std::time::Duration::from_secs(120));
    }

    #[test]
    fn plain_replies_pass_through() {
        assert_eq!(
            sanitize("hey", "Hello there.\nHow are you?").as_deref(),
            Some("Hello there.\nHow are you?")
        );
        assert_eq!(sanitize("x", "  Done.  ").as_deref(), Some("Done."));
    }

    #[test]
    fn wrapping_and_chatter_are_removed() {
        assert_eq!(
            sanitize("hey", "<think>\nThey want it formal.\n</think>\nHello.").as_deref(),
            Some("Hello.")
        );
        assert_eq!(sanitize("hey", "<think>\nStill going"), None);
        assert_eq!(
            sanitize("hey", "<selection>\nHello.\n</selection>").as_deref(),
            Some("Hello.")
        );
        assert_eq!(
            sanitize("hey", "<result>Hello.</result>").as_deref(),
            Some("Hello.")
        );
        assert_eq!(sanitize("hey", "\"Hello.\"").as_deref(), Some("Hello."));
        assert_eq!(sanitize("hey", "“Hello.”").as_deref(), Some("Hello."));
        assert_eq!(
            sanitize("hey", "Here's the rewritten text:\n\nHello there.").as_deref(),
            Some("Hello there.")
        );
        assert_eq!(
            sanitize("fn a(){}", "```rust\nfn a() {}\n```").as_deref(),
            Some("fn a() {}")
        );
    }

    #[test]
    fn what_the_selection_had_stays() {
        assert_eq!(
            sanitize("\"quoted\"", "\"Quoted.\"").as_deref(),
            Some("\"Quoted.\"")
        );
        assert_eq!(
            sanitize("```\ncode\n```", "```\nbetter code\n```").as_deref(),
            Some("```\nbetter code\n```")
        );
        // A first line that's part of the text, not chatter.
        assert_eq!(
            sanitize("x", "Sure thing, see you then.\nBest,\nSam").as_deref(),
            Some("Sure thing, see you then.\nBest,\nSam")
        );
    }

    #[test]
    fn empty_replies_are_rejected() {
        assert_eq!(sanitize("hey", ""), None);
        assert_eq!(sanitize("hey", "   \n "), None);
        assert_eq!(sanitize("hey", "\"\""), None);
        assert_eq!(sanitize("hey", "<result></result>"), None);
    }

    #[test]
    fn a_whole_line_copied_in_a_code_editor_is_no_selection() {
        assert!(is_whole_line_copy(AppCategory::Code, "let x = 1;\n"));
        assert!(is_whole_line_copy(AppCategory::Code, "let x = 1;\r\n"));
        assert!(!is_whole_line_copy(AppCategory::Code, "let x = 1;"));
        assert!(!is_whole_line_copy(AppCategory::Code, "a\nb"));
        assert!(!is_whole_line_copy(AppCategory::Other, "Dear Sam,\n"));
        assert!(!is_whole_line_copy(AppCategory::Terminal, "ls -la\n"));
    }

    #[test]
    fn unchanged_text_is_noticed_through_spacing() {
        assert!(is_unchanged("hey there", "  hey\nthere "));
        assert!(!is_unchanged("hey there", "Hello there."));
    }

    #[test]
    fn history_shows_the_command_and_result() {
        assert_eq!(
            history_text("make it formal", "Good morning."),
            "Command: make it formal\n\nGood morning."
        );
    }
}
