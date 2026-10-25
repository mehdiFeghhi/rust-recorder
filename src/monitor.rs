//! Discovery of connected X11 monitors through `xrandr --query`.

use std::error::Error;
use std::fmt::{self, Write as _};
use std::io;
use std::process::{Command, ExitStatus};

/// Executable name, resolved through `PATH`.
pub const XRANDR_PROGRAM: &str = "xrandr";

/// One connected output and its position in the X11 root window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monitor {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub primary: bool,
}

/// Why monitor discovery failed.
#[derive(Debug)]
pub enum MonitorError {
    /// The executable could not be spawned (missing binary, permission denied).
    Spawn { source: io::Error },
    /// `xrandr` ran but reported failure.
    Failed { status: ExitStatus, stderr: String },
}

impl MonitorError {
    /// Exit code for runtime failures (see README).
    pub const EXIT_CODE: u8 = 1;
}

impl fmt::Display for MonitorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { source } => write!(f, "could not start xrandr: {source}"),
            Self::Failed { status, stderr } => {
                let code = match status.code() {
                    Some(code) => format!("status {code}"),
                    None => "signal".to_owned(),
                };
                if stderr.is_empty() {
                    write!(f, "xrandr failed ({code})")
                } else {
                    write!(f, "xrandr failed ({code}): {stderr}")
                }
            }
        }
    }
}

impl Error for MonitorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Spawn { source } => Some(source),
            Self::Failed { .. } => None,
        }
    }
}

/// Discover connected monitors with the system `xrandr`.
pub fn discover() -> Result<Vec<Monitor>, MonitorError> {
    discover_with(XRANDR_PROGRAM)
}

/// Discover with an explicit program, so tests do not need a real X server.
pub(crate) fn discover_with(program: &str) -> Result<Vec<Monitor>, MonitorError> {
    let output = Command::new(program)
        .arg("--query")
        .output()
        .map_err(|source| MonitorError::Spawn { source })?;

    if !output.status.success() {
        return Err(MonitorError::Failed {
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    Ok(parse(&String::from_utf8_lossy(&output.stdout)))
}

/// Parse `xrandr --query` output, keeping connected outputs with a geometry.
fn parse(output: &str) -> Vec<Monitor> {
    let mut monitors = Vec::new();

    for line in output.lines() {
        // Mode lines inside a monitor block are indented.
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }

        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() < 3 || !tokens.contains(&"connected") {
            continue;
        }

        let geometry = tokens
            .iter()
            .skip(2)
            .find_map(|token| parse_geometry(token));
        let Some((width, height, x, y)) = geometry else {
            continue;
        };

        monitors.push(Monitor {
            name: tokens[0].to_owned(),
            width,
            height,
            x,
            y,
            primary: tokens.contains(&"primary"),
        });
    }

    monitors
}

/// Parse `WIDTHxHEIGHT+X+Y` as emitted in a monitor header.
fn parse_geometry(token: &str) -> Option<(u32, u32, i32, i32)> {
    let (size, offset) = token.split_once('+')?;
    let (width, height) = size.split_once('x')?;
    let (x, y) = offset.split_once('+')?;
    Some((
        width.parse().ok()?,
        height.parse().ok()?,
        x.parse().ok()?,
        y.parse().ok()?,
    ))
}

/// Render the human-readable monitor table printed by `--list-monitors`.
pub fn render(monitors: &[Monitor]) -> String {
    let mut output = String::from("Available monitors:\n");

    for (index, monitor) in monitors.iter().enumerate() {
        let primary = if monitor.primary { "yes" } else { "no" };
        let _ = writeln!(
            output,
            "\n[{index}] {name}\n    Position: {x},{y}\n    Resolution: {width}x{height}\n    Primary: {primary}",
            name = monitor.name,
            x = monitor.x,
            y = monitor.y,
            width = monitor.width,
            height = monitor.height,
        );
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\nScreen 0: minimum 320 x 200, current 4480 x 1440, maximum 16384 x 16384\neDP-1 connected primary 1920x1200+0+0 (normal left inverted right x axis y axis) 344mm x 215mm\n   1920x1200     60.00*+\nHDMI-1 connected 2560x1440+1920+0 (normal left inverted right x axis y axis) 600mm x 340mm\n   2560x1440     60.00*+\nDP-1 disconnected (normal left inverted right x axis y axis)\n";

    #[test]
    fn parses_connected_monitors() {
        let monitors = parse(SAMPLE);

        assert_eq!(monitors.len(), 2);
        assert_eq!(monitors[0].name, "eDP-1");
        assert_eq!((monitors[0].width, monitors[0].height), (1920, 1200));
        assert_eq!((monitors[0].x, monitors[0].y), (0, 0));
        assert!(monitors[0].primary);

        assert_eq!(monitors[1].name, "HDMI-1");
        assert_eq!((monitors[1].width, monitors[1].height), (2560, 1440));
        assert_eq!((monitors[1].x, monitors[1].y), (1920, 0));
        assert!(!monitors[1].primary);
    }

    #[test]
    fn ignores_disconnected_and_malformed_lines() {
        let output =
            "\nDP-1 disconnected (normal)\nHDMI-1 connected (no geometry)\nnot a monitor line\n";
        assert!(parse(output).is_empty());
    }

    #[test]
    fn renders_the_table() {
        let monitors = parse(SAMPLE);
        let table = render(&monitors);

        assert!(table.starts_with("Available monitors:"));
        assert!(table.contains("[0] eDP-1"));
        assert!(table.contains("Position: 0,0"));
        assert!(table.contains("Resolution: 1920x1200"));
        assert!(table.contains("Primary: yes"));
        assert!(table.contains("[1] HDMI-1"));
        assert!(table.contains("Position: 1920,0"));
        assert!(table.contains("Primary: no"));
    }

    #[test]
    fn spawn_failure_is_reported() {
        let error = discover_with("rust-recorder-xrandr-that-does-not-exist")
            .expect_err("missing program must fail");
        match error {
            MonitorError::Spawn { source } => {
                assert_eq!(source.kind(), io::ErrorKind::NotFound);
            }
            other => panic!("expected Spawn, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn nonzero_exit_is_reported() {
        let error = discover_with("false").expect_err("false must fail");
        assert!(matches!(error, MonitorError::Failed { .. }));
    }

    #[test]
    fn runtime_exit_code_is_one() {
        assert_eq!(MonitorError::EXIT_CODE, 1);
    }
}
