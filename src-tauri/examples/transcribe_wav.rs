//! Transcribes a WAV file without the UI, to check the engine and text pipeline.
//!
//!     cargo run --example transcribe_wav -- <model.bin> <audio.wav> [language]

#[path = "../src/audio.rs"]
#[allow(dead_code)]
mod audio;
#[path = "../src/text.rs"]
mod text;
#[path = "../src/transcribe.rs"]
#[allow(dead_code)]
mod transcribe;

use std::time::Instant;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let [_, model, wav, rest @ ..] = args.as_slice() else {
        return Err("usage: transcribe_wav <model.bin> <audio.wav> [language]".into());
    };
    let language = rest.first().map(String::as_str).unwrap_or("auto");

    let mut reader = hound::WavReader::open(wav).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(Result::unwrap).collect(),
        hound::SampleFormat::Int => {
            let scale = (1u64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap() as f32 / scale)
                .collect()
        }
    };
    let channels = spec.channels as usize;
    let mono: Vec<f32> = raw
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect();
    let samples = audio::resample(&mono, spec.sample_rate, audio::WHISPER_SAMPLE_RATE);

    transcribe::silence_logs();
    let mut engine = transcribe::Engine::default();
    let started = Instant::now();
    engine.load("cli", std::path::Path::new(model))?;
    println!("loaded in {:.2?}", started.elapsed());

    let started = Instant::now();
    let raw_text = engine.transcribe(&samples, language)?;
    println!(
        "transcribed {:.1}s of audio in {:.2?}",
        samples.len() as f32 / 16_000.0,
        started.elapsed()
    );
    println!("raw:   {raw_text:?}");
    let options = text::Options {
        auto_edit: false,
        smart_trailing_punctuation: true,
        dictionary: &[],
    };
    println!("final: {:?}", text::process(&raw_text, &options));
    Ok(())
}
