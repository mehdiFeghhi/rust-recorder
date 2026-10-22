//! Command-line configuration for `rust-recorder`.
//!
//! Parsing (syntax) and validation (semantics) are deliberately separate:
//! `Config::parse` turns arguments into a `Config` and never spawns anything;
//! `Config::validate` checks that configuration against the filesystem and the
//! supported limits. Both run before FFmpeg is involved.

use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::path::PathBuf;

/// Frame rate used when `--fps` is omitted.
pub const DEFAULT_FPS: u32 = 30;

/// Upper frame-rate bound. Higher values only burn CPU for screen capture.
pub const MAX_FPS: u32 = 144;

/// One day. Longer recordings should be split into several files.
pub const MAX_DURATION_SECS: u64 = 86_400;

/// Validated recording configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub output: PathBuf,
    pub fps: u32,
    pub duration: Option<u64>,
    pub display: String,
}

/// Outcome of parsing the command line.
#[derive(Debug, PartialEq, Eq)]
pub enum ParseResult {
    Help,
    Run(Config),
}

/// A user-facing configuration problem. Every variant maps to exit code 2.
#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    MissingOutput,
    MissingValue(&'static str),
    UnknownOption(String),
    InvalidFps(String),
    InvalidDuration(String),
    InvalidDisplay(String),
    DisplayUnset,
    EmptyOutput,
    MissingExtension,
    UnsupportedExtension(String),
    ParentMissing(PathBuf),
    ParentNotDirectory(PathBuf),
    OutputExists(PathBuf),
}

impl ConfigError {
    /// Exit code for usage and configuration errors (see README).
    pub const EXIT_CODE: u8 = 2;
}

impl Config {
    /// Parse arguments, using the process `DISPLAY` variable as the default.
    pub fn parse<I>(args: I) -> Result<ParseResult, ConfigError>
    where
        I: IntoIterator<Item = OsString>,
    {
        Self::parse_with_display(args, std::env::var("DISPLAY").ok())
    }

    /// Parse arguments with an explicit display default.
    ///
    /// `default_display` is a parameter rather than a direct `env::var` call so
    /// tests can exercise the "DISPLAY unset" path without mutating the
    /// process environment (which is global and racy).
    pub(crate) fn parse_with_display<I>(
        args: I,
        default_display: Option<String>,
    ) -> Result<ParseResult, ConfigError>
    where
        I: IntoIterator<Item = OsString>,
    {
        let mut output: Option<PathBuf> = None;
        let mut fps = DEFAULT_FPS;
        let mut duration: Option<u64> = None;
        let mut display = default_display;
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            let flag = arg
                .to_str()
                .ok_or_else(|| ConfigError::UnknownOption(arg.to_string_lossy().into_owned()))?;

            match flag {
                "-h" | "--help" => return Ok(ParseResult::Help),
                "-o" | "--output" => {
                    output = Some(PathBuf::from(next_value(&mut args, "--output")?));
                }
                "--fps" => {
                    let raw = next_value(&mut args, "--fps")?;
                    let text = raw.to_str().ok_or_else(|| {
                        ConfigError::InvalidFps(raw.to_string_lossy().into_owned())
                    })?;
                    fps = text
                        .parse()
                        .map_err(|_| ConfigError::InvalidFps(text.to_owned()))?;
                }
                "--duration" => {
                    let raw = next_value(&mut args, "--duration")?;
                    let text = raw.to_str().ok_or_else(|| {
                        ConfigError::InvalidDuration(raw.to_string_lossy().into_owned())
                    })?;
                    duration = Some(
                        text.parse()
                            .map_err(|_| ConfigError::InvalidDuration(text.to_owned()))?,
                    );
                }
                "--display" => {
                    let raw = next_value(&mut args, "--display")?;
                    let text = raw.to_str().ok_or_else(|| {
                        ConfigError::InvalidDisplay(raw.to_string_lossy().into_owned())
                    })?;
                    display = Some(text.to_owned());
                }
                other => return Err(ConfigError::UnknownOption(other.to_owned())),
            }
        }

        let output = output.ok_or(ConfigError::MissingOutput)?;
        let display = display.ok_or(ConfigError::DisplayUnset)?;

