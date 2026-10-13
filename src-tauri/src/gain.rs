//! Whisper mode: raises quiet speech to a normal level before transcription.
//!
//! Speaking softly gives the speech models a faint signal, and the silence
//! checks in `live` and `pipeline` mistake it for silence. When the speech in
//! a recording is quiet, it is amplified towards a normal level first.
//! Amplifying by a factor lowers every silence threshold by the same factor,
//! so those checks need no changes of their own.
//!
//! The gain is limited so that a quiet room never becomes "speech" that
//! Whisper would invent words for:
//! - nothing is amplified unless some of it is clearly louder than the rest,
//! - background noise stays below the silence thresholds,
//! - the loudest sample doesn't clip, and the gain never exceeds `MAX_GAIN_DB`.
//!
//! Loudness is measured per 20 ms frame. Speech level is a high percentile of
//! frame loudness and background noise a low one, so the estimate holds up
//! when most of a recording is pauses.

use std::borrow::Cow;

use crate::audio::WHISPER_SAMPLE_RATE;

const FRAME: usize = WHISPER_SAMPLE_RATE as usize / 50;
/// Speech is raised towards this frame loudness, in dBFS: ordinary speech
/// close to a laptop microphone.
const TARGET_DB: f32 = -22.0;
/// At most 16x.
const MAX_GAIN_DB: f32 = 24.0;
/// Speech must be this much louder than the background to be amplified.
const MIN_SNR_DB: f32 = 12.0;
/// Amplified background noise stays below this. The silence checks treat
/// frames under about -44 dBFS (RMS 0.006) as quiet.
const NOISE_CEILING_DB: f32 = -52.0;
/// The loudest sample is kept below this, in dBFS.
const PEAK_CEILING_DB: f32 = -1.0;
/// Less audio than this (0.3 s) says too little about the levels.
const MIN_FRAMES: u32 = 15;
const SPEECH_PERCENTILE: f32 = 0.95;
const NOISE_PERCENTILE: f32 = 0.10;
/// Frame loudness histogram, one bin per dB from -100 dBFS to 0 dBFS.
const FLOOR_DB: f32 = -100.0;
const BINS: usize = 100;

/// Measures audio as it arrives and says how much to amplify it.
#[derive(Debug, Clone)]
pub struct Booster {
    histogram: [u32; BINS],
    frames: u32,
    peak: f32,
    /// Sum of squares and count of the samples in the unfinished frame.
    partial: (f64, usize),
}

impl Default for Booster {
    fn default() -> Self {
        Self {
            histogram: [0; BINS],
            frames: 0,
            peak: 0.0,
            partial: (0.0, 0),
        }
    }
}

impl Booster {
    pub fn push(&mut self, samples: &[f32]) {
        for &sample in samples {
            // `max` ignores NaN, and a frame with NaN in it measures as silence.
            self.peak = self.peak.max(sample.abs());
            self.partial.0 += f64::from(sample) * f64::from(sample);
            self.partial.1 += 1;
            if self.partial.1 == FRAME {
                let rms = (self.partial.0 / FRAME as f64).sqrt() as f32;
                self.histogram[bin(rms)] += 1;
                self.frames += 1;
                self.partial = (0.0, 0);
            }
        }
    }

    /// The factor to multiply the audio by: 1 when it is loud enough, too
    /// short to judge, or not clearly speech.
    pub fn gain(&self) -> f32 {
        if self.frames < MIN_FRAMES {
            return 1.0;
        }
        let speech = self.percentile(SPEECH_PERCENTILE);
        let noise = self.percentile(NOISE_PERCENTILE);
        if speech - noise < MIN_SNR_DB {
            return 1.0;
        }
        let peak = 20.0 * self.peak.max(1e-6).log10();
        let db = (TARGET_DB - speech)
            .min(MAX_GAIN_DB)
            .min(NOISE_CEILING_DB - noise)
            .min(PEAK_CEILING_DB - peak);
        if db > 0.0 { 10f32.powf(db / 20.0) } else { 1.0 }
    }

    /// Frame loudness in dBFS that `fraction` of frames are at or below,
    /// taking the top of the bin.
    fn percentile(&self, fraction: f32) -> f32 {
        let wanted = (self.frames as f32 * fraction).ceil().max(1.0) as u32;
        let mut seen = 0;
        for (index, count) in self.histogram.iter().enumerate() {
            seen += count;
            if seen >= wanted {
                return FLOOR_DB + index as f32 + 1.0;
            }
        }
        0.0
    }
}

/// `max` turns a NaN level into the quietest bin.
fn bin(rms: f32) -> usize {
    let db = 20.0 * rms.max(1e-6).log10();
    ((db - FLOOR_DB).floor().max(0.0) as usize).min(BINS - 1)
}

/// `samples` amplified by `gain`, or unchanged when the gain is 1.
pub fn amplify(samples: &[f32], gain: f32) -> Cow<'_, [f32]> {
    if gain == 1.0 {
        Cow::Borrowed(samples)
    } else {
        Cow::Owned(samples.iter().map(|s| s * gain).collect())
    }
}

