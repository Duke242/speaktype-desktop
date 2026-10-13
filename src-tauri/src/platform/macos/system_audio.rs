//! System audio for meetings. On macOS 14.2 and later, cpal records what an
//! output plays through a Core Audio process tap, which asks for permission to
//! record system audio the first time. Earlier versions can record from a
//! virtual device such as BlackHole that the output is routed through, chosen
//! as an input.

use crate::meeting::capture::{self, AudioSource, Buffer, Opened};

pub const SYSTEM_AUDIO_NOTE: Option<&str> = Some(
    "Recording system audio needs macOS 14.2 or later, and asks for permission the first \
     time. On earlier versions, route your output through a virtual device such as BlackHole \
     and choose it here.",
);

/// The default output and each output where taps are supported, then input devices.
pub fn system_audio_sources() -> Vec<AudioSource> {
    let mut sources = Vec::new();
    if supports_taps() {
        sources.push(capture::default_output_source());
        sources.extend(capture::list_outputs());
    }
    sources.extend(capture::list_inputs());
    sources
}

pub fn open_system_audio(source: &str, buffer: &Buffer) -> Result<Opened, String> {
    // Input devices, like BlackHole, work on any version.
    if source.starts_with("input:")
        && let Some(opened) = capture::open_listed(source, buffer)
    {
        return opened;
    }
    if !supports_taps() {
        return Err(
            "Recording system audio needs macOS 14.2 or later. Choose a virtual device \
                    such as BlackHole as the source instead"
                .into(),
        );
    }
    capture::open_listed(source, buffer).unwrap_or_else(|| capture::open_output(None, buffer))
}

/// Process taps arrived in macOS 14.2. Checked first, since cpal's tap code
/// expects classes older versions don't have.
fn supports_taps() -> bool {
    sysinfo::System::os_version()
        .as_deref()
        .and_then(parse_version)
        .is_some_and(|version| version >= (14, 2))
}

fn parse_version(version: &str) -> Option<(u32, u32)> {
    let mut parts = version.trim().split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = match parts.next() {
        Some(part) => part.parse().ok()?,
        None => 0,
    };
    Some((major, minor))
}

#[cfg(test)]
mod tests {
    use super::parse_version;

    #[test]
    fn reads_macos_versions() {
        assert_eq!(parse_version("14.2.1"), Some((14, 2)));
        assert_eq!(parse_version("15"), Some((15, 0)));
        assert_eq!(parse_version("13.6"), Some((13, 6)));
        assert_eq!(parse_version(""), None);
        assert!(parse_version("14.1").unwrap() < (14, 2));
    }
}
