//! Execution of the FFmpeg command built by [`crate::command`].
//!
//! Milestone 3.1 is deliberately synchronous: spawn FFmpeg, wait for it to
//! finish, and report the outcome. Ctrl-C handling and graceful shutdown are
//! not implemented yet, so this module never signals or kills the child.

use std::error::Error;
use std::fmt;
use std::io;
use std::process::{Command, ExitStatus};

/// Why running FFmpeg did not succeed.
#[derive(Debug)]
pub enum ProcessError {
    /// The executable could not be spawned (missing binary, permission denied).
    Spawn { source: io::Error },
    /// FFmpeg ran but reported failure.
    Failed { status: ExitStatus },
}

impl ProcessError {
    /// Exit code for runtime failures (see README).
    pub const EXIT_CODE: u8 = 1;
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { source } => write!(f, "could not start ffmpeg: {source}"),
            Self::Failed { status } => match status.code() {
                Some(code) => write!(f, "ffmpeg exited unsuccessfully (status {code})"),
                None => write!(f, "ffmpeg was terminated by a signal"),
            },
        }
    }
}

impl Error for ProcessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Spawn { source } => Some(source),
            Self::Failed { .. } => None,
        }
    }
}

/// Run `command` to completion and report its outcome.
///
/// Stdio is inherited, so FFmpeg's own diagnostics reach the terminal and are
/// neither captured nor suppressed.
pub fn run(command: &mut Command) -> Result<(), ProcessError> {
    let status = command
        .status()
        .map_err(|source| ProcessError::Spawn { source })?;

    if status.success() {
        Ok(())
    } else {
        Err(ProcessError::Failed { status })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::process::Command;

    // These spawn the standard Unix `true`/`false` utilities rather than
    // FFmpeg, so they need neither a display nor an FFmpeg installation.

    #[cfg(unix)]
    #[test]
    fn succeeds_when_the_child_exits_zero() {
        let mut command = Command::new("true");
        assert!(run(&mut command).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn fails_when_the_child_exits_nonzero() {
        let mut command = Command::new("false");
        let error = run(&mut command).expect_err("false must fail");

        match error {
            ProcessError::Failed { status } => assert_eq!(status.code(), Some(1)),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn spawn_failure_is_reported_separately() {
        let mut command = Command::new("rust-recorder-program-that-does-not-exist");
        let error = run(&mut command).expect_err("missing program must fail");

        match error {
            ProcessError::Spawn { source } => {
                assert_eq!(source.kind(), io::ErrorKind::NotFound);
            }
            other => panic!("expected Spawn, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn message_mentions_the_exit_status() {
        let status = Command::new("false").status().expect("spawn false");
        let message = ProcessError::Failed { status }.to_string();
        assert!(
            message.contains("status 1"),
            "unexpected message: {message}"
        );
    }

    #[test]
    fn runtime_exit_code_is_one() {
        assert_eq!(ProcessError::EXIT_CODE, 1);
    }
}
