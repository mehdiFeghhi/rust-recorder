//! End-to-end tests for the `rust-recorder` binary.
//!
//! The tests run the real executable but replace `ffmpeg` and `ffprobe` with
//! small shell scripts earlier on `PATH`. They therefore need no X11 display,
//! no FFmpeg installation, and no real capture. The project targets Linux/X11
//! only, hence the Unix gate.
//!
//! A real capture test is available behind `#[ignore]`:
//! `cargo test --locked -- --ignored`.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

/// Path to the binary under test, provided by Cargo for integration tests.
const BIN: &str = env!("CARGO_BIN_EXE_rust-recorder");

const FFMPEG_WRITES_FILE: &str = r#"#!/bin/sh
last=""
for argument in "$@"; do last="$argument"; done
printf 'payload' > "$last"
exit 0
"#;

const FFMPEG_EXITS_ZERO_SILENTLY: &str = r#"#!/bin/sh
exit 0
"#;

const FFMPEG_WRITES_EMPTY_FILE: &str = r#"#!/bin/sh
last=""
for argument in "$@"; do last="$argument"; done
: > "$last"
exit 0
"#;

const FFMPEG_FAILS: &str = r#"#!/bin/sh
echo 'boom from ffmpeg' >&2
exit 7
"#;

const FFPROBE_SUCCEEDS: &str = r#"#!/bin/sh
printf '%s\n' 'codec_name=h264' 'duration=1.500000'
exit 0
"#;

const FFPROBE_FAILS: &str = r#"#!/bin/sh
echo 'probe-boom' >&2
exit 1
"#;

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "rust-recorder-it-{}-{label}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temporary test directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn write_script(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    fs::write(&path, body).expect("write fake tool");
    let mut permissions = fs::metadata(&path)
        .expect("read fake tool metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("make fake tool executable");
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .env("PATH", dir)
        .env_remove("RUST_LOG")
        .env("DISPLAY", ":1")
        .output()
        .expect("run rust-recorder")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn configuration_errors_exit_with_two() {
    let dir = TempDir::new("config");
    let output = run(dir.path(), &[]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("missing required option --output"));
}

#[test]
fn existing_output_is_never_overwritten() {
    let dir = TempDir::new("exists");
    let output_path = dir.join("out.mp4");
    fs::write(&output_path, b"keep me").expect("create existing file");

    let output = run(
        dir.path(),
        &["--output", output_path.to_str().expect("utf-8 path")],
    );

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("refusing to overwrite"));
    assert_eq!(
        fs::read(&output_path).expect("read existing file"),
        b"keep me"
    );
}

#[test]
fn missing_ffmpeg_is_reported() {
    let dir = TempDir::new("no-ffmpeg");
    let output_path = dir.join("out.mp4");

    let output = run(
        dir.path(),
        &["--output", output_path.to_str().expect("utf-8 path")],
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("could not start ffmpeg"));
}

#[test]
fn ffmpeg_failure_is_reported() {
    let dir = TempDir::new("ffmpeg-fails");
    write_script(dir.path(), "ffmpeg", FFMPEG_FAILS);
    let output_path = dir.join("out.mp4");

    let output = run(
        dir.path(),
        &["--output", output_path.to_str().expect("utf-8 path")],
    );

    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(message.contains("status 7"), "unexpected: {message}");
    assert!(
        message.contains("boom from ffmpeg"),
        "unexpected: {message}"
    );
}

#[test]
fn missing_output_file_is_rejected() {
    let dir = TempDir::new("no-output");
    write_script(dir.path(), "ffmpeg", FFMPEG_EXITS_ZERO_SILENTLY);
    let output_path = dir.join("out.mp4");

    let output = run(
        dir.path(),
        &["--output", output_path.to_str().expect("utf-8 path")],
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("no output file exists"));
}

#[test]
fn empty_output_file_is_rejected() {
    let dir = TempDir::new("empty-output");
    write_script(dir.path(), "ffmpeg", FFMPEG_WRITES_EMPTY_FILE);
    let output_path = dir.join("out.mp4");

    let output = run(
        dir.path(),
        &["--output", output_path.to_str().expect("utf-8 path")],
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("wrote no data"));
}

#[test]
fn ffprobe_failure_is_reported() {
    let dir = TempDir::new("ffprobe-fails");
    write_script(dir.path(), "ffmpeg", FFMPEG_WRITES_FILE);
    write_script(dir.path(), "ffprobe", FFPROBE_FAILS);
    let output_path = dir.join("out.mp4");

    let output = run(
        dir.path(),
        &["--output", output_path.to_str().expect("utf-8 path")],
    );

    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(
        message.contains("ffprobe could not read"),
        "unexpected: {message}"
    );
    assert!(message.contains("probe-boom"), "unexpected: {message}");
}

#[test]
fn successful_recording_reports_codec_and_duration() {
    let dir = TempDir::new("success");
    write_script(dir.path(), "ffmpeg", FFMPEG_WRITES_FILE);
    write_script(dir.path(), "ffprobe", FFPROBE_SUCCEEDS);
    let output_path = dir.join("out.mp4");

    let output = run(
        dir.path(),
        &[
            "--output",
            output_path.to_str().expect("utf-8 path"),
            "--duration",
            "1",
        ],
    );

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let message = stderr(&output);
    assert!(
        message.contains("recording finished"),
        "unexpected: {message}"
    );
    assert!(message.contains("h264"), "unexpected: {message}");
    assert!(message.contains("1.50 s"), "unexpected: {message}");
    // stdout stays reserved; diagnostics are on stderr.
    assert!(
        output.stdout.is_empty(),
        "unexpected stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
#[ignore = "requires a real X11 display and ffmpeg/ffprobe on PATH"]
fn real_recording_succeeds() {
    let dir = TempDir::new("real");
    let output_path = dir.join("out.mp4");

    let output = Command::new(BIN)
        .args([
            "--output",
            output_path.to_str().expect("utf-8 path"),
            "--duration",
            "1",
        ])
        .output()
        .expect("run rust-recorder");

    assert!(
        output.status.success(),
        "recording failed: {}",
        stderr(&output)
    );
    assert!(stderr(&output).contains("recording finished"));
}
