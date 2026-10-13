//! Lines up the microphone and system audio tracks, keeps the speakers'
//! sound out of the microphone track, and mixes both into one recording.
//!
//! The two sources run on separate clocks and don't always deliver: Windows
//! sends no system audio at all while nothing plays. `Aligner` hands out equal
//! lengths of both, filling a source that falls behind with silence, so the
//! tracks share one timeline.

use std::{fs, io::BufWriter, path::Path};

use crate::audio::WHISPER_SAMPLE_RATE;

const RATE: usize = WHISPER_SAMPLE_RATE as usize;
const FRAME: usize = RATE / 50; // 20 ms

/// How far one source may run ahead before the other is filled with silence.
pub const MAX_LAG: usize = RATE;

#[derive(Default)]
pub struct Aligner {
    mic: Vec<f32>,
    system: Vec<f32>,
}

impl Aligner {
    pub fn push(&mut self, mic: &[f32], system: &[f32]) {
        self.mic.extend_from_slice(mic);
        self.system.extend_from_slice(system);
    }

    /// Equal lengths of microphone and system audio, as far as both have
    /// arrived. A source more than `MAX_LAG` behind is taken to have gone
    /// quiet and is filled with silence all the way, as is any source once
    /// `finished`. Filling it only up to the lag would leave whatever it sends
    /// next that far early for the rest of the meeting.
    pub fn take(&mut self, finished: bool) -> (Vec<f32>, Vec<f32>) {
        let longest = self.mic.len().max(self.system.len());
        for track in [&mut self.mic, &mut self.system] {
            if finished || longest - track.len() > MAX_LAG {
                track.resize(longest, 0.0);
            }
        }
        let n = self.mic.len().min(self.system.len());
        (
            self.mic.drain(..n).collect(),
            self.system.drain(..n).collect(),
        )
    }
}

/// Microphone frames quieter than this share of the system audio around them
/// are taken to be the speakers heard through the microphone, and silenced so
/// the other side isn't transcribed twice, once as "You".
const BLEED_RATIO: f32 = 0.7;
/// System audio this quiet can't be heard through the microphone.
const AUDIBLE_RMS: f32 = 0.006;
/// Frames either side checked for system audio, for the delay between the
/// two sources and the room's echo.
const BLEED_REACH: usize = 5;

/// Silences the microphone where the system audio is louder than it. With
/// headphones there is nothing to remove; through speakers this drops the
/// other side's voices, at the cost of talking over them.
pub fn suppress_bleed(mic: &mut [f32], system: &[f32]) {
    let level = |samples: &[f32]| rms(samples);
    let system_levels: Vec<f32> = system.chunks(FRAME).map(level).collect();
    for (i, frame) in mic.chunks_mut(FRAME).enumerate() {
        let near = system_levels
            .get(i.saturating_sub(BLEED_REACH)..(i + BLEED_REACH + 1).min(system_levels.len()))
            .unwrap_or_default();
        let loudest = near.iter().copied().fold(0.0, f32::max);
        if loudest > AUDIBLE_RMS && level(frame) < loudest * BLEED_RATIO {
            frame.fill(0.0);
        }
    }
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / samples.len().max(1) as f64)
        .sqrt() as f32
}

/// Adds the two tracks, clipped to full scale. A missing sample is silence.
pub fn mix(a: &[f32], b: &[f32]) -> Vec<f32> {
    (0..a.len().max(b.len()))
        .map(|i| {
            let sum = a.get(i).copied().unwrap_or(0.0) + b.get(i).copied().unwrap_or(0.0);
            sum.clamp(-1.0, 1.0)
        })
        .collect()
}

pub fn wav_spec() -> hound::WavSpec {
    hound::WavSpec {
        channels: 1,
        sample_rate: WHISPER_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    }
}

