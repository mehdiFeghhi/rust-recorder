//! Logging setup.
//!
//! Diagnostics go to stderr and are filtered by `RUST_LOG`. The default is
//! `warn`, so normal CLI output stays clean; run with
//! `RUST_LOG=rust_recorder=info` (or `debug`) to see recording lifecycle
//! detail. Captured screen or audio content is never logged.

use std::io::{self, IsTerminal};

use tracing_subscriber::EnvFilter;

/// Install the global tracing subscriber.
///
/// Failure is deliberately non-fatal: the recorder still works without
/// diagnostics, and a warning explains why logging is unavailable.
pub fn init() {
    let filter = std::env::var("RUST_LOG")
        .ok()
        .and_then(|value| EnvFilter::try_new(value).ok())
        .unwrap_or_else(|| EnvFilter::new("warn"));

    let result = tracing_subscriber::fmt()
        .with_env_filter(filter)
        // stdout is reserved for program output; diagnostics belong on stderr.
        .with_writer(io::stderr)
        .with_ansi(io::stderr().is_terminal())
        .try_init();

    if let Err(error) = result {
        eprintln!("warning: could not initialize logging: {error}");
    }
}