/// A whole recording, with quiet speech raised to a normal level.
pub fn boost(samples: &[f32]) -> Cow<'_, [f32]> {
    let mut booster = Booster::default();
    booster.push(samples);
    amplify(samples, booster.gain())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::has_speech;

    const RATE: usize = WHISPER_SAMPLE_RATE as usize;

    /// Deterministic white noise with the given RMS.
    fn noise(rms: f32, len: usize) -> Vec<f32> {
        let mut state = 0x2545_f491_u32;
        (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                // Uniform in -1..1 has an RMS of 1/sqrt(3).
                (state as f32 / u32::MAX as f32 * 2.0 - 1.0) * rms * 3f32.sqrt()
            })
            .collect()
    }

    /// Syllable-like bursts of a voiced tone at `speech_rms`, half a second
    /// on and a quarter off, over background noise at `noise_rms`.
    fn murmur(speech_rms: f32, noise_rms: f32, secs: f32) -> Vec<f32> {
        let len = (RATE as f32 * secs) as usize;
        let amplitude = speech_rms * 2f32.sqrt();
        noise(noise_rms, len)
            .into_iter()
            .enumerate()
            .map(|(i, n)| {
                let t = i as f32 / RATE as f32;
                let voiced = t % 0.75 < 0.5;
                let tone = (t * 180.0 * std::f32::consts::TAU).sin()
                    + 0.5 * (t * 360.0 * std::f32::consts::TAU).sin();
                if voiced {
                    n + amplitude * tone / 1.25f32.sqrt()
                } else {
                    n
                }
            })
            .collect()
    }

    fn db(gain: f32) -> f32 {
        20.0 * gain.log10()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn whispered_speech_is_raised_until_the_silence_checks_hear_it() {
        // Speech around -52 dBFS in a very quiet room, at -86 dBFS.
        let quiet = murmur(0.0025, 0.00005, 3.0);
        assert!(!has_speech(&quiet), "too quiet to pass on its own");
        let boosted = boost(&quiet);
        assert!(matches!(boosted, Cow::Owned(_)));
        assert!(has_speech(&boosted));
        let gain = boosted[1000] / quiet[1000];
        assert!(
            (db(gain) - MAX_GAIN_DB).abs() < 0.01,
            "gain {} dB",
            db(gain)
        );
    }

    #[test]
    fn soft_speech_is_raised_to_the_target_without_overshooting() {
        let soft = murmur(0.01, 0.0002, 3.0);
        let boosted = boost(&soft);
        let gain = boosted[1000] / soft[1000];
        assert!(db(gain) > 10.0 && db(gain) < MAX_GAIN_DB, "{} dB", db(gain));
        // The loudest frames land close to the target, within a bin or two.
        let mut booster = Booster::default();
        booster.push(&boosted);
        let speech = booster.percentile(SPEECH_PERCENTILE);
        assert!((speech - TARGET_DB).abs() <= 2.0, "speech at {speech} dBFS");
        assert!(db(booster.gain()) <= 2.0, "boosting again changes little");
    }

    #[test]
    fn silence_and_steady_noise_are_never_amplified() {
        assert_eq!(boost(&vec![0.0; RATE * 3]).as_ref(), vec![0.0; RATE * 3]);
        for level in [0.0005, 0.002, 0.004] {
            let hiss = noise(level, RATE * 3);
            assert!(matches!(boost(&hiss), Cow::Borrowed(_)), "noise at {level}");
            assert!(!has_speech(&boost(&hiss)));
        }
    }

    #[test]
    fn noisy_rooms_keep_their_background_below_the_silence_thresholds() {
        // Soft speech in a noisy room: amplified, but only as far as the noise allows.
        let noisy = murmur(0.006, 0.0006, 3.0);
        let boosted = boost(&noisy);
        let gain = boosted[1000] / noisy[1000];
        assert!(gain > 1.0);
        // The pauses between bursts.
        let gaps: Vec<f32> = boosted
            .iter()
            .enumerate()
            .filter(|(i, _)| (0.55..0.7).contains(&(*i as f32 / RATE as f32 % 0.75)))
            .map(|(_, s)| *s)
            .collect();
        assert!(rms(&gaps) < 0.006, "background at {}", rms(&gaps));
    }

    #[test]
    fn normal_speech_and_short_clips_are_left_alone() {
        assert!(matches!(boost(&murmur(0.1, 0.001, 2.0)), Cow::Borrowed(_)));
        assert!(matches!(
            boost(&murmur(0.002, 0.0001, 0.2)),
            Cow::Borrowed(_)
        ));
        assert!(matches!(boost(&[]), Cow::Borrowed(_)));
    }

    #[test]
    fn the_loudest_sample_never_clips() {
        let mut quiet = murmur(0.0025, 0.00025, 3.0);
        quiet[RATE] = 0.5;
        let boosted = boost(&quiet);
        let peak = boosted.iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.5 && peak <= 0.9, "peak {peak}");
    }

    #[test]
    fn measuring_in_pieces_matches_measuring_at_once() {
        let audio = murmur(0.004, 0.0003, 2.5);
        let mut streamed = Booster::default();
        for piece in audio.chunks(1234) {
            streamed.push(piece);
        }
        let mut whole = Booster::default();
        whole.push(&audio);
        assert_eq!(streamed.gain(), whole.gain());
        assert!(whole.gain() > 1.0);
    }

    #[test]
    fn nan_samples_count_as_silence() {
        let mut booster = Booster::default();
        booster.push(&vec![f32::NAN; RATE]);
        assert_eq!(booster.gain(), 1.0);
    }
}
