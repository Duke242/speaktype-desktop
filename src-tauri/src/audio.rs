//! Microphone capture with cpal.
//!
//! Each recording runs on its own thread, because a cpal stream can't be moved
//! between threads on every platform. Audio is kept in memory as mono f32 at the
//! device's rate and resampled to 16 kHz for Whisper when the recording stops.

use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant},
};

use cpal::{
    FromSample, Sample, SampleFormat, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use serde::Serialize;

pub const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// How often the level meter is updated while recording.
const LEVEL_INTERVAL: Duration = Duration::from_millis(33);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

pub struct Captured {
    /// Mono samples at 16 kHz.
    pub samples: Vec<f32>,
    pub duration_secs: f64,
}

pub fn list_input_devices() -> Vec<InputDevice> {
    let host = cpal::default_host();
    let default_id = host
        .default_input_device()
        .and_then(|d| d.id().ok())
        .map(|id| id.to_string());
    let Ok(devices) = host.input_devices() else {
        return Vec::new();
    };
    devices
        .filter_map(|device| {
            let id = device.id().ok()?.to_string();
            let name = device
                .description()
                .map(|d| d.name().to_string())
                .unwrap_or_else(|_| id.clone());
            // Same filter as the macOS app: Teams installs a virtual mic that records nothing useful.
            if name.to_lowercase().contains("microsoft teams") {
                return None;
            }
            Some(InputDevice {
                is_default: default_id.as_deref() == Some(id.as_str()),
                id,
                name,
            })
        })
        .collect()
}

pub struct Recording {
    stop_tx: Sender<()>,
    done_rx: Receiver<Result<Captured, String>>,
}

impl Recording {
    /// Starts recording from `device_id`, or the default input if it is empty or
    /// no longer connected. `on_level` receives the peak level (0..1) of recent audio.
    pub fn start(
        device_id: &str,
        on_level: impl Fn(f32) + Send + 'static,
    ) -> Result<Recording, String> {
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let (done_tx, done_rx) = mpsc::channel();
        let device_id = device_id.to_string();

        thread::Builder::new()
            .name("audio-capture".into())
            .spawn(move || {
                let started = Instant::now();
                let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
                let (stream, sample_rate) = match open_stream(&device_id, buffer.clone(), on_level)
                {
                    Ok(opened) => opened,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(()));

                // Blocks until `finish` is called or the Recording is dropped.
                let _ = stop_rx.recv();
                drop(stream);

                let samples = std::mem::take(&mut *buffer.lock().unwrap());
                let samples = resample(&samples, sample_rate, WHISPER_SAMPLE_RATE);
                let _ = done_tx.send(Ok(Captured {
                    duration_secs: started.elapsed().as_secs_f64(),
                    samples,
                }));
            })
            .map_err(|e| e.to_string())?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Recording { stop_tx, done_rx }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("Audio thread exited before the microphone opened".into()),
        }
    }

    /// Stops the microphone and returns the audio.
    pub fn finish(self) -> Result<Captured, String> {
        let _ = self.stop_tx.send(());
        self.done_rx
            .recv()
            .unwrap_or_else(|_| Err("Audio thread exited unexpectedly".into()))
    }
}

fn open_stream(
    device_id: &str,
    buffer: Arc<Mutex<Vec<f32>>>,
    on_level: impl Fn(f32) + Send + 'static,
) -> Result<(cpal::Stream, u32), String> {
    let host = cpal::default_host();
    let device = device_id
        .parse()
        .ok()
        .and_then(|id| host.device_by_id(&id))
        .or_else(|| host.default_input_device())
        .ok_or("No microphone found")?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("Microphone config: {e}"))?;
    let sample_rate = config.sample_rate();
    let channels = config.channels() as usize;
    let format = config.sample_format();
    let stream_config = config.config();

    let meter = LevelMeter::new(on_level);
    let stream = match format {
        SampleFormat::I8 => build::<i8>(&device, &stream_config, channels, buffer, meter),
        SampleFormat::I16 => build::<i16>(&device, &stream_config, channels, buffer, meter),
        SampleFormat::I32 => build::<i32>(&device, &stream_config, channels, buffer, meter),
        SampleFormat::U8 => build::<u8>(&device, &stream_config, channels, buffer, meter),
        SampleFormat::U16 => build::<u16>(&device, &stream_config, channels, buffer, meter),
        SampleFormat::F32 => build::<f32>(&device, &stream_config, channels, buffer, meter),
        SampleFormat::F64 => build::<f64>(&device, &stream_config, channels, buffer, meter),
        other => return Err(format!("Unsupported microphone sample format {other:?}")),
    }?;
    stream
        .play()
        .map_err(|e| format!("Couldn't start the microphone: {e}"))?;
    Ok((stream, sample_rate))
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    buffer: Arc<Mutex<Vec<f32>>>,
    mut meter: LevelMeter,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    device
        .build_input_stream(
            *config,
            move |data: &[T], _: &_| {
                let mut buf = buffer.lock().unwrap();
                let start = buf.len();
                // Downmix to mono by averaging each frame's channels.
                buf.extend(data.chunks(channels.max(1)).map(|frame| {
                    frame.iter().map(|s| f32::from_sample(*s)).sum::<f32>() / frame.len() as f32
                }));
                meter.push(&buf[start..]);
            },
            |err| eprintln!("[audio] stream error: {err}"),
            None,
        )
        .map_err(|e| format!("Couldn't open the microphone: {e}"))
}

/// Turns incoming audio into a steady 0..1 loudness for the pill's waveform,
/// the same way the macOS app does: loudness in decibels rather than raw
/// amplitude, a gate so room noise stays flat, and smoothing that rises fast
/// and falls slowly.
struct LevelMeter {
    callback: Box<dyn Fn(f32) + Send>,
    sum_squares: f64,
    count: usize,
    peak: f32,
    smoothed: f32,
    last_sent: Instant,
}

