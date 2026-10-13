//! System audio for meetings, through WASAPI loopback: cpal records what an
//! output plays when asked for an input stream on it. While nothing plays,
//! Windows sends nothing, and the meeting's aligner fills in the silence.

use crate::meeting::capture::{self, AudioSource, Buffer, Opened};

pub const SYSTEM_AUDIO_NOTE: Option<&str> = None;

/// The default output, each output, then input devices.
pub fn system_audio_sources() -> Vec<AudioSource> {
    let mut sources = vec![capture::default_output_source()];
    sources.extend(capture::list_outputs());
    sources.extend(capture::list_inputs());
    sources
}

pub fn open_system_audio(source: &str, buffer: &Buffer) -> Result<Opened, String> {
    capture::open_listed(source, buffer).unwrap_or_else(|| capture::open_output(None, buffer))
}