pub fn to_i16(sample: f32) -> i16 {
    // NaN clamps to NaN, which the cast turns into silence.
    (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16
}

pub fn from_i16(sample: i16) -> f32 {
    f32::from(sample) / f32::from(i16::MAX)
}

/// Mixes two 16 kHz mono WAV files into `out`, a second at a time, so an
/// hour-long meeting never has to fit in memory. Writes to a temporary file
/// and renames it into place.
pub fn mix_files(a: &Path, b: &Path, out: &Path) -> Result<(), String> {
    let tmp = {
        let mut name = out.file_name().unwrap_or_default().to_os_string();
        name.push(".part");
        out.with_file_name(name)
    };
    let result =
        mix_into(a, b, &tmp).and_then(|()| fs::rename(&tmp, out).map_err(|e| e.to_string()));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn mix_into(a: &Path, b: &Path, out: &Path) -> Result<(), String> {
    let open = |path: &Path| hound::WavReader::open(path).map_err(|e| e.to_string());
    let mut a = open(a)?;
    let mut b = open(b)?;
    let mut writer = hound::WavWriter::new(
        BufWriter::new(fs::File::create(out).map_err(|e| e.to_string())?),
        wav_spec(),
    )
    .map_err(|e| e.to_string())?;
    let mut a = a.samples::<i16>();
    let mut b = b.samples::<i16>();
    let read = |samples: &mut hound::WavSamples<'_, _, i16>| -> Vec<f32> {
        // A truncated file, as after a crash, ends at its last whole sample.
        samples
            .by_ref()
            .take(RATE)
            .map_while(Result::ok)
            .map(from_i16)
            .collect()
    };
    loop {
        let (left, right) = (read(&mut a), read(&mut b));
        if left.is_empty() && right.is_empty() {
            break;
        }
        for sample in mix(&left, &right) {
            writer
                .write_sample(to_i16(sample))
                .map_err(|e| e.to_string())?;
        }
    }
    writer.finalize().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligned_tracks_come_out_in_step() {
        let mut aligner = Aligner::default();
        aligner.push(&[0.1; 100], &[0.2; 60]);
        let (mic, system) = aligner.take(false);
        assert_eq!((mic.len(), system.len()), (60, 60));
        // The rest waits for the system audio to catch up.
        aligner.push(&[], &[0.2; 50]);
        let (mic, system) = aligner.take(false);
        assert_eq!((mic.len(), system.len()), (40, 40));
        assert!(mic.iter().all(|s| *s == 0.1));
        let (mic, system) = aligner.take(true);
        assert_eq!(mic, vec![0.0; 10], "filled at the end");
        assert_eq!(system, vec![0.2; 10]);
    }

    #[test]
    fn a_silent_source_is_filled_once_it_lags_too_far() {
        // Windows sends nothing while nothing plays.
        let mut aligner = Aligner::default();
        aligner.push(&vec![0.1; MAX_LAG / 2], &[]);
        assert_eq!(aligner.take(false).0.len(), 0);
        aligner.push(&vec![0.1; MAX_LAG], &[]);
        let (mic, system) = aligner.take(false);
        assert_eq!(mic.len(), MAX_LAG * 3 / 2, "filled all the way");
        assert!(system.iter().all(|s| *s == 0.0));
        // Playback resumes in step with the microphone, not a second early.
        aligner.push(&[0.1; 10], &[0.3; 10]);
        let (mic, system) = aligner.take(false);
        assert_eq!(mic, vec![0.1; 10]);
        assert_eq!(system, vec![0.3; 10]);
    }

    #[test]
    fn speaker_sound_in_the_microphone_is_removed() {
        let frames = 40;
        // The other side talks for the first half, heard faintly by the microphone.
        let system: Vec<f32> = (0..FRAME * frames)
            .map(|i| if i < FRAME * frames / 2 { 0.2 } else { 0.0 })
            .collect();
        let mut mic: Vec<f32> = (0..FRAME * frames)
            .map(|i| if i < FRAME * frames / 2 { 0.05 } else { 0.1 })
            .collect();
        suppress_bleed(&mut mic, &system);
        assert!(mic[..FRAME * frames / 2].iter().all(|s| *s == 0.0));
        // The user's own voice, once the other side is quiet.
        assert!(
            mic[FRAME * (frames / 2 + BLEED_REACH + 1)..]
                .iter()
                .all(|s| *s == 0.1)
        );
    }

    #[test]
    fn talking_louder_than_the_speakers_is_kept() {
        let system = vec![0.05; FRAME * 10];
        let mut mic = vec![0.2; FRAME * 10];
        suppress_bleed(&mut mic, &system);
        assert!(mic.iter().all(|s| *s == 0.2));
        // Quiet system audio never silences anything.
        let mut quiet_mic = vec![0.001; FRAME * 10];
        suppress_bleed(&mut quiet_mic, &[0.002; FRAME * 10]);
        assert!(quiet_mic.iter().all(|s| *s == 0.001));
        // No system audio to compare with.
        suppress_bleed(&mut quiet_mic, &[]);
        assert!(quiet_mic.iter().all(|s| *s == 0.001));
    }

    #[test]
    fn mixing_adds_and_clips() {
        assert_eq!(
            mix(&[0.25, 0.9, -0.9], &[0.25, 0.5, -0.5, 0.1]),
            [0.5, 1.0, -1.0, 0.1]
        );
        assert!(mix(&[], &[]).is_empty());
        assert_eq!(to_i16(2.0), i16::MAX);
        assert_eq!(to_i16(f32::NAN), 0);
        assert!((from_i16(to_i16(0.5)) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn files_of_different_lengths_mix_into_the_longer_one() {
        let dir = std::env::temp_dir().join(format!("speaktype-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, samples: &[f32]| {
            let path = dir.join(name);
            let mut writer = hound::WavWriter::create(&path, wav_spec()).unwrap();
            for s in samples {
                writer.write_sample(to_i16(*s)).unwrap();
            }
            writer.finalize().unwrap();
            path
        };
        let a = write("a.wav", &vec![0.25; RATE * 2 + 5]);
        let b = write("b.wav", &vec![0.25; RATE / 2]);
        let out = dir.join("mixed.wav");
        mix_files(&a, &b, &out).unwrap();

        let reader = hound::WavReader::open(&out).unwrap();
        assert_eq!(reader.spec(), wav_spec());
        let samples: Vec<f32> = reader
            .into_samples::<i16>()
            .map(|s| from_i16(s.unwrap()))
            .collect();
        assert_eq!(samples.len(), RATE * 2 + 5);
        assert!((samples[0] - 0.5).abs() < 1e-3);
        assert!((samples[RATE] - 0.25).abs() < 1e-3);
        assert!(!dir.join("mixed.wav.part").exists());

        assert!(mix_files(&a, &dir.join("missing.wav"), &out).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
