//! Discovery of PulseAudio-compatible audio capture sources.
//!
//! Sources come from `pactl list sources`. A source whose name ends in
//! `.monitor` is the digital stream of an output device (system audio);
//! everything else is a microphone.

use std::error::Error;
use std::fmt::{self, Write as _};
use std::io;
use std::process::{Command, ExitStatus};

/// Executable name, resolved through `PATH`.
pub const PACTL_PROGRAM: &str = "pactl";

/// What kind of audio a source carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSourceKind {
    /// Sound entering the computer.
    Microphone,
    /// Sound the computer is playing, exposed as an output monitor.
    SystemOutput,
}

/// One capture source with stable metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioSource {
    /// PulseAudio source name; used as the stable identifier.
    pub id: String,
    /// Human-readable description, falling back to the id.
    pub description: String,
    pub kind: AudioSourceKind,
}

/// Why audio source discovery failed.
#[derive(Debug)]
pub enum AudioError {
    /// The executable could not be spawned (missing binary, permission denied).
    Spawn { source: io::Error },
    /// `pactl` ran but reported failure.
    Failed { status: ExitStatus, stderr: String },
}

impl AudioError {
    /// Exit code for runtime failures (see README).
    pub const EXIT_CODE: u8 = 1;
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { source } => write!(
                f,
                "could not start pactl ({source}); install pulseaudio-utils or pipewire-pulse"
            ),
            Self::Failed { status, stderr } => {
                let code = match status.code() {
                    Some(code) => format!("status {code}"),
                    None => "signal".to_owned(),
                };
                if stderr.is_empty() {
                    write!(f, "pactl failed ({code})")
                } else {
                    write!(f, "pactl failed ({code}): {stderr}")
                }
            }
        }
    }
}

impl Error for AudioError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Spawn { source } => Some(source),
            Self::Failed { .. } => None,
        }
    }
}

/// Discover capture sources with the system `pactl`.
pub fn discover() -> Result<Vec<AudioSource>, AudioError> {
    discover_with(PACTL_PROGRAM)
}

/// Discover with an explicit program, so tests need no sound server.
pub(crate) fn discover_with(program: &str) -> Result<Vec<AudioSource>, AudioError> {
    let output = Command::new(program)
        .args(["list", "sources"])
        .output()
        .map_err(|source| AudioError::Spawn { source })?;

    if !output.status.success() {
        return Err(AudioError::Failed {
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    Ok(parse(&String::from_utf8_lossy(&output.stdout)))
}

/// Parse the `pactl list sources` blocks for `Name` and `Description`.
fn parse(output: &str) -> Vec<AudioSource> {
    let mut sources = Vec::new();
    let mut name: Option<String> = None;
    let mut description: Option<String> = None;

    for line in output.lines() {
        let line = line.trim();

        if line.starts_with("Source #") {
            if let Some(id) = name.take() {
                sources.push(build(id, description.take()));
            }
            description = None;
            continue;
        }

        if let Some(value) = line.strip_prefix("Name: ") {
            name = Some(value.to_owned());
        } else if let Some(value) = line.strip_prefix("Description: ") {
            description = Some(value.to_owned());
        }
    }

    if let Some(id) = name.take() {
        sources.push(build(id, description.take()));
    }

    sources
}

fn build(id: String, description: Option<String>) -> AudioSource {
    let kind = if id.ends_with(".monitor") {
        AudioSourceKind::SystemOutput
    } else {
        AudioSourceKind::Microphone
    };
    let description = description.unwrap_or_else(|| id.clone());

    AudioSource {
        id,
        description,
        kind,
    }
}

/// Render the table printed by `--list-audio-devices`.
pub fn render(sources: &[AudioSource]) -> String {
    let mut output = String::from("Audio sources:\n");

    for (index, source) in sources.iter().enumerate() {
        let kind = match source.kind {
            AudioSourceKind::Microphone => "microphone",
            AudioSourceKind::SystemOutput => "system audio",
        };
        let _ = writeln!(
            output,
            "\n[{index}] {description}\n    Type: {kind}\n    ID: {id}",
            description = source.description,
            id = source.id,
        );
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\nSource #49\n\tState: SUSPENDED\n\tName: alsa_output.pci-0000_00_1f.3.analog-stereo.monitor\n\tDescription: Monitor of Built-in Audio Analog Stereo\n\tDriver: PipeWire\nSource #50\n\tState: SUSPENDED\n\tName: alsa_input.pci-0000_00_1f.3.analog-stereo\n\tDescription: Built-in Audio Analog Stereo\n\tDriver: PipeWire\n";

    #[test]
    fn parses_and_classifies_sources() {
        let sources = parse(SAMPLE);

        assert_eq!(sources.len(), 2);
        assert_eq!(
            sources[0].id,
            "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor"
        );
        assert_eq!(sources[0].kind, AudioSourceKind::SystemOutput);
        assert_eq!(
            sources[0].description,
            "Monitor of Built-in Audio Analog Stereo"
        );

        assert_eq!(sources[1].id, "alsa_input.pci-0000_00_1f.3.analog-stereo");
        assert_eq!(sources[1].kind, AudioSourceKind::Microphone);
        assert_eq!(sources[1].description, "Built-in Audio Analog Stereo");
    }

    #[test]
    fn falls_back_to_the_id_when_description_is_missing() {
        let output = "Source #1\n\tName: some_input\n";
        let sources = parse(output);

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].description, "some_input");
    }

    #[test]
    fn ignores_empty_and_malformed_output() {
        assert!(parse("").is_empty());
        assert!(parse("garbage\nwithout fields\n").is_empty());
    }

    #[test]
    fn renders_the_table() {
        let table = render(&parse(SAMPLE));

        assert!(table.starts_with("Audio sources:"));
        assert!(table.contains("Monitor of Built-in Audio Analog Stereo"));
        assert!(table.contains("Type: system audio"));
        assert!(table.contains("Type: microphone"));
        assert!(table.contains("[1] Built-in Audio Analog Stereo"));
    }

    #[test]
    fn spawn_failure_is_reported() {
        let error = discover_with("rust-recorder-pactl-that-does-not-exist")
            .expect_err("missing program must fail");

        match error {
            AudioError::Spawn { source } => {
                assert_eq!(source.kind(), io::ErrorKind::NotFound);
            }
            other => panic!("expected Spawn, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn nonzero_exit_is_reported() {
        let error = discover_with("false").expect_err("false must fail");
        assert!(matches!(error, AudioError::Failed { .. }));
    }

    #[test]
    fn runtime_exit_code_is_one() {
        assert_eq!(AudioError::EXIT_CODE, 1);
    }
}
