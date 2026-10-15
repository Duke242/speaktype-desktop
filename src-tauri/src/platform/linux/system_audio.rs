//! System audio for meetings, from a PulseAudio or PipeWire monitor: the
//! sound an output plays, offered as a source. cpal talks to ALSA, which
//! doesn't list monitors, so `parec` (pulseaudio-utils, also served by
//! pipewire-pulse) records it and streams raw samples to us.

use std::{
    io::{ErrorKind, Read},
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

use crate::{
    LockExt,
    meeting::capture::{self, AudioSource, Buffer, Opened, SourceKind},
};

pub const SYSTEM_AUDIO_NOTE: Option<&str> = Some(
    "System audio is recorded from your output's monitor through PulseAudio or PipeWire, \
     which needs parec (in pulseaudio-utils).",
);

const MONITOR_PREFIX: &str = "pulse:";
const RATE: u32 = 16_000;

/// The default output, each output's monitor, then input devices.
pub fn system_audio_sources() -> Vec<AudioSource> {
    let mut sources = vec![capture::default_output_source()];
    if let Ok(output) = Command::new("pactl")
        .args(["list", "sources"])
        // Field names are translated otherwise.
        .env("LC_ALL", "C")
        .stderr(Stdio::null())
        .output()
    {
        sources.extend(parse_monitors(&String::from_utf8_lossy(&output.stdout)));
    }
    sources.extend(capture::list_inputs());
    sources
}

/// Monitor sources from `pactl list sources`.
fn parse_monitors(listing: &str) -> Vec<AudioSource> {
    let mut sources = Vec::new();
    let mut name: Option<&str> = None;
    for line in listing.lines() {
        let line = line.trim();
        if line.starts_with("Source #") {
            name = None;
        } else if let Some(value) = line.strip_prefix("Name: ") {
            name = Some(value.trim());
        } else if let Some(description) = line.strip_prefix("Description: ")
            && let Some(name) = name.filter(|n| n.ends_with(".monitor"))
        {
            sources.push(AudioSource {
                id: format!("{MONITOR_PREFIX}{name}"),
                name: description.trim().to_string(),
                kind: SourceKind::Output,
            });
        }
    }
    sources
}

pub fn open_system_audio(source: &str, buffer: &Buffer) -> Result<Opened, String> {
    if let Some(opened) = capture::open_listed(source, buffer) {
        return opened;
    }
    let device = source
        .strip_prefix(MONITOR_PREFIX)
        .unwrap_or("@DEFAULT_MONITOR@");
    let mut child = Command::new("parec")
        .arg(format!("--device={device}"))
        .args([
            "--format=float32le",
            &format!("--rate={RATE}"),
            "--channels=1",
            "--raw",
            "--latency-msec=100",
            "--client-name=WordFlow",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                "Recording system audio needs parec. Install pulseaudio-utils, or choose an input \
                 device as the source"
                    .to_string()
            } else {
                format!("Couldn't start parec: {e}")
            }
        })?;
    // A missing monitor or no sound server makes parec quit straight away.
    thread::sleep(Duration::from_millis(300));
    if let Ok(Some(status)) = child.try_wait() {
        return Err(format!(
            "Couldn't record system audio: parec stopped ({status}). Is PulseAudio or PipeWire running?"
        ));
    }
    let Some(mut stdout) = child.stdout.take() else {
        let _ = child.kill();
        return Err("Couldn't read from parec".into());
    };
    let buffer = buffer.clone();
    let reader = thread::Builder::new()
        .name("meeting-parec".into())
        .spawn(move || {
            let mut carry = Vec::new();
            let mut chunk = [0u8; 8192];
            loop {
                match stdout.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let samples = decode_f32le(&mut carry, &chunk[..n]);
                        buffer.lock_unpoisoned().extend(samples);
                    }
                }
            }
        });
    if let Err(e) = reader {
        let _ = child.kill();
        let _ = child.wait();
        return Err(e.to_string());
    }
    Ok(Opened {
        keep: Box::new(Parec(child)),
        sample_rate: RATE,
    })
}

/// Stops parec when recording ends; its reader thread then sees the pipe close.
struct Parec(Child);

impl Drop for Parec {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Little-endian f32 samples from a byte stream, keeping a partial sample
/// in `carry` for the next read.
fn decode_f32le(carry: &mut Vec<u8>, bytes: &[u8]) -> Vec<f32> {
    carry.extend_from_slice(bytes);
    let whole = carry.len() / 4 * 4;
    let samples = carry[..whole]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect();
    carry.drain(..whole);
    samples
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_split_across_reads_are_joined() {
        let bytes: Vec<u8> = [0.5f32, -0.25, 1.0]
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .collect();
        let mut carry = Vec::new();
        let mut out = decode_f32le(&mut carry, &bytes[..5]);
        assert_eq!(out, [0.5]);
        assert_eq!(carry.len(), 1);
        out.extend(decode_f32le(&mut carry, &bytes[5..]));
        assert_eq!(out, [0.5, -0.25, 1.0]);
        assert!(carry.is_empty());
    }

    #[test]
    fn only_monitors_are_listed_by_their_description() {
        let listing = "Source #48\n\tState: SUSPENDED\n\
            \tName: alsa_output.pci-0000_00_1f.3.analog-stereo.monitor\n\
            \tDescription: Monitor of Built-in Audio Analog Stereo\n\tDriver: PipeWire\n\
            Source #49\n\tName: alsa_input.pci-0000_00_1f.3.analog-stereo\n\
            \tDescription: Built-in Audio Analog Stereo\n";
        let sources = parse_monitors(listing);
        assert_eq!(sources.len(), 1);
        assert_eq!(
            sources[0].id,
            "pulse:alsa_output.pci-0000_00_1f.3.analog-stereo.monitor"
        );
        assert_eq!(sources[0].name, "Monitor of Built-in Audio Analog Stereo");
        assert!(parse_monitors("").is_empty());
    }
}
