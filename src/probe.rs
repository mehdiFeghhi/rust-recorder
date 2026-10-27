//! `ffprobe` verification that the recording is actually readable.
//!
//! The filesystem check in [`crate::output`] only proves that a non-empty file
//! exists. Probing proves that the container can be opened, contains a video
//! stream, and reports a positive duration.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};

/// Executable name, resolved through `PATH`.
pub const FFPROBE_PROGRAM: &str = "ffprobe";

/// What `ffprobe` reported about the recording.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeInfo {
    pub codec: String,
    pub duration: f64,
}

/// Why the recording could not be probed.
#[derive(Debug)]
pub enum ProbeError {
    /// The executable could not be spawned (missing binary, permission denied).
    Spawn { source: io::Error },
    /// `ffprobe` ran but could not read the file.
    Failed { status: ExitStatus, stderr: String },
    /// The container has no readable video stream.
    NoVideoStream,
    /// The container has no readable audio stream.
    NoAudioStream,
    /// `ffprobe` did not report a duration.
    MissingDuration,
    /// The reported duration is not a number.
    InvalidDuration(String),
    /// The reported duration is zero or negative.
    NonPositiveDuration(f64),
}

impl ProbeError {
    /// Exit code for runtime failures (see README).
    pub const EXIT_CODE: u8 = 1;
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { source } => write!(f, "could not start ffprobe: {source}"),
            Self::Failed { status, stderr } => {
                let code = match status.code() {
                    Some(code) => format!("status {code}"),
                    None => "signal".to_owned(),
                };
                if stderr.is_empty() {
                    write!(f, "ffprobe could not read the recording ({code})")
                } else {
                    write!(f, "ffprobe could not read the recording ({code}): {stderr}")
                }
            }
            Self::NoVideoStream => write!(f, "the recording contains no video stream"),
            Self::NoAudioStream => write!(f, "the recording contains no audio stream"),
            Self::MissingDuration => write!(f, "ffprobe reported no duration"),
            Self::InvalidDuration(value) => {
                write!(f, "ffprobe reported an invalid duration: {value:?}")
            }
            Self::NonPositiveDuration(duration) => {
                write!(f, "the recording duration is not positive ({duration} s)")
            }
        }
    }
}

impl Error for ProbeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Spawn { source } => Some(source),
            Self::Failed { .. }
            | Self::NoVideoStream
            | Self::NoAudioStream
            | Self::MissingDuration
            | Self::InvalidDuration(_)
            | Self::NonPositiveDuration(_) => None,
        }
    }
}

/// Codec of the first audio stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioInfo {
    pub codec: String,
}

/// Probe the audio stream at `path` with the system `ffprobe`.
pub fn inspect_audio(path: &Path) -> Result<AudioInfo, ProbeError> {
    inspect_audio_with(FFPROBE_PROGRAM, path)
}