        let config = Config {
            output,
            fps,
            duration,
            display,
        };
        config.validate()?;
        Ok(ParseResult::Run(config))
    }

    /// Check the configuration against limits and the filesystem.
    ///
    /// This is intentionally separate from parsing so each rule can be tested
    /// (and later reused) without building an argument list.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if !(1..=MAX_FPS).contains(&self.fps) {
            return Err(ConfigError::InvalidFps(self.fps.to_string()));
        }

        match self.duration {
            Some(duration) if !(1..=MAX_DURATION_SECS).contains(&duration) => {
                return Err(ConfigError::InvalidDuration(duration.to_string()));
            }
            _ => {}
        }

        validate_display(&self.display)?;

        if self.output.as_os_str().is_empty() {
            return Err(ConfigError::EmptyOutput);
        }

        match self.output.extension().and_then(|ext| ext.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("mp4") || ext.eq_ignore_ascii_case("mkv") => {}
            Some(ext) => return Err(ConfigError::UnsupportedExtension(ext.to_owned())),
            None => return Err(ConfigError::MissingExtension),
        }

        // A relative file name such as `out.mp4` has an empty parent; that
        // means the current directory and needs no further checks.
        if let Some(parent) = self
            .output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            if !parent.exists() {
                return Err(ConfigError::ParentMissing(parent.to_path_buf()));
            }
            if !parent.is_dir() {
                return Err(ConfigError::ParentNotDirectory(parent.to_path_buf()));
            }
        }

        // `symlink_metadata` also catches broken symlinks, which `exists()`
        // would report as absent. We must never silently replace an entry.
        if fs::symlink_metadata(&self.output).is_ok() {
            return Err(ConfigError::OutputExists(self.output.clone()));
        }

        Ok(())
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "output={} fps={} display={}",
            self.output.display(),
            self.fps,
            self.display
        )?;
        if let Some(duration) = self.duration {
            write!(f, " duration={duration}s")?;
        }
        Ok(())
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingOutput => write!(f, "missing required option --output"),
            Self::MissingValue(option) => write!(f, "option {option} requires a value"),
            Self::UnknownOption(option) => write!(f, "unknown option {option:?}"),
            Self::InvalidFps(value) => write!(
                f,
                "invalid --fps {value:?}: expected an integer between 1 and {MAX_FPS}"
            ),
            Self::InvalidDuration(value) => write!(
                f,
                "invalid --duration {value:?}: expected an integer between 1 and {MAX_DURATION_SECS}"
            ),
            Self::InvalidDisplay(value) => write!(
                f,
                "invalid --display {value:?}: expected a local display such as \":0\""
            ),
            Self::DisplayUnset => {
                write!(f, "no --display given and DISPLAY is not set")
            }
            Self::EmptyOutput => write!(f, "--output must not be empty"),
            Self::MissingExtension => write!(f, "--output must end in .mp4 or .mkv"),
            Self::UnsupportedExtension(ext) => {
                write!(f, "unsupported output extension {ext:?}: use .mp4 or .mkv")
            }
            Self::ParentMissing(path) => {
                write!(f, "output directory does not exist: {}", path.display())
            }
            Self::ParentNotDirectory(path) => {
                write!(f, "output parent is not a directory: {}", path.display())
            }
            Self::OutputExists(path) => {
                write!(f, "refusing to overwrite existing file: {}", path.display())
            }
        }
    }
}

impl Error for ConfigError {}

