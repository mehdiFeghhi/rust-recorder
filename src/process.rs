//! Execution of the FFmpeg command built by [`crate::command`].
//!
//! FFmpeg runs in its own process group, so a terminal Ctrl-C reaches only this
//! process. The handler just sets a flag; the run loop then asks FFmpeg to
//! finish by writing `q` to its stdin (FFmpeg's interactive quit command) and
//! waits for it to finalize the container. If it does not exit within
//! [`SHUTDOWN_GRACE`], it is killed and reaped.

use std::error::Error;
use std::fmt;
use std::io::{self, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// How long FFmpeg gets to finalize after an interrupt before it is killed.
pub const SHUTDOWN_GRACE: Duration = Duration::from_secs(10);

/// How often the run loop checks the child and the interrupt flag.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Why running FFmpeg did not succeed.
#[derive(Debug)]
pub enum ProcessError {
    /// The executable could not be spawned (missing binary, permission denied).
    Spawn { source: io::Error },
    /// Waiting for the child failed.
    Wait { source: io::Error },
    /// The graceful-quit request could not be written to FFmpeg's stdin.
    Signal { source: io::Error },
    /// The Ctrl-C handler could not be installed.
    Handler { source: ctrlc::Error },
    /// FFmpeg ran but reported failure.
    Failed { status: ExitStatus },
    /// FFmpeg ignored the quit request and was killed after the grace period.
    ShutdownTimeout { grace: Duration },
}

impl ProcessError {
    /// Exit code for runtime failures (see README).
    pub const EXIT_CODE: u8 = 1;
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { source } => write!(f, "could not start ffmpeg: {source}"),
            Self::Wait { source } => write!(f, "could not wait for ffmpeg: {source}"),
            Self::Signal { source } => write!(f, "could not ask ffmpeg to stop: {source}"),
            Self::Handler { source } => {
                write!(f, "could not install the Ctrl-C handler: {source}")
            }
            Self::Failed { status } => match status.code() {
                Some(code) => write!(f, "ffmpeg exited unsuccessfully (status {code})"),
                None => write!(f, "ffmpeg was terminated by a signal"),
            },
            Self::ShutdownTimeout { grace } => write!(
                f,
                "ffmpeg did not exit within {grace:?} of the interrupt and was killed"
            ),
        }
    }
}

impl Error for ProcessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Spawn { source } | Self::Wait { source } | Self::Signal { source } => {
                Some(source)
            }
            Self::Handler { .. } | Self::Failed { .. } | Self::ShutdownTimeout { .. } => None,
        }
    }
}

/// Install the Ctrl-C handler that flips `interrupted` to `true`.
///
/// `ctrlc` allows only one handler per process, so this is called once from
/// `main` after configuration has been validated.
pub fn install_interrupt_handler(interrupted: Arc<AtomicBool>) -> Result<(), ProcessError> {
    ctrlc::set_handler(move || interrupted.store(true, Ordering::SeqCst))
        .map_err(|source| ProcessError::Handler { source })
}

/// Run `command` to completion, shutting it down gracefully on Ctrl-C.
///
/// Stdio is inherited, so FFmpeg's own diagnostics reach the terminal and are
/// neither captured nor suppressed.
pub fn run(command: &mut Command, interrupted: &AtomicBool) -> Result<(), ProcessError> {
    run_with_grace(command, interrupted, SHUTDOWN_GRACE)
}

fn run_with_grace(
    command: &mut Command,
    interrupted: &AtomicBool,
    grace: Duration,
) -> Result<(), ProcessError> {
    // Keep the child out of the terminal's foreground process group, so Ctrl-C
    // reaches only this process and shutdown is explicit and observable.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command.stdin(Stdio::piped());

    let mut child = ChildGuard(
        command
            .spawn()
            .map_err(|source| ProcessError::Spawn { source })?,
    );

    loop {
        if let Some(status) = child
            .0
            .try_wait()
            .map_err(|source| ProcessError::Wait { source })?
        {
            return exit_status(status);
        }

        if interrupted.load(Ordering::SeqCst) {
            return shutdown(&mut child, grace);
        }

        thread::sleep(POLL_INTERVAL);
    }
}

/// Ask FFmpeg to finalize, then wait at most `grace` before killing it.
fn shutdown(child: &mut ChildGuard, grace: Duration) -> Result<(), ProcessError> {
    if let Some(mut stdin) = child.0.stdin.take() {
        match stdin.write_all(b"q\n") {
            Ok(()) => {}
            // A broken pipe means FFmpeg already exited; the wait below
            // decides the outcome, so only other I/O errors are reported.
            Err(source) if source.kind() == io::ErrorKind::BrokenPipe => {}
            Err(source) => return Err(ProcessError::Signal { source }),
        }
        // Dropping the handle closes the pipe, so FFmpeg also sees EOF.
    }

    let deadline = Instant::now() + grace;
    loop {
        if let Some(status) = child
            .0
            .try_wait()
            .map_err(|source| ProcessError::Wait { source })?
        {
            return exit_status(status);
        }

        if Instant::now() >= deadline {
            // Best effort: the child may have exited between the check and the
            // kill. Either way the guard below reaps it.
            let _ = child.0.kill();
            let _ = child.0.wait();
            return Err(ProcessError::ShutdownTimeout { grace });
        }

        thread::sleep(POLL_INTERVAL);
    }
}

fn exit_status(status: ExitStatus) -> Result<(), ProcessError> {
    if status.success() {
        Ok(())
    } else {
        Err(ProcessError::Failed { status })
    }
}

/// Kills and reaps the child if it is dropped before being waited on.
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::process::Command;

    fn flag(set: bool) -> AtomicBool {
        AtomicBool::new(set)
    }

    // These spawn the standard Unix coreutils rather than FFmpeg, so they need
    // neither a display nor an FFmpeg installation.

    #[cfg(unix)]
    #[test]
    fn succeeds_when_the_child_exits_zero() {
        let mut command = Command::new("true");
        assert!(run(&mut command, &flag(false)).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn fails_when_the_child_exits_nonzero() {
        let mut command = Command::new("false");
        let error = run(&mut command, &flag(false)).expect_err("false must fail");

        match error {
            ProcessError::Failed { status } => assert_eq!(status.code(), Some(1)),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn spawn_failure_is_reported_separately() {
        let mut command = Command::new("rust-recorder-program-that-does-not-exist");
        let error = run(&mut command, &flag(false)).expect_err("missing program must fail");

        match error {
            ProcessError::Spawn { source } => {
                assert_eq!(source.kind(), io::ErrorKind::NotFound);
            }
            other => panic!("expected Spawn, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn interrupt_asks_the_child_to_finish() {
        // `cat` copies stdin to stdout and exits on EOF; `q` + closed stdin is
        // exactly the shutdown sequence FFmpeg receives.
        let mut command = Command::new("cat");
        command.stdout(Stdio::null());

        let result = run_with_grace(&mut command, &flag(true), Duration::from_secs(5));
        assert!(
            result.is_ok(),
            "cat should exit after q and EOF: {result:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn unresponsive_child_is_killed_after_the_grace_period() {
        let mut command = Command::new("sleep");
        command.arg("30");

        let started = Instant::now();
        let error = run_with_grace(&mut command, &flag(true), Duration::from_millis(200))
            .expect_err("sleep must time out");

        assert!(matches!(error, ProcessError::ShutdownTimeout { .. }));
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "timeout path took too long"
        );
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
