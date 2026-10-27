//! Construction of the FFmpeg command line.
//!
//! This module only *describes* the process: it builds a
//! [`std::process::Command`] and never spawns it. Execution lives in
//! [`crate::process::run`]. Every argument is passed as its own OS string, so
//! no shell parses the values; a path containing spaces or `;` stays one
//! argument.

use std::process::Command;

use crate::audio::AudioMode;
use crate::capture::Capture;
use crate::config::{Config, ConfigError, Container};

/// Executable name, resolved through `PATH`.
pub const FFMPEG_PROGRAM: &str = "ffmpeg";

/// x264 speed/size trade-off. Screen content compresses well even at this
/// preset, which keeps 1080p30 capture affordable on a laptop CPU.
pub const X264_PRESET: &str = "veryfast";

/// PulseAudio fragment size. FFmpeg's default buffering can delay the first
/// frame and finalization by several seconds; 1024 frames starts recording in
/// about a second without truncating audio.
pub const PULSE_FRAGMENT_SIZE: u32 = 1024;

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

    let audio_devices = audio_devices(config)?;

    if !config.audio_only {
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
            Capture::SelectRegion => {
                command
                    .arg("-f")
                    .arg("x11grab")
                    .arg("-framerate")
                    .arg(config.fps.to_string())
                    .arg("-select_region")
                    .arg("1")
                    .arg("-i")
                    .arg(&config.display);
            }
            Capture::Monitor(index) => {
                // `main` resolves monitor selection before building the command.
                return Err(ConfigError::MonitorNotResolved(*index));
            }
        }
    }

    for device in &audio_devices {
        command
            .arg("-f")
            .arg("pulse")
            .arg("-fragment_size")
            .arg(PULSE_FRAGMENT_SIZE.to_string())
            .arg("-i")
            .arg(device);
    }

    if !config.audio_only {
        command
            // Output: H.264 that plays everywhere, in the selected container.
            .arg("-c:v")
            .arg("libx264")
            .arg("-preset")
            .arg(X264_PRESET)
            .arg("-pix_fmt")
            .arg("yuv420p");
    }

    command.arg("-f").arg(container.ffmpeg_format());

    // Input 0 is the video unless we are recording audio only; audio streams
    // follow it.
    let first_audio = if config.audio_only { 0 } else { 1 };

    // Only map explicitly when there is audio to keep in sync with; a
    // video-only command keeps FFmpeg's default stream selection.
    if !config.audio_only && !audio_devices.is_empty() {
        command.arg("-map").arg("0:v:0");
    }

    match audio_devices.len() {
        0 => {}
        1 => {
            command.arg("-map").arg(format!("{first_audio}:a:0"));
        }
        _ => {
            let second_audio = first_audio + 1;
            command
                .arg("-filter_complex")
                .arg(format!(
                    "[{first_audio}:a][{second_audio}:a]amix=inputs=2:duration=longest:dropout_transition=0[a]"
                ))
                .arg("-map")
                .arg("[a]");
        }
    }

    if !audio_devices.is_empty() {
        command
            .arg("-c:a")
            .arg("aac")
            .arg("-b:a")
            .arg("192k")
            .arg("-ar")
            .arg("48000")
            .arg("-ac")
            .arg("2");
    }

    if let Some(duration) = config.duration {
        command.arg("-t").arg(duration.to_string());
    }

    // `-n` refuses to replace an existing file even if validation was bypassed
    // or the file appeared between validation and spawning.
    command.arg("-n").arg(&config.output);

    Ok(command)
}

