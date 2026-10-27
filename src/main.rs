//! `rust-recorder` entry point.
//!
//! Flow: parse and validate the command line into a [`Config`], build the
//! FFmpeg invocation with [`command::build`], execute it to completion with
//! [`process::run`], then verify the file with [`output::verify`]. Failures are
//! aggregated by [`error::AppError`], which owns the exit-code mapping.

use std::ffi::OsString;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

mod audio;
mod capture;
mod command;
mod config;
mod error;
mod logging;
mod monitor;
mod output;
mod probe;
mod process;

use capture::Capture;
use config::{Config, ParseResult};
use error::AppError;

fn main() -> ExitCode {
    logging::init();

    match run(std::env::args_os().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}

fn run(args: impl Iterator<Item = OsString>) -> Result<(), AppError> {
    match Config::parse(args)? {
        ParseResult::Help => {
            print_help();
            Ok(())
        }
        ParseResult::Version => {
            println!("rust-recorder {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        ParseResult::ListAudioDevices => {
            let sources = audio::discover()?;
            if sources.is_empty() {
                eprintln!("No audio capture sources found.");
            } else {
                print!("{}", audio::render(&sources));
            }
            Ok(())
        }
        ParseResult::ListMonitors => {
            let monitors = monitor::discover()?;
            if monitors.is_empty() {
                eprintln!("No connected monitors found.");
            } else {
                print!("{}", monitor::render(&monitors));
            }
            Ok(())
        }
        ParseResult::Run(mut config) => {
            match config.audio {
                audio::AudioMode::Microphone => {
                    let sources = audio::discover()?;
                    let selected =
                        audio::select_microphone(config.microphone_device.as_deref(), &sources)?;
                    tracing::info!(device = %selected.id, "using microphone");
                    config.microphone_device = Some(selected.id);
                }
                audio::AudioMode::System => {
                    let sources = audio::discover()?;
                    let selected = audio::select_system_output(
                        config.system_audio_device.as_deref(),
                        &sources,
                    )?;
                    tracing::info!(device = %selected.id, "using system audio");
                    config.system_audio_device = Some(selected.id);
                }
                audio::AudioMode::Both => {
                    let sources = audio::discover()?;
                    let microphone =
                        audio::select_microphone(config.microphone_device.as_deref(), &sources)?;
                    let system = audio::select_system_output(
                        config.system_audio_device.as_deref(),
                        &sources,
                    )?;
                    tracing::info!(
                        microphone = %microphone.id,
                        system = %system.id,
                        "using microphone and system audio"
                    );
                    config.microphone_device = Some(microphone.id);
                    config.system_audio_device = Some(system.id);
                }
                audio::AudioMode::None => {}
            }

            // Monitor indices and explicit regions are resolved against the
            // live display list before the command is built.
            match config.capture {
                Capture::Monitor(index) => {
                    let monitors = monitor::discover()?;
                    config.capture = Capture::Region(capture::select(index, &monitors)?);
                }
                Capture::Region(region) => {
                    let monitors = monitor::discover()?;
                    capture::validate_region(region, &monitors)?;
                }
                // FFmpeg owns the interactive selection; nothing to resolve.
                Capture::SelectRegion | Capture::FullScreen => {}
            }

            let session_id = format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |elapsed| elapsed.as_secs())
            );
            // A WARN-level span stays enabled at the default `warn` filter, so
            // warnings and errors keep the session context; an INFO span would
            // be disabled and its fields lost.
            let span = tracing::warn_span!(
                "session",
                session_id = %session_id,
                output = %config.output.display(),
                fps = config.fps,
                display = %config.display,
                duration_limit = ?config.duration,
            );
            let _guard = span.enter();
            let started = std::time::Instant::now();

            tracing::info!("recording starting");

            let mut cmd = command::build(&config)?;
            let interrupted = Arc::new(AtomicBool::new(false));
            process::install_interrupt_handler(Arc::clone(&interrupted))?;
            process::run(&mut cmd, &interrupted)?;

            // Only reached after FFmpeg exits successfully; the file is then
            // checked so a silent no-output run is not reported as success.
            let output = output::verify(&config.output)?;

            if config.audio_only {
                let audio = probe::inspect_audio(&config.output)?;

                tracing::info!(
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    size = output.size,
                    audio_codec = %audio.codec,
                    duration_s = audio.duration,
                    "recording finished"
                );

                eprintln!(
                    "recording finished: {} ({}, {:.2} s, {} bytes)",
                    config.output.display(),
                    audio_label(&audio),
                    audio.duration,
                    output.size
                );
            } else {
                let probe = probe::inspect(&config.output)?;
                let audio_info = if config.audio == audio::AudioMode::None {
                    None
                } else {
                    Some(probe::inspect_audio(&config.output)?)
                };

                tracing::info!(
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    size = output.size,
                    codec = %probe.codec,
                    width = probe.width,
                    height = probe.height,
                    fps = ?probe.fps,
                    audio_codec = ?audio_info.as_ref().map(|audio| audio.codec.as_str()),
                    duration_s = probe.duration,
                    "recording finished"
                );

                let codecs = match &audio_info {
                    Some(audio) => format!("{} + {}", probe.codec, audio_label(audio)),
                    None => probe.codec.clone(),
                };
                eprintln!(
                    "recording finished: {} ({}x{} @ {} fps, {}, {:.2} s, {} bytes)",
                    config.output.display(),
                    probe.width,
                    probe.height,
                    display_fps(probe.fps),
                    codecs,
                    probe.duration,
                    output.size
                );
            }

            Ok(())
        }
    }
}

fn display_fps(fps: Option<f64>) -> String {
    match fps {
        Some(value) if (value - value.round()).abs() < 0.01 => format!("{value:.0}"),
        Some(value) => format!("{value:.2}"),
        None => "?".to_owned(),
    }
}

fn audio_label(audio: &probe::AudioInfo) -> String {
    match (audio.sample_rate, audio.channels) {
        (Some(rate), Some(2)) => format!("{} {} kHz stereo", audio.codec, rate / 1000),
        (Some(rate), Some(1)) => format!("{} {} kHz mono", audio.codec, rate / 1000),
        (Some(rate), Some(channels)) => {
            format!("{} {} kHz {channels} ch", audio.codec, rate / 1000)
        }
        _ => audio.codec.clone(),
    }
}

fn print_help() {
    let version = env!("CARGO_PKG_VERSION");
    println!(
        "rust-recorder {version}
Record an X11 display to a video file.

USAGE:
    rust-recorder --output <PATH> [OPTIONS]

OPTIONS:
    -o, --output <PATH>        Destination file (.mp4 or .mkv); must not exist
        --fps <N>              Frames per second (1-144) [default: 30]
        --duration <SECONDS>   Stop after N seconds (1-86400); omit to record until Ctrl-C
        --display <:N>         X11 display [default: $DISPLAY]
        --region <WxH+X+Y>     Capture a rectangle, e.g. 1280x720+100+80
        --list-monitors        List connected monitors and exit
        --list-audio-devices   List audio capture sources and exit
        --audio <MODE>         Audio mode: none, mic, system, or both [default: none]
        --audio-only           Record audio without screen capture
        --microphone-device <ID>  Microphone source ID from --list-audio-devices
        --system-audio-device <ID>  Output-monitor source ID from --list-audio-devices
        --monitor <N>          Capture the Nth monitor from --list-monitors
        --select-region        Select the region with the mouse, then record
    -h, --help                 Print this help
    -V, --version              Print version

EXIT CODES:
    0  success
    1  runtime failure
    2  invalid usage or configuration"
    );
}
