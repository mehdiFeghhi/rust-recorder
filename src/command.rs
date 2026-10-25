//! Construction of the FFmpeg command line.
//!
//! This module only *describes* the process: it builds a
//! [`std::process::Command`] and never spawns it. Execution lives in
//! [`crate::process::run`]. Every argument is passed as its own OS string, so
//! no shell parses the values; a path containing spaces or `;` stays one
//! argument.

use std::process::Command;

use crate::capture::Capture;
use crate::config::{Config, ConfigError, Container};

/// Executable name, resolved through `PATH`.
pub const FFMPEG_PROGRAM: &str = "ffmpeg";

/// x264 speed/size trade-off. Screen content compresses well even at this
/// preset, which keeps 1080p30 capture affordable on a laptop CPU.
pub const X264_PRESET: &str = "veryfast";

/// Build the FFmpeg invocation for `config`.
///
/// This only describes the process; [`crate::process::run`] executes it.
pub fn build(config: &Config) -> Result<Command, ConfigError> {
    let container = Container::from_path(&config.output)?;

    let mut command = Command::new(FFMPEG_PROGRAM);
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-nostats");

    match &config.capture {
        // Omitting -video_size makes x11grab capture the whole screen.
        Capture::FullScreen => {
            command
                .arg("-f")
                .arg("x11grab")
                .arg("-framerate")
                .arg(config.fps.to_string())
                .arg("-i")
                .arg(&config.display);
        }
        Capture::Region(region) => {
            command
                .arg("-f")
                .arg("x11grab")
                .arg("-video_size")
                .arg(region.video_size())
                .arg("-framerate")
                .arg(config.fps.to_string())
                .arg("-i")
                .arg(format!("{}{}", config.display, region.offset()));
        }
    }

    command
        // Output: H.264 that plays everywhere, in the selected container.
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg(X264_PRESET)
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-f")
        .arg(container.ffmpeg_format());

    if let Some(duration) = config.duration {
        command.arg("-t").arg(duration.to_string());
    }

    // `-n` refuses to replace an existing file even if validation was bypassed
    // or the file appeared between validation and spawning.
    command.arg("-n").arg(&config.output);

    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::path::PathBuf;

    fn config(output: &str) -> Config {
        Config {
            output: PathBuf::from(output),
            fps: 30,
            duration: None,
            display: ":1".to_owned(),
            capture: Capture::FullScreen,
        }
    }

    fn args(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    fn command_for(config: &Config) -> Command {
        build(config).expect("expected a valid command")
    }

    fn assert_has_pair(args: &[String], flag: &str, value: &str) {
        assert!(
            args.windows(2)
                .any(|pair| pair[0] == flag && pair[1] == value),
            "expected {flag} {value} in {args:?}"
        );
    }

    #[test]
    fn uses_ffmpeg_from_path() {
        let command = command_for(&config("out.mp4"));
        assert_eq!(command.get_program(), OsStr::new(FFMPEG_PROGRAM));
    }

    #[test]
    fn captures_the_requested_display_at_the_requested_rate() {
        let mut cfg = config("out.mp4");
        cfg.fps = 60;
        cfg.display = ":0".to_owned();
        let args = args(&command_for(&cfg));

        assert_has_pair(&args, "-f", "x11grab");
        assert_has_pair(&args, "-framerate", "60");
        assert_has_pair(&args, "-i", ":0");
    }

    #[test]
    fn encodes_h264_with_yuv420p() {
        let args = args(&command_for(&config("out.mp4")));

        assert_has_pair(&args, "-c:v", "libx264");
        assert_has_pair(&args, "-pix_fmt", "yuv420p");
        assert_has_pair(&args, "-preset", X264_PRESET);
    }

    #[test]
    fn refuses_to_overwrite_existing_files() {
        let args = args(&command_for(&config("out.mp4")));

        assert!(args.contains(&"-n".to_owned()), "missing -n: {args:?}");
        assert!(
            !args.contains(&"-y".to_owned()),
            "-y would overwrite: {args:?}"
        );
    }

    #[test]
    fn output_path_is_the_last_argument() {
        let cfg = config("/tmp/clip.mp4");
        let args = args(&command_for(&cfg));
        assert_eq!(args.last().map(String::as_str), Some("/tmp/clip.mp4"));
    }

    #[test]
    fn selects_mp4_container() {
        let args = args(&command_for(&config("clip.mp4")));
        assert_has_pair(&args, "-f", "mp4");
    }

    #[test]
    fn selects_matroska_container() {
        let args = args(&command_for(&config("clip.mkv")));
        assert_has_pair(&args, "-f", "matroska");
    }

    #[test]
    fn extension_matching_is_case_insensitive() {
        let args = args(&command_for(&config("clip.MP4")));
        assert_has_pair(&args, "-f", "mp4");
    }

    #[test]
    fn region_adds_video_size_and_offset() {
        use crate::capture::Region;

        let mut cfg = config("clip.mp4");
        cfg.capture = Capture::Region(Region {
            width: 640,
            height: 480,
            x: 100,
            y: 80,
        });
        let args = args(&command_for(&cfg));

        assert_has_pair(&args, "-video_size", "640x480");
        assert_has_pair(&args, "-i", ":1+100,80");
    }

    #[test]
    fn full_screen_has_no_video_size() {
        let args = args(&command_for(&config("clip.mp4")));
        assert!(!args.iter().any(|arg| arg == "-video_size"));
        assert_has_pair(&args, "-i", ":1");
    }

    #[test]
    fn duration_is_omitted_when_not_requested() {
        let args = args(&command_for(&config("clip.mp4")));
        assert!(
            !args.iter().any(|arg| arg == "-t"),
            "unexpected -t: {args:?}"
        );
    }

    #[test]
    fn duration_is_passed_when_requested() {
        let mut cfg = config("clip.mp4");
        cfg.duration = Some(15);
        let args = args(&command_for(&cfg));
        assert_has_pair(&args, "-t", "15");
    }

    #[test]
    fn path_with_spaces_and_metacharacters_is_a_single_argument() {
        // The point of separate arguments: the shell never sees this string.
        let path = "/tmp/clip; rm -rf ~/important.mp4";
        let cfg = config(path);
        let args = args(&command_for(&cfg));

        assert_eq!(args.last().map(String::as_str), Some(path));
        assert_eq!(args.iter().filter(|arg| *arg == path).count(), 1);
    }

    #[test]
    fn invalid_extension_is_rejected() {
        let error = build(&config("clip.avi")).expect_err("avi must be rejected");
        assert_eq!(error, ConfigError::UnsupportedExtension("avi".to_owned()));
    }

    #[test]
    fn missing_extension_is_rejected() {
        let error = build(&config("clip")).expect_err("no extension must be rejected");
        assert_eq!(error, ConfigError::MissingExtension);
    }
}
