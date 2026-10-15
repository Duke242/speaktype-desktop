//! Audio sources for meetings: the microphone, and what the computer plays.
//!
//! Each source records on its own thread, as in `audio`, because a cpal stream
//! can't move between threads on every platform. The device callback only
//! appends mono samples to a buffer that the meeting's capture loop empties
//! several times a second, so nothing grows with the meeting's length.
//!
//! System audio is OS-specific (`platform::open_system_audio`). On Windows
//! and macOS it's cpal's loopback: an input stream built on an output device.
//! On Linux it's the output's PulseAudio or PipeWire monitor.

use std::{
    any::Any,
    sync::{
        Arc, Mutex,
        mpsc::{self, Sender},
    },
    thread::{self, JoinHandle},
};

use cpal::{
    FromSample, Sample, SampleFormat, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use serde::Serialize;

use crate::{
    LockExt,
    audio::{self, WHISPER_SAMPLE_RATE},
};

/// Mono samples at the source's rate, appended by its callback.
pub type Buffer = Arc<Mutex<Vec<f32>>>;

/// A source that's recording. Whatever has to stay alive for it to keep
/// recording, like a cpal stream or a helper process, is held in `keep`.
pub struct Opened {
    pub keep: Box<dyn Any>,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    /// Records what an output plays.
    Output,
    /// An input device, such as a virtual one like BlackHole that apps play into.
    Input,
}

/// A choice of where system audio comes from.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSource {
    /// Empty for the system's default output.
    pub id: String,
    pub name: String,
    pub kind: SourceKind,
}

const OUTPUT_PREFIX: &str = "output:";
const INPUT_PREFIX: &str = "input:";

/// The default output, recorded through loopback where the OS supports it.
pub fn default_output_source() -> AudioSource {
    AudioSource {
        id: String::new(),
        name: "Default output".into(),
        kind: SourceKind::Output,
    }
}

fn describe(device: &cpal::Device) -> Option<(String, String)> {
    let id = device.id().ok()?.to_string();
    let name = device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| id.clone());
    Some((id, name))
}

/// Output devices, for loopback recording.
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub fn list_outputs() -> Vec<AudioSource> {
    let Ok(devices) = cpal::default_host().output_devices() else {
        return Vec::new();
    };
    devices
        .filter_map(|device| describe(&device))
        .map(|(id, name)| AudioSource {
            id: format!("{OUTPUT_PREFIX}{id}"),
            name,
            kind: SourceKind::Output,
        })
        .collect()
}

/// Input devices, for a virtual device that apps play into.
pub fn list_inputs() -> Vec<AudioSource> {
    let Ok(devices) = cpal::default_host().input_devices() else {
        return Vec::new();
    };
    devices
        .filter_map(|device| describe(&device))
        .map(|(id, name)| AudioSource {
            id: format!("{INPUT_PREFIX}{id}"),
            name,
            kind: SourceKind::Input,
        })
        .collect()
}

/// Opens a source listed by `list_outputs` or `list_inputs`. `None` when
/// `source` is neither, such as the empty id for the default output.
pub fn open_listed(source: &str, buffer: &Buffer) -> Option<Result<Opened, String>> {
    if let Some(id) = source.strip_prefix(INPUT_PREFIX) {
        return Some(open_microphone(id, buffer));
    }
    let id = source.strip_prefix(OUTPUT_PREFIX)?;
    Some(open_output(Some(id), buffer))
}

/// Records from an input device, or the default one if `device_id` is empty
/// or no longer connected.
pub fn open_microphone(device_id: &str, buffer: &Buffer) -> Result<Opened, String> {
    let host = cpal::default_host();
    let device = device_id
        .parse()
        .ok()
        .and_then(|id| host.device_by_id(&id))
        .or_else(|| host.default_input_device())
        .ok_or("No microphone found")?;
    open_device(&device, buffer, false)
}

/// Records what an output device plays, or the default output's. cpal turns
/// an input stream on an output device into loopback recording on Windows
/// (WASAPI) and macOS 14.2 or later (a Core Audio process tap).
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub fn open_output(device_id: Option<&str>, buffer: &Buffer) -> Result<Opened, String> {
    let host = cpal::default_host();
    let device = match device_id {
        Some(id) => id
            .parse()
            .ok()
            .and_then(|id| host.device_by_id(&id))
            .ok_or("That output isn't connected")?,
        None => host
            .default_output_device()
            .ok_or("No audio output found")?,
    };
    open_device(&device, buffer, true)
}

