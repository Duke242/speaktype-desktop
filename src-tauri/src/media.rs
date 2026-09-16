//! Reading audio and video files, and saving recordings as WAV.

use std::{fs::File, path::Path};

use symphonia::core::{
    codecs::audio::AudioDecoderOptions,
    errors::Error,
    formats::{FormatOptions, TrackType, probe::Hint},
    io::MediaSourceStream,
    meta::MetadataOptions,
};

use crate::audio::{self, WHISPER_SAMPLE_RATE};

/// Decodes the first audio track of a file to 16 kHz mono.
/// Returns the samples and the audio's length in seconds.
pub fn decode_file(path: &Path) -> Result<(Vec<f32>, f64), String> {
    let file = File::open(path).map_err(|e| format!("Couldn't open file: {e}"))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let mut format = symphonia::default::get_probe()
        .probe(&hint, stream, FormatOptions::default(), MetadataOptions::default())
        .map_err(|_| "This file type isn't supported".to_string())?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or("This file has no audio track")?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or("This file's audio format isn't supported")?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(|_| "This file's audio codec isn't supported".to_string())?;

    let mut mono = Vec::new();
    let mut sample_rate = 0;
    let mut frame = Vec::<f32>::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(Error::IoError(_)) => break,
            Err(e) => return Err(format!("Couldn't read file: {e}")),
        };
        if packet.track_id != track_id {
            continue;
        }
        let buffer = match decoder.decode(&packet) {
            Ok(buffer) => buffer,
            // A corrupt packet shouldn't fail the whole file.
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("Couldn't decode audio: {e}")),
        };
        sample_rate = buffer.spec().rate();
        let channels = buffer.spec().channels().count().max(1);
        frame.resize(buffer.samples_interleaved(), 0.0);
        buffer.copy_to_slice_interleaved(&mut frame);
        mono.extend(
            frame
                .chunks(channels)
                .map(|f| f.iter().sum::<f32>() / channels as f32),
        );
    }

    if mono.is_empty() || sample_rate == 0 {
        return Err("This file has no audio".into());
    }
    let duration = mono.len() as f64 / sample_rate as f64;
    Ok((audio::resample(&mono, sample_rate, WHISPER_SAMPLE_RATE), duration))
}

/// Saves 16 kHz mono audio as a 16-bit WAV file.
pub fn write_wav(path: &Path, samples: &[f32]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: WHISPER_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer.write_sample(value).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_round_trips_through_the_decoder() {
        let dir = std::env::temp_dir().join(format!("speaktype-test-{}", uuid::Uuid::new_v4()));
        let path = dir.join("tone.wav");
        let samples: Vec<f32> = (0..16_000)
            .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / 16_000.0).sin() * 0.5)
            .collect();
        write_wav(&path, &samples).unwrap();

        let (decoded, duration) = decode_file(&path).unwrap();
        assert_eq!(decoded.len(), samples.len());
        assert!((duration - 1.0).abs() < 1e-6);
        let max_error = decoded
            .iter()
            .zip(&samples)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max);
        assert!(max_error < 1e-3, "max error {max_error}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_and_unsupported_files_fail_cleanly() {
        assert!(decode_file(Path::new("/definitely/not/here.wav")).is_err());
        let path = std::env::temp_dir().join(format!("speaktype-{}.txt", uuid::Uuid::new_v4()));
        std::fs::write(&path, "not audio").unwrap();
        assert!(decode_file(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
