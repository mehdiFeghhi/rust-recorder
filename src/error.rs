//! Top-level application error.
//!
//! Each subsystem keeps its own typed error; this enum only aggregates them
//! and owns the exit-code mapping: 2 for usage/configuration problems, 1 for
//! runtime failures.

use std::error::Error;
use std::fmt;

use crate::config::ConfigError;
use crate::monitor::MonitorError;
use crate::output::OutputError;
use crate::probe::ProbeError;
use crate::process::ProcessError;

/// Failure that terminates the program.
#[derive(Debug)]
pub enum AppError {
    Config(ConfigError),
    Process(ProcessError),
    Output(OutputError),
    Probe(ProbeError),
    Monitor(MonitorError),
}

impl AppError {
    /// Exit code for this failure (see README).
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Config(_) => ConfigError::EXIT_CODE,
            Self::Process(_) => ProcessError::EXIT_CODE,
            Self::Output(_) => OutputError::EXIT_CODE,
            Self::Probe(_) => ProbeError::EXIT_CODE,
            Self::Monitor(_) => MonitorError::EXIT_CODE,
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

impl From<OutputError> for AppError {
    fn from(error: OutputError) -> Self {
        Self::Output(error)
    }
}

impl From<ProbeError> for AppError {
    fn from(error: ProbeError) -> Self {
        Self::Probe(error)
    }
}

impl From<MonitorError> for AppError {
    fn from(error: MonitorError) -> Self {
        Self::Monitor(error)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(error) => write!(f, "{error}"),
            Self::Process(error) => write!(f, "{error}"),
            Self::Output(error) => write!(f, "{error}"),
            Self::Probe(error) => write!(f, "{error}"),
            Self::Monitor(error) => write!(f, "{error}"),
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Config(error) => Some(error as &(dyn Error + 'static)),
            Self::Process(error) => Some(error as &(dyn Error + 'static)),
            Self::Output(error) => Some(error as &(dyn Error + 'static)),
            Self::Probe(error) => Some(error as &(dyn Error + 'static)),
            Self::Monitor(error) => Some(error as &(dyn Error + 'static)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn configuration_errors_exit_with_two() {
        let error = AppError::from(ConfigError::MissingOutput);
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn process_errors_exit_with_one() {
        let error = AppError::from(ProcessError::ShutdownTimeout {
            grace: Duration::from_secs(1),
        });
        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn monitor_errors_exit_with_one() {
        let error = AppError::from(MonitorError::Spawn {
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "no xrandr"),
        });
        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn probe_errors_exit_with_one() {
        let error = AppError::from(ProbeError::NoVideoStream);
        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn output_errors_exit_with_one() {
        let error = AppError::from(OutputError::Empty {
            path: PathBuf::from("/tmp/out.mp4"),
        });
        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn display_delegates_to_the_inner_error() {
        let error = AppError::from(ConfigError::MissingOutput);
        assert_eq!(error.to_string(), "missing required option --output");
    }

    #[test]
    fn source_exposes_the_inner_error() {
        let error = AppError::from(ConfigError::MissingOutput);
        assert!(error.source().is_some());
    }
}