fn open_device(device: &cpal::Device, buffer: &Buffer, loopback: bool) -> Result<Opened, String> {
    let name = describe(device).map(|(_, name)| name).unwrap_or_default();
    // cpal only records an output's sound when the device has no input of its
    // own, otherwise it would open that, like a headset's microphone.
    if loopback && device.supports_input() {
        return Err(format!(
            "{name} is also a microphone, so Wordflow can't record what it plays. \
             Choose another output, or a virtual device such as BlackHole"
        ));
    }
    let config = if loopback {
        device.default_output_config()
    } else {
        device.default_input_config()
    }
    .map_err(|e| format!("{name}: {e}"))?;
    let sample_rate = config.sample_rate();
    let channels = usize::from(config.channels()).max(1);
    let format = config.sample_format();
    let stream_config = config.config();
    let buffer = buffer.clone();
    let stream = match format {
        SampleFormat::I8 => build::<i8>(device, &stream_config, channels, buffer),
        SampleFormat::I16 => build::<i16>(device, &stream_config, channels, buffer),
        SampleFormat::I32 => build::<i32>(device, &stream_config, channels, buffer),
        SampleFormat::U8 => build::<u8>(device, &stream_config, channels, buffer),
        SampleFormat::U16 => build::<u16>(device, &stream_config, channels, buffer),
        SampleFormat::F32 => build::<f32>(device, &stream_config, channels, buffer),
        SampleFormat::F64 => build::<f64>(device, &stream_config, channels, buffer),
        other => return Err(format!("{name}: unsupported sample format {other:?}")),
    }
    .map_err(|e| format!("Couldn't record from {name}: {e}"))?;
    stream
        .play()
        .map_err(|e| format!("Couldn't start recording from {name}: {e}"))?;
    Ok(Opened {
        keep: Box::new(stream),
        sample_rate,
    })
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    buffer: Buffer,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    device.build_input_stream(
        *config,
        move |data: &[T], _: &_| {
            // Downmix to mono by averaging each frame's channels.
            buffer
                .lock_unpoisoned()
                .extend(data.chunks(channels).map(|frame| {
                    frame.iter().map(|s| f32::from_sample(*s)).sum::<f32>() / frame.len() as f32
                }));
        },
        |err| eprintln!("[meetings] stream error: {err}"),
        None,
    )
}

/// A source recording on its own thread until dropped.
pub struct Track {
    buffer: Buffer,
    pub sample_rate: u32,
    stop_tx: Option<Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Track {
    /// Opens a source with `open` on a new thread, and waits until it records.
    pub fn start(
        name: &str,
        open: impl FnOnce(&Buffer) -> Result<Opened, String> + Send + 'static,
    ) -> Result<Track, String> {
        let buffer = Buffer::default();
        let thread_buffer = buffer.clone();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let thread = thread::Builder::new()
            .name(format!("meeting-{name}"))
            .spawn(move || match open(&thread_buffer) {
                Ok(opened) => {
                    let _ = ready_tx.send(Ok(opened.sample_rate));
                    // Recording until told to stop, or the Track is dropped.
                    let _ = stop_rx.recv();
                    drop(opened.keep);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            })
            .map_err(|e| e.to_string())?;
        let sample_rate = match ready_rx.recv() {
            Ok(Ok(rate)) if rate > 0 => rate,
            Ok(Ok(_)) => return Err("The audio device reported no sample rate".into()),
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err("The audio thread exited before recording started".into()),
        };
        Ok(Track {
            buffer,
            sample_rate,
            stop_tx: Some(stop_tx),
            thread: Some(thread),
        })
    }

    /// Everything recorded since the last call.
    pub fn take(&self) -> Vec<f32> {
        std::mem::take(&mut *self.buffer.lock_unpoisoned())
    }

    /// Stops recording and returns what arrived since the last `take`.
    pub fn stop(mut self) -> Vec<f32> {
        self.join();
        self.take()
    }

    fn join(&mut self) {
        drop(self.stop_tx.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Track {
    fn drop(&mut self) {
        self.join();
    }
}

/// Resamples a stream to 16 kHz in whole quarter seconds, holding back the
/// rest, so block boundaries line up exactly for common rates like 44.1 kHz.
pub struct Resampler {
    rate: u32,
    quantum: usize,
    carry: Vec<f32>,
}

impl Resampler {
    pub fn new(rate: u32) -> Self {
        let quantum = if rate.is_multiple_of(4) {
            rate / 4
        } else {
            rate
        };
        Self {
            rate,
            quantum: quantum.max(1) as usize,
            carry: Vec::new(),
        }
    }

    pub fn push(&mut self, samples: &[f32]) -> Vec<f32> {
        self.carry.extend_from_slice(samples);
        let whole = self.carry.len() / self.quantum * self.quantum;
        if whole == 0 {
            return Vec::new();
        }
        let block: Vec<f32> = self.carry.drain(..whole).collect();
        audio::resample(&block, self.rate, WHISPER_SAMPLE_RATE)
    }

    /// Whatever is held back, once the source has stopped.
    pub fn finish(&mut self) -> Vec<f32> {
        let rest = std::mem::take(&mut self.carry);
        audio::resample(&rest, self.rate, WHISPER_SAMPLE_RATE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampling_in_pieces_keeps_the_running_length_exact() {
        for rate in [16_000, 44_100, 48_000, 22_050] {
            let mut resampler = Resampler::new(rate);
            let mut out = 0;
            // Ten seconds in uneven callback-sized pieces.
            let total = rate as usize * 10;
            let mut sent = 0;
            for size in [441, 1024, 480, 3000, 17].iter().cycle() {
                let n = (*size).min(total - sent);
                out += resampler.push(&vec![0.1; n]).len();
                sent += n;
                if sent == total {
                    break;
                }
            }
            out += resampler.finish().len();
            assert_eq!(out, 160_000, "rate {rate}");
        }
    }

    #[test]
    fn a_track_hands_over_what_its_source_records() {
        let track = Track::start("test", |buffer| {
            buffer.lock_unpoisoned().extend([0.5, 0.25]);
            Ok(Opened {
                keep: Box::new(()),
                sample_rate: 8_000,
            })
        })
        .unwrap();
        assert_eq!(track.sample_rate, 8_000);
        assert_eq!(track.take(), [0.5, 0.25]);
        assert!(track.take().is_empty());
        assert!(track.stop().is_empty());

        let failed = Track::start("test", |_| Err("No microphone found".into()));
        assert_eq!(failed.err().as_deref(), Some("No microphone found"));
    }
}
