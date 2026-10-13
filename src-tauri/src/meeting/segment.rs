//! Cuts a track into windows for the speech model while the meeting goes on.
//!
//! A window is 15–30 seconds, ended at a pause so words aren't split, or sooner
//! after a long pause so a quiet turn in the conversation shows up quickly.
//! Windows without speech are skipped: Whisper can invent words in silence.
//!
//! Only the window being built is kept in memory. Windows are reported as
//! sample ranges of the track, which the transcriber reads back from disk.

use crate::{audio::WHISPER_SAMPLE_RATE, live};

const RATE: usize = WHISPER_SAMPLE_RATE as usize;
const FRAME: usize = RATE / 50; // 20 ms
/// Below this RMS a frame counts as quiet, as in live dictation.
const QUIET_RMS: f32 = 0.006;

/// A window ends at the first pause after this much audio...
const MIN_WINDOW: usize = RATE * 15;
/// ...and is cut, at the quietest moment near the end, when it reaches this.
pub const MAX_WINDOW: usize = RATE * 30;
const PAUSE: usize = RATE * 6 / 10;
/// A shorter window ends at a long pause, so a quick reply isn't held back.
const EARLY_WINDOW: usize = RATE * 3;
const LONG_PAUSE: usize = RATE * 3 / 2;
/// How far back from `MAX_WINDOW` a forced cut looks for a quiet moment.
const CUT_SEARCH: usize = RATE * 3;

/// Samples `start..end` of a track, at 16 kHz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub start: u64,
    pub end: u64,
}

impl Window {
    pub fn len(&self) -> usize {
        (self.end - self.start) as usize
    }
}

#[derive(Default)]
pub struct Segmenter {
    pending: Vec<f32>,
    /// Track position of `pending[0]`.
    start: u64,
}

impl Segmenter {
    /// Adds 16 kHz audio and returns the windows it completes that hold speech.
    pub fn push(&mut self, samples: &[f32]) -> Vec<Window> {
        self.pending.extend_from_slice(samples);
        let mut windows = Vec::new();
        while let Some(end) = window_end(&self.pending, false) {
            windows.extend(self.take(end));
        }
        windows
    }

    /// The rest of the track, once recording has stopped.
    pub fn finish(&mut self) -> Vec<Window> {
        let mut windows = Vec::new();
        while let Some(end) = window_end(&self.pending, true) {
            windows.extend(self.take(end));
        }
        windows
    }

    fn take(&mut self, end: usize) -> Option<Window> {
        let speech = live::has_speech(&self.pending[..end]);
        let window = Window {
            start: self.start,
            end: self.start + end as u64,
        };
        self.pending.drain(..end);
        self.start = window.end;
        speech.then_some(window)
    }
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / samples.len().max(1) as f64)
        .sqrt() as f32
}