pub(crate) fn inspect_audio_with(program: &str, path: &Path) -> Result<AudioInfo, ProbeError> {
    let output = Command::new(program)
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=codec_name",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(path)
        .output()
        .map_err(|source| ProbeError::Spawn { source })?;

    if !output.status.success() {
        return Err(ProbeError::Failed {
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    parse_audio(&String::from_utf8_lossy(&output.stdout))
}

fn parse_audio(stdout: &str) -> Result<AudioInfo, ProbeError> {
    let codec = stdout
        .lines()
        .find_map(|line| line.strip_prefix("codec_name="))
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "unknown")
        .map(str::to_owned)
        .ok_or(ProbeError::NoAudioStream)?;

    Ok(AudioInfo { codec })
}

/// Probe the recording at `path` with the system `ffprobe`.
pub fn inspect(path: &Path) -> Result<ProbeInfo, ProbeError> {
    inspect_with(FFPROBE_PROGRAM, path)
}

/// Probe with an explicit program, so tests do not depend on ffprobe.
pub(crate) fn inspect_with(program: &str, path: &Path) -> Result<ProbeInfo, ProbeError> {
    let output = Command::new(program)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(path)
        .output()
        .map_err(|source| ProbeError::Spawn { source })?;

    if !output.status.success() {
        return Err(ProbeError::Failed {
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    parse(&String::from_utf8_lossy(&output.stdout))
}

/// Parse the `key=value` lines emitted by the `default` output format.
fn parse(stdout: &str) -> Result<ProbeInfo, ProbeError> {
    let mut codec = None;
    let mut duration = None;

    for line in stdout.lines() {
        if let Some(value) = line.strip_prefix("codec_name=") {
            let value = value.trim();
            if !value.is_empty() && value != "unknown" {
                codec = Some(value.to_owned());
            }
        } else if let Some(value) = line.strip_prefix("duration=") {
            duration = Some(value.trim());
        }
    }

    let codec = codec.ok_or(ProbeError::NoVideoStream)?;
    let raw = duration.ok_or(ProbeError::MissingDuration)?;
    let duration: f64 = raw
        .parse()
        .map_err(|_| ProbeError::InvalidDuration(raw.to_owned()))?;

    if !duration.is_finite() || duration <= 0.0 {
        return Err(ProbeError::NonPositiveDuration(duration));
    }

    Ok(ProbeInfo { codec, duration })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_audio_codec() {
        assert_eq!(
            parse_audio("codec_name=aac\n").expect("audio codec"),
            AudioInfo {
                codec: "aac".to_owned()
            }
        );
    }

    #[test]
    fn rejects_missing_audio_streams() {
        assert!(matches!(
            parse_audio("codec_name=unknown\n"),
            Err(ProbeError::NoAudioStream)
        ));
        assert!(matches!(parse_audio(""), Err(ProbeError::NoAudioStream)));
    }

    #[cfg(unix)]
    #[test]
    fn audio_probe_failures_are_reported() {
        assert!(matches!(
            inspect_audio_with("false", Path::new("/tmp/never-read.mp4")),
            Err(ProbeError::Failed { .. })
        ));
        assert!(matches!(
            inspect_audio_with("true", Path::new("/tmp/never-read.mp4")),
            Err(ProbeError::NoAudioStream)
        ));
    }

    #[test]
    fn parses_codec_and_duration() {
        let info = parse("codec_name=h264\nduration=1.500000\n").expect("valid output");
        assert_eq!(info.codec, "h264");
        assert_eq!(info.duration, 1.5);
    }

    #[test]
    fn rejects_a_missing_video_stream() {
        let error = parse("duration=1.0\n").expect_err("no codec must fail");
        assert!(matches!(error, ProbeError::NoVideoStream));
    }

    #[test]
    fn rejects_a_unknown_codec() {
        let error = parse("codec_name=unknown\nduration=1.0\n").expect_err("unknown codec");
        assert!(matches!(error, ProbeError::NoVideoStream));
    }

    #[test]
    fn rejects_a_missing_duration() {
        let error = parse("codec_name=h264\n").expect_err("no duration must fail");
        assert!(matches!(error, ProbeError::MissingDuration));
    }

    #[test]
    fn rejects_an_invalid_duration() {
        let error = parse("codec_name=h264\nduration=soon\n").expect_err("bad duration");
        assert!(matches!(error, ProbeError::InvalidDuration(value) if value == "soon"));
    }

    #[test]
    fn rejects_zero_and_negative_durations() {
        for raw in ["0.0", "-1.5"] {
            let output = format!("codec_name=h264\nduration={raw}\n");
            let error = parse(&output).expect_err("non-positive duration must fail");
            assert!(matches!(error, ProbeError::NonPositiveDuration(_)));
        }
    }

    #[test]
    fn spawn_failure_is_reported() {
        let error = inspect_with(
            "rust-recorder-ffprobe-that-does-not-exist",
            Path::new("/tmp/never-read.mp4"),
        )
        .expect_err("missing program must fail");

        match error {
            ProbeError::Spawn { source } => {
                assert_eq!(source.kind(), io::ErrorKind::NotFound);
            }
            other => panic!("expected Spawn, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn nonzero_exit_is_reported() {
        let error =
            inspect_with("false", Path::new("/tmp/never-read.mp4")).expect_err("false must fail");
        assert!(matches!(error, ProbeError::Failed { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn successful_but_empty_output_is_rejected() {
        let error = inspect_with("true", Path::new("/tmp/never-read.mp4"))
            .expect_err("empty output must fail");
        assert!(matches!(error, ProbeError::NoVideoStream));
    }

    #[test]
    fn runtime_exit_code_is_one() {
        assert_eq!(ProbeError::EXIT_CODE, 1);
    }
}
