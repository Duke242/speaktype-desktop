//! Benchmarks a downloaded model on audio files, without the UI.
//!
//!     cargo run --release --example transcribe_wav -- <model-id> <models-dir> <audio>... [--language en] [--runs 5]
//!     cargo run --release --example transcribe_wav -- download <model-id> <models-dir>

use std::path::Path;

const USAGE: &str =
    "usage: transcribe_wav <model-id> <models-dir> <audio>... [--language auto] [--runs 3]";

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [command, model, dir] = args.as_slice()
        && command == "download"
    {
        return wordflow_lib::devtools::download(model, dir.as_ref(), None);
    }
    let mut language = "auto".to_string();
    let mut runs = 3;
    let mut positional = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--language" => language = args.next().ok_or(USAGE)?.clone(),
            "--runs" => runs = args.next().and_then(|r| r.parse().ok()).ok_or(USAGE)?,
            _ => positional.push(arg),
        }
    }
    let [model, dir, files @ ..] = positional.as_slice() else {
        return Err(USAGE.into());
    };
    if files.is_empty() {
        return Err(USAGE.into());
    }
    let files: Vec<&Path> = files.iter().map(Path::new).collect();
    wordflow_lib::devtools::benchmark(model, dir.as_ref(), &files, &language, runs)
}