/// The resolved pulse sources for the selected audio mode.
fn audio_devices(config: &Config) -> Result<Vec<&str>, ConfigError> {
    match config.audio {
        AudioMode::None => Ok(Vec::new()),
        AudioMode::Microphone => Ok(vec![
            config
                .microphone_device
                .as_deref()
                .ok_or(ConfigError::MissingAudioDevice)?,
        ]),
        AudioMode::System => Ok(vec![
            config
                .system_audio_device
                .as_deref()
                .ok_or(ConfigError::MissingSystemAudioDevice)?,
        ]),
        AudioMode::Both => Ok(vec![
            config
                .microphone_device
                .as_deref()
                .ok_or(ConfigError::MissingAudioDevice)?,
            config
                .system_audio_device
                .as_deref()
                .ok_or(ConfigError::MissingSystemAudioDevice)?,
        ]),
    }
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
            audio: AudioMode::None,
            microphone_device: None,
            system_audio_device: None,
            audio_only: false,
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
    fn microphone_adds_a_pulse_input_and_mapping() {
        let mut cfg = config("clip.mp4");
        cfg.audio = AudioMode::Microphone;
        cfg.microphone_device = Some("alsa_input.test".to_owned());
        let args = args(&command_for(&cfg));

        assert_has_pair(&args, "-f", "pulse");
        assert_has_pair(&args, "-i", "alsa_input.test");
        assert_has_pair(&args, "-map", "0:v:0");
        assert_has_pair(&args, "-map", "1:a:0");
        assert_has_pair(&args, "-c:a", "aac");
    }

    #[test]
    fn system_audio_adds_the_monitor_input() {
        let mut cfg = config("clip.mp4");
        cfg.audio = AudioMode::System;
        cfg.system_audio_device = Some("sink.monitor".to_owned());
        let args = args(&command_for(&cfg));

        assert_has_pair(&args, "-f", "pulse");
        assert_has_pair(&args, "-i", "sink.monitor");
        assert_has_pair(&args, "-map", "1:a:0");
    }

    #[test]
    fn audio_only_microphone_has_no_video_input() {
        let mut cfg = config("voice.mp4");
        cfg.audio = AudioMode::Microphone;
        cfg.microphone_device = Some("mic.input".to_owned());
        cfg.audio_only = true;
        let args = args(&command_for(&cfg));

        assert!(!args.iter().any(|arg| arg == "x11grab"));
        assert!(!args.iter().any(|arg| arg == "-c:v"));
        assert_has_pair(&args, "-f", "pulse");
        assert_has_pair(&args, "-fragment_size", "1024");
        assert_has_pair(&args, "-i", "mic.input");
        assert_has_pair(&args, "-map", "0:a:0");
    }

    #[test]
    fn audio_only_both_mixes_from_input_zero() {
        let mut cfg = config("voice.mp4");
        cfg.audio = AudioMode::Both;
        cfg.microphone_device = Some("mic.input".to_owned());
        cfg.system_audio_device = Some("sink.monitor".to_owned());
        cfg.audio_only = true;
        let args = args(&command_for(&cfg));

        assert!(!args.iter().any(|arg| arg == "x11grab"));
        assert!(
            args.iter()
                .any(|arg| arg.contains("[0:a][1:a]") && arg.contains("amix=inputs=2")),
            "argv: {args:?}"
        );
        assert_has_pair(&args, "-map", "[a]");
    }

    #[test]
    fn both_modes_mix_two_inputs() {
        let mut cfg = config("clip.mp4");
        cfg.audio = AudioMode::Both;
        cfg.microphone_device = Some("mic.input".to_owned());
        cfg.system_audio_device = Some("sink.monitor".to_owned());
        let args = args(&command_for(&cfg));

        assert_has_pair(&args, "-i", "mic.input");
        assert_has_pair(&args, "-i", "sink.monitor");
        assert!(
            args.iter()
                .any(|arg| arg.contains("amix=inputs=2") && arg.contains("[1:a][2:a]")),
            "argv: {args:?}"
        );
        assert_has_pair(&args, "-map", "[a]");
        assert_eq!(args.iter().filter(|arg| *arg == "pulse").count(), 2);
    }

    #[test]
    fn unresolved_system_audio_is_rejected() {
        let mut cfg = config("clip.mp4");
        cfg.audio = AudioMode::System;

        let error = build(&cfg).expect_err("system audio must be resolved first");
        assert_eq!(error, ConfigError::MissingSystemAudioDevice);
    }

    #[test]
    fn video_only_recording_has_no_audio_argument() {
        let args = args(&command_for(&config("clip.mp4")));

        assert!(!args.iter().any(|arg| arg == "pulse"));
        assert!(!args.iter().any(|arg| arg == "-map"));
        assert!(!args.iter().any(|arg| arg == "-c:a"));
    }

    #[test]
    fn unresolved_microphone_is_rejected() {
        let mut cfg = config("clip.mp4");
        cfg.audio = AudioMode::Microphone;

        let error = build(&cfg).expect_err("microphone must be resolved first");
        assert_eq!(error, ConfigError::MissingAudioDevice);
    }

    #[test]
    fn interactive_selection_passes_the_flag() {
        let mut cfg = config("clip.mp4");
        cfg.capture = Capture::SelectRegion;
        let args = args(&command_for(&cfg));

        assert_has_pair(&args, "-select_region", "1");
        assert!(!args.iter().any(|arg| arg == "-video_size"));
        assert_has_pair(&args, "-i", ":1");
    }

    #[test]
    fn unresolved_monitor_is_rejected() {
        let mut cfg = config("clip.mp4");
        cfg.capture = Capture::Monitor(2);

        let error = build(&cfg).expect_err("monitor must be resolved first");
        assert_eq!(error, ConfigError::MonitorNotResolved(2));
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
