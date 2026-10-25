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

mod capture;
mod command;
mod config;
mod error;
mod logging;
mod monitor;
mod output;
mod probe;
mod process;

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
        ParseResult::ListMonitors => {
            let monitors = monitor::discover()?;
            if monitors.is_empty() {
                eprintln!("No connected monitors found.");
            } else {
                print!("{}", monitor::render(&monitors));
            }
            Ok(())
        }
        ParseResult::Run(config) => {
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
            let probe = probe::inspect(&config.output)?;

            tracing::info!(
                elapsed_ms = started.elapsed().as_millis() as u64,
                size = output.size,
                codec = %probe.codec,
                duration_s = probe.duration,
                "recording finished"
            );

            eprintln!(
                "recording finished: {} ({} bytes, {:.2} s, {})",
                config.output.display(),
                output.size,
                probe.duration,
                probe.codec
            );
            Ok(())
        }
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
    -h, --help                 Print this help
    -V, --version              Print version

EXIT CODES:
    0  success
    1  runtime failure
    2  invalid usage or configuration"
    );
}
