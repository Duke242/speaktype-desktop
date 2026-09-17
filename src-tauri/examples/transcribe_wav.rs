//! Benchmarks a downloaded model on an audio file, without the UI.
//!
//!     cargo run --release --example transcribe_wav -- <model-id> <models-dir> <audio> [runs]
//!     cargo run --release --example transcribe_wav -- download <model-id> <models-dir>

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if let [_, command, model, dir] = args.as_slice()
        && command == "download"
    {
        return speaktype_lib::devtools::download(model, dir.as_ref(), None);
    }
    let [_, model, dir, audio, rest @ ..] = args.as_slice() else {
        return Err("usage: transcribe_wav <model-id> <models-dir> <audio> [runs]".into());
    };
    let runs = rest.first().and_then(|r| r.parse().ok()).unwrap_or(3);
    speaktype_lib::devtools::benchmark(model, dir.as_ref(), audio.as_ref(), runs)
}
