//! `rust-recorder` entry point.
//!
//! Milestone 1 covers argument parsing and validation only. It prints the
//! effective configuration and exits; FFmpeg is introduced in milestone 3.

use std::ffi::OsString;
use std::process::ExitCode;

mod command;
mod config;

use config::{Config, ConfigError, ParseResult};

fn main() -> ExitCode {
    match run(std::env::args_os().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(ConfigError::EXIT_CODE)
        }
    }
}

fn run(args: impl Iterator<Item = OsString>) -> Result<(), ConfigError> {
    match Config::parse(args)? {
        ParseResult::Help => {
            print_help();
            Ok(())
        }
        ParseResult::Run(config) => {
            // Milestone 2 checkpoint: print the command that milestone 3 will
            // spawn. It is never passed through a shell.
            let cmd = command::build(&config)?;
            println!("{cmd:?}");
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
    -h, --help                 Print this help

EXIT CODES:
    0  success
    1  runtime failure
    2  invalid usage or configuration"
    );
}