/// Where the next window ends in `samples`, if it's complete. `finished`
/// closes whatever is left.
fn window_end(samples: &[f32], finished: bool) -> Option<usize> {
    let limit = samples.len().min(MAX_WINDOW);
    let mut quiet = 0;
    for (index, frame) in samples[..limit].chunks(FRAME).enumerate() {
        quiet = if rms(frame) < QUIET_RMS {
            quiet + frame.len()
        } else {
            0
        };
        let end = index * FRAME + frame.len();
        if (end >= MIN_WINDOW && quiet >= PAUSE) || (end >= EARLY_WINDOW && quiet >= LONG_PAUSE) {
            return Some(end);
        }
    }
    if limit == MAX_WINDOW {
        // No pause long enough: cut at the quietest frame near the end.
        let start = MAX_WINDOW - CUT_SEARCH;
        let end = samples[start..MAX_WINDOW]
            .chunks(FRAME)
            .enumerate()
            .min_by(|(_, a), (_, b)| rms(a).total_cmp(&rms(b)))
            .map(|(i, frame)| start + i * FRAME + frame.len())
            .unwrap_or(MAX_WINDOW);
        return Some(end);
    }
    (finished && !samples.is_empty()).then_some(samples.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEECH: f32 = 0.08;

    fn speech(secs: f64) -> Vec<f32> {
        // Varies a little so it isn't one constant value.
        (0..(RATE as f64 * secs) as usize)
            .map(|i| SPEECH + (i % 7) as f32 / 1000.0)
            .collect()
    }

    fn silence(secs: f64) -> Vec<f32> {
        vec![0.0; (RATE as f64 * secs) as usize]
    }

    #[test]
    fn a_pause_after_fifteen_seconds_ends_the_window() {
        let mut segmenter = Segmenter::default();
        let mut audio = speech(16.0);
        audio.extend(silence(1.0));
        let windows = segmenter.push(&audio);
        assert_eq!(windows.len(), 1);
        let window = windows[0];
        assert_eq!(window.start, 0);
        // Right after the pause was long enough.
        assert_eq!(window.len(), RATE * 16 + PAUSE);
    }

    #[test]
    fn short_pauses_in_a_short_window_keep_it_open() {
        let mut segmenter = Segmenter::default();
        let mut audio = speech(5.0);
        audio.extend(silence(1.0));
        audio.extend(speech(5.0));
        assert!(segmenter.push(&audio).is_empty());
        // Everything left is closed when the meeting ends.
        let windows = segmenter.finish();
        assert_eq!(
            windows,
            [Window {
                start: 0,
                end: audio.len() as u64
            }]
        );
    }

    #[test]
    fn a_long_pause_ends_a_short_window_early() {
        let mut segmenter = Segmenter::default();
        let mut audio = speech(4.0);
        audio.extend(silence(2.0));
        let windows = segmenter.push(&audio);
        assert_eq!(
            windows,
            [Window {
                start: 0,
                end: (RATE * 4 + LONG_PAUSE) as u64
            }]
        );
    }

    #[test]
    fn continuous_speech_is_cut_by_thirty_seconds_without_losing_samples() {
        let mut segmenter = Segmenter::default();
        let audio = speech(95.0);
        let mut windows = Vec::new();
        // Arrives in quarter seconds, as from the capture loop.
        for block in audio.chunks(RATE / 4) {
            windows.extend(segmenter.push(block));
        }
        windows.extend(segmenter.finish());
        assert!(windows.len() >= 4);
        assert_eq!(windows[0].start, 0);
        for pair in windows.windows(2) {
            assert_eq!(pair[0].end, pair[1].start, "windows are contiguous");
        }
        assert!(windows.iter().all(|w| w.len() <= MAX_WINDOW));
        assert!(
            windows
                .iter()
                .all(|w| w.len() >= MAX_WINDOW - CUT_SEARCH || w.end as usize == audio.len())
        );
        assert_eq!(windows.last().unwrap().end, audio.len() as u64);
    }

    #[test]
    fn silence_is_skipped_and_never_piles_up() {
        let mut segmenter = Segmenter::default();
        let mut windows = Vec::new();
        // An hour of near silence, like a muted call.
        for _ in 0..3600 {
            windows.extend(segmenter.push(&vec![0.001; RATE]));
            assert!(segmenter.pending.len() <= MAX_WINDOW + RATE);
        }
        assert!(windows.is_empty());
        // Speech afterwards keeps its position on the track.
        let mut audio = speech(4.0);
        audio.extend(silence(2.0));
        let windows = segmenter.push(&audio);
        assert_eq!(windows.len(), 1);
        assert!(windows[0].start >= (RATE * 3600 - MAX_WINDOW - RATE) as u64);
        assert!(windows[0].end <= (RATE * 3606) as u64);
    }

    #[test]
    fn nothing_left_means_no_window() {
        let mut segmenter = Segmenter::default();
        assert!(segmenter.finish().is_empty());
        assert!(segmenter.push(&[]).is_empty());
        assert!(segmenter.finish().is_empty());
    }
}
