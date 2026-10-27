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

/// Whether and how audio is recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioMode {
    /// No audio (default); preserves the video-only behaviour.
    #[default]
    None,
    /// One microphone source.
    Microphone,
    /// One output-monitor source (system audio).
    System,
    /// Microphone and system audio, mixed.
    Both,
}

impl AudioMode {
    /// Parse the CLI spelling.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "mic" => Some(Self::Microphone),
            "system" => Some(Self::System),
            "both" => Some(Self::Both),
            _ => None,
        }
    }

    /// The CLI spelling, for error messages.
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Microphone => "mic",
            Self::System => "system",
            Self::Both => "both",
        }
    }
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
    /// The requested microphone id does not exist.
    UnknownDevice { requested: String },
    /// The requested id exists but is an output monitor, not a microphone.
    NotAMicrophone { requested: String },
    /// No microphone sources are available.
    NoMicrophone,
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
            Self::UnknownDevice { requested } => write!(
                f,
                "microphone {requested:?} was not found; run --list-audio-devices"
            ),
            Self::NotAMicrophone { requested } => {
                write!(f, "{requested:?} is an output monitor, not a microphone")
            }
            Self::NoMicrophone => {
                write!(f, "no microphone sources found; run --list-audio-devices")
            }
        }
    }
}

impl Error for AudioError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Spawn { source } => Some(source),
            Self::Failed { .. }
            | Self::UnknownDevice { .. }
            | Self::NotAMicrophone { .. }
            | Self::NoMicrophone => None,
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

/// Choose the microphone to record: the requested id, or the first one.
pub fn select_microphone(
    requested: Option<&str>,
    sources: &[AudioSource],
) -> Result<AudioSource, AudioError> {
    if let Some(id) = requested {
        let source = sources
            .iter()
            .find(|source| source.id == id)
            .ok_or_else(|| AudioError::UnknownDevice {
                requested: id.to_owned(),
            })?;

        if source.kind != AudioSourceKind::Microphone {
            return Err(AudioError::NotAMicrophone {
                requested: id.to_owned(),
            });
        }

        return Ok(source.clone());
    }

    sources
        .iter()
        .find(|source| source.kind == AudioSourceKind::Microphone)
        .cloned()
        .ok_or(AudioError::NoMicrophone)
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
    fn parses_audio_modes() {
        assert_eq!(AudioMode::parse("none"), Some(AudioMode::None));
        assert_eq!(AudioMode::parse("mic"), Some(AudioMode::Microphone));
        assert_eq!(AudioMode::parse("system"), Some(AudioMode::System));
        assert_eq!(AudioMode::parse("both"), Some(AudioMode::Both));
        assert_eq!(AudioMode::parse("stereo"), None);
        assert_eq!(AudioMode::default(), AudioMode::None);
    }

    #[test]
    fn selects_the_requested_microphone() {
        let sources = parse(SAMPLE);
        let selected =
            select_microphone(Some("alsa_input.pci-0000_00_1f.3.analog-stereo"), &sources)
                .expect("known microphone");

        assert_eq!(selected.kind, AudioSourceKind::Microphone);
    }

    #[test]
    fn defaults_to_the_first_microphone() {
        let sources = parse(SAMPLE);
        let selected = select_microphone(None, &sources).expect("default microphone");

        assert_eq!(selected.id, "alsa_input.pci-0000_00_1f.3.analog-stereo");
    }

    #[test]
    fn rejects_unknown_or_monitor_devices() {
        let sources = parse(SAMPLE);

        assert!(matches!(
            select_microphone(Some("missing"), &sources),
            Err(AudioError::UnknownDevice { .. })
        ));

        let monitor = "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor";
        assert!(matches!(
            select_microphone(Some(monitor), &sources),
            Err(AudioError::NotAMicrophone { .. })
        ));
    }

    #[test]
    fn reports_when_no_microphone_exists() {
        let monitor_only = "Source #1\n\tName: sink.monitor\n";
        assert!(matches!(
            select_microphone(None, &parse(monitor_only)),
            Err(AudioError::NoMicrophone)
        ));
    }

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
