//! `rust-recorder` entry point.
//!
//! Flow: parse and validate the command line into a [`Config`], build the
//! FFmpeg invocation with [`command::build`], then execute it to completion
//! with [`process::run`]. Exit code 2 means a usage/configuration error; exit
//! code 1 means FFmpeg could not be started or exited unsuccessfully.

use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::process::ExitCode;

mod command;
mod config;
mod process;

use config::{Config, ConfigError, ParseResult};
use process::ProcessError;

/// Top-level failure, keeping configuration and process errors distinct so
/// each can map to its own exit code.
#[derive(Debug)]
enum AppError {
    Config(ConfigError),
    Process(ProcessError),
}

impl AppError {
    fn exit_code(&self) -> u8 {
        match self {
            Self::Config(_) => ConfigError::EXIT_CODE,
            Self::Process(_) => ProcessError::EXIT_CODE,
        }
    }
}

impl From<ConfigError> for AppError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

impl From<ProcessError> for AppError {
    fn from(error: ProcessError) -> Self {
        Self::Process(error)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(error) => write!(f, "{error}"),
            Self::Process(error) => write!(f, "{error}"),
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Config(error) => Some(error as &(dyn Error + 'static)),
            Self::Process(error) => Some(error as &(dyn Error + 'static)),
        }
    }
}

fn main() -> ExitCode {
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
        ParseResult::Run(config) => {
            let mut cmd = command::build(&config)?;
            process::run(&mut cmd)?;
            // Only reached after FFmpeg exits successfully.
            eprintln!("recording finished: {}", config.output.display());
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