impl LevelMeter {
    /// Quietest level shown, in dBFS. Everything below reads as silence.
    const FLOOR_DB: f32 = -58.0;
    /// Share of the scale treated as background noise and cut off.
    const NOISE_GATE: f32 = 0.25;
    const RISE: f32 = 0.55;
    const FALL: f32 = 0.18;

    fn new(callback: impl Fn(f32) + Send + 'static) -> Self {
        Self {
            callback: Box::new(callback),
            sum_squares: 0.0,
            count: 0,
            peak: 0.0,
            smoothed: 0.0,
            last_sent: Instant::now(),
        }
    }

    fn push(&mut self, samples: &[f32]) {
        for s in samples {
            self.sum_squares += (*s as f64) * (*s as f64);
            self.peak = self.peak.max(s.abs());
        }
        self.count += samples.len();
        if self.last_sent.elapsed() < LEVEL_INTERVAL || self.count == 0 {
            return;
        }

        let rms = (self.sum_squares / self.count as f64).sqrt() as f32;
        let level = level_from_amplitudes(rms, self.peak);
        let rate = if level > self.smoothed { Self::RISE } else { Self::FALL };
        self.smoothed += (level - self.smoothed) * rate;
        (self.callback)(self.smoothed);

        self.sum_squares = 0.0;
        self.count = 0;
        self.peak = 0.0;
        self.last_sent = Instant::now();
    }
}

/// Maps RMS and peak amplitude (0..1) to a gated 0..1 display level.
fn level_from_amplitudes(rms: f32, peak: f32) -> f32 {
    let normalize = |amplitude: f32| {
        let db = 20.0 * amplitude.max(1e-5).log10();
        ((db - LevelMeter::FLOOR_DB) / -LevelMeter::FLOOR_DB).clamp(0.0, 1.0)
    };
    let level = (normalize(rms) * 0.8).max(normalize(peak));
    ((level - LevelMeter::NOISE_GATE) / (1.0 - LevelMeter::NOISE_GATE)).clamp(0.0, 1.0)
}

/// Converts mono audio between sample rates using windowed-sinc interpolation.
/// When downsampling, the filter cutoff drops to the new Nyquist frequency so
/// high frequencies don't fold back into the speech band.
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    const ZERO_CROSSINGS: f64 = 12.0;
    let ratio = to as f64 / from as f64;
    let cutoff = ratio.min(1.0) * 0.95;
    let radius = ZERO_CROSSINGS / cutoff;
    let out_len = (input.len() as f64 * ratio).round() as usize;
    let last = input.len() as isize - 1;

    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let center = i as f64 / ratio;
        let lo = ((center - radius).ceil() as isize).max(0);
        let hi = ((center + radius).floor() as isize).min(last);
        let mut sum = 0.0;
        let mut weight = 0.0;
        for j in lo..=hi {
            let x = j as f64 - center;
            let t = std::f64::consts::PI * x * cutoff;
            let sinc = if t.abs() < 1e-9 { 1.0 } else { t.sin() / t };
            // Hann window over the filter's width.
            let window = 0.5 + 0.5 * (std::f64::consts::PI * x / radius).cos();
            let w = sinc * window;
            sum += input[j as usize] as f64 * w;
            weight += w;
        }
        out.push(if weight.abs() > 1e-9 {
            (sum / weight) as f32
        } else {
            0.0
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f64, rate: u32, secs: f64) -> Vec<f32> {
        let n = (rate as f64 * secs) as usize;
        (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * freq * i as f64 / rate as f64).sin() as f32)
            .collect()
    }

    fn rms(samples: &[f32]) -> f64 {
        let body = &samples[samples.len() / 10..samples.len() * 9 / 10];
        (body.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / body.len() as f64).sqrt()
    }

    #[test]
    fn level_is_flat_for_silence_and_noise_and_high_for_speech() {
        assert_eq!(level_from_amplitudes(0.0, 0.0), 0.0);
        // Room noise around -50 dBFS stays below the gate.
        assert_eq!(level_from_amplitudes(0.002, 0.004), 0.0);
        // Normal speech around -20 dBFS peaks shows clearly.
        let speech = level_from_amplitudes(0.03, 0.1);
        assert!(speech > 0.4 && speech < 0.8, "speech level {speech}");
        assert_eq!(level_from_amplitudes(1.0, 1.0), 1.0);
    }

    #[test]
    fn output_length_follows_the_rate_ratio() {
        assert_eq!(resample(&vec![0.0; 48_000], 48_000, 16_000).len(), 16_000);
        assert_eq!(resample(&vec![0.0; 44_100], 44_100, 16_000).len(), 16_000);
        assert_eq!(resample(&vec![0.0; 8_000], 8_000, 16_000).len(), 16_000);
    }

    #[test]
    fn same_rate_is_a_copy() {
        let input = vec![0.1, -0.2, 0.3];
        assert_eq!(resample(&input, 16_000, 16_000), input);
    }

    #[test]
    fn speech_band_tones_keep_their_level() {
        for rate in [44_100, 48_000] {
            let out = resample(&sine(440.0, rate, 1.0), rate, 16_000);
            let expected = std::f64::consts::FRAC_1_SQRT_2;
            assert!(
                (rms(&out) - expected).abs() < 0.02,
                "rate {rate}: {}",
                rms(&out)
            );
        }
    }

    #[test]
    fn tones_above_the_new_nyquist_are_removed() {
        let out = resample(&sine(12_000.0, 48_000, 1.0), 48_000, 16_000);
        assert!(rms(&out) < 0.05, "aliasing left rms {}", rms(&out));
    }
}