fn next_value<I>(args: &mut I, option: &'static str) -> Result<OsString, ConfigError>
where
    I: Iterator<Item = OsString>,
{
    args.next().ok_or(ConfigError::MissingValue(option))
}

fn validate_display(display: &str) -> Result<(), ConfigError> {
    let Some(number) = display.strip_prefix(':') else {
        return Err(ConfigError::InvalidDisplay(display.to_owned()));
    };
    if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ConfigError::InvalidDisplay(display.to_owned()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "rust-recorder-m1-{}-{label}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create temporary test directory");
        dir
    }

    fn parse(list: &[&str]) -> Result<ParseResult, ConfigError> {
        Config::parse_with_display(
            list.iter().copied().map(OsString::from),
            Some(":1".to_owned()),
        )
    }

    fn config(list: &[&str]) -> Config {
        match parse(list).expect("expected a valid configuration") {
            ParseResult::Run(config) => config,
            ParseResult::Help => panic!("expected a configuration, got help"),
        }
    }

    #[test]
    fn help_wins_even_with_other_arguments() {
        assert_eq!(
            parse(&["--help", "--output", "out.mp4"]).unwrap(),
            ParseResult::Help
        );
        assert_eq!(parse(&["-h"]).unwrap(), ParseResult::Help);
    }

    #[test]
    fn defaults_are_applied() {
        let dir = temp_dir("defaults");
        let output = dir.join("out.mp4");
        let config = config(&["--output", output.to_str().unwrap()]);

        assert_eq!(config.fps, DEFAULT_FPS);
        assert_eq!(config.duration, None);
        assert_eq!(config.display, ":1");
        assert_eq!(config.output, output);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn explicit_options_are_applied() {
        let dir = temp_dir("explicit");
        let output = dir.join("clip.mkv");
        let config = config(&[
            "--output",
            output.to_str().unwrap(),
            "--fps",
            "60",
            "--duration",
            "12",
            "--display",
            ":0",
        ]);

        assert_eq!(config.output, output);
        assert_eq!(config.fps, 60);
        assert_eq!(config.duration, Some(12));
        assert_eq!(config.display, ":0");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn short_output_flag_is_accepted() {
        let dir = temp_dir("short");
        let output = dir.join("out.mp4");
        let config = config(&["-o", output.to_str().unwrap()]);
        assert_eq!(config.output, output);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_output_is_rejected() {
        assert_eq!(
            parse(&["--fps", "30"]).unwrap_err(),
            ConfigError::MissingOutput
        );
    }

    #[test]
    fn unknown_option_is_rejected() {
        assert_eq!(
            parse(&["--output", "out.mp4", "--turbo"]).unwrap_err(),
            ConfigError::UnknownOption("--turbo".to_owned())
        );
    }

    #[test]
    fn missing_value_is_rejected() {
        assert_eq!(
            parse(&["--output"]).unwrap_err(),
            ConfigError::MissingValue("--output")
        );
        assert_eq!(
            parse(&["--fps"]).unwrap_err(),
            ConfigError::MissingValue("--fps")
        );
    }

    #[test]
    fn non_numeric_fps_is_rejected() {
        assert_eq!(
            parse(&["--output", "out.mp4", "--fps", "fast"]).unwrap_err(),
            ConfigError::InvalidFps("fast".to_owned())
        );
    }

    #[test]
    fn fps_out_of_range_is_rejected() {
        assert_eq!(
            parse(&["--output", "out.mp4", "--fps", "0"]).unwrap_err(),
            ConfigError::InvalidFps("0".to_owned())
        );
        assert_eq!(
            parse(&["--output", "out.mp4", "--fps", "145"]).unwrap_err(),
            ConfigError::InvalidFps("145".to_owned())
        );
    }

    #[test]
    fn flag_like_value_is_reported_as_invalid() {
        // `--fps` consumes the next token even if it looks like a flag, so the
        // error names the value instead of silently ignoring it.
        assert_eq!(
            parse(&["--output", "out.mp4", "--fps", "--duration"]).unwrap_err(),
            ConfigError::InvalidFps("--duration".to_owned())
        );
    }

    #[test]
    fn duration_is_validated() {
        assert_eq!(
            parse(&["--output", "out.mp4", "--duration", "0"]).unwrap_err(),
            ConfigError::InvalidDuration("0".to_owned())
        );
        assert_eq!(
            parse(&["--output", "out.mp4", "--duration", "86401"]).unwrap_err(),
            ConfigError::InvalidDuration("86401".to_owned())
        );
        assert_eq!(
            parse(&["--output", "out.mp4", "--duration", "soon"]).unwrap_err(),
            ConfigError::InvalidDuration("soon".to_owned())
        );
    }

    #[test]
    fn unsupported_extension_is_rejected() {
        assert_eq!(
            parse(&["--output", "clip.avi"]).unwrap_err(),
            ConfigError::UnsupportedExtension("avi".to_owned())
        );
    }

    #[test]
    fn missing_extension_is_rejected() {
        assert_eq!(
            parse(&["--output", "clip"]).unwrap_err(),
            ConfigError::MissingExtension
        );
    }

    #[test]
    fn existing_output_is_rejected() {
        let dir = temp_dir("exists");
        let output = dir.join("out.mp4");
        fs::write(&output, b"keep me").expect("create existing file");

        assert_eq!(
            parse(&["--output", output.to_str().unwrap()]).unwrap_err(),
            ConfigError::OutputExists(output.clone())
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_parent_is_rejected() {
        let dir = temp_dir("no-parent");
        let output = dir.join("missing").join("out.mp4");

        assert_eq!(
            parse(&["--output", output.to_str().unwrap()]).unwrap_err(),
            ConfigError::ParentMissing(output.parent().unwrap().to_path_buf())
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn file_as_parent_is_rejected() {
        let dir = temp_dir("file-parent");
        let blocker = dir.join("blocker");
        fs::write(&blocker, b"not a directory").expect("create blocker file");
        let output = blocker.join("out.mp4");

        assert_eq!(
            parse(&["--output", output.to_str().unwrap()]).unwrap_err(),
            ConfigError::ParentNotDirectory(blocker)
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn invalid_display_is_rejected() {
        for bad in ["", "1", ":abc", ":", "host:0"] {
            assert_eq!(
                parse(&["--output", "out.mp4", "--display", bad]).unwrap_err(),
                ConfigError::InvalidDisplay(bad.to_owned()),
                "display {bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn unset_display_is_rejected() {
        let error = Config::parse_with_display(
            [OsString::from("--output"), OsString::from("out.mp4")],
            None,
        )
        .unwrap_err();

        assert_eq!(error, ConfigError::DisplayUnset);
    }

    #[test]
    fn display_default_is_used_when_flag_is_absent() {
        let dir = temp_dir("display-default");
        let output = dir.join("out.mp4");
        let config = config(&["--output", output.to_str().unwrap()]);
        assert_eq!(config.display, ":1");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn display_renders_all_fields() {
        let dir = temp_dir("display-impl");
        let output = dir.join("out.mp4");
        let config = config(&[
            "--output",
            output.to_str().unwrap(),
            "--fps",
            "24",
            "--duration",
            "5",
        ]);

        assert_eq!(
            config.to_string(),
            format!("output={} fps=24 display=:1 duration=5s", output.display())
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn error_messages_are_actionable() {
        assert_eq!(
            ConfigError::MissingOutput.to_string(),
            "missing required option --output"
        );
        assert_eq!(
            ConfigError::OutputExists(PathBuf::from("/tmp/clip.mp4")).to_string(),
            "refusing to overwrite existing file: /tmp/clip.mp4"
        );
        assert_eq!(ConfigError::EXIT_CODE, 2);
    }
}
