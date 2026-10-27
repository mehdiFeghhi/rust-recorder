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

const PACTL_OUTPUT: &str = r#"#!/bin/sh
printf '%s\n' \
 'Source #49' \
 '    State: SUSPENDED' \
 '    Name: alsa_output.pci-0000_00_1f.3.analog-stereo.monitor' \
 '    Description: Monitor of Built-in Audio Analog Stereo' \
 'Source #50' \
 '    State: SUSPENDED' \
 '    Name: alsa_input.pci-0000_00_1f.3.analog-stereo' \
 '    Description: Built-in Audio Analog Stereo'
"#;

const XRANDR_OUTPUT: &str = r#"#!/bin/sh
printf '%s\n' \
 'Screen 0: minimum 320 x 200, current 1920 x 1200, maximum 16384 x 16384' \
 'eDP-1 connected primary 1920x1200+0+0 (normal left inverted right x axis y axis) 344mm x 215mm' \
 'DP-1 disconnected (normal left inverted right x axis y axis)'
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
fn lists_connected_monitors() {
    let dir = TempDir::new("monitors");
    write_script(dir.path(), "xrandr", XRANDR_OUTPUT);

    let output = run(dir.path(), &["--list-monitors"]);

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("eDP-1"), "stdout: {stdout}");
    assert!(stdout.contains("Resolution: 1920x1200"), "stdout: {stdout}");
    assert!(stdout.contains("Primary: yes"), "stdout: {stdout}");
    assert!(
        !stdout.contains("[1]"),
        "disconnected outputs must be hidden: {stdout}"
    );
}

#[test]
fn missing_xrandr_is_reported() {
    let dir = TempDir::new("no-xrandr");
    let output = run(dir.path(), &["--list-monitors"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("could not start xrandr"));
}

#[test]
fn records_the_selected_monitor_region() {
    let dir = TempDir::new("monitor-select");
    write_script(dir.path(), "xrandr", XRANDR_OUTPUT);

    let args_file = dir.join("ffmpeg-args.txt");
    let fake_ffmpeg = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nlast=\"\"\nfor argument in \"$@\"; do last=\"$argument\"; done\nprintf 'payload' > \"$last\"\nexit 0\n",
        args_file.display()
    );
    write_script(dir.path(), "ffmpeg", &fake_ffmpeg);
    write_script(dir.path(), "ffprobe", FFPROBE_SUCCEEDS);

    let output_path = dir.join("out.mp4");
    let output = run(
        dir.path(),
        &[
            "--monitor",
            "0",
            "--output",
            output_path.to_str().expect("utf-8 path"),
            "--duration",
            "1",
        ],
    );

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));

    let argv = fs::read_to_string(&args_file).expect("read fake ffmpeg argv");
    assert!(argv.contains("-video_size\n1920x1200\n"), "argv: {argv}");
    assert!(argv.contains("-i\n:1+0,0\n"), "argv: {argv}");
}

#[test]
fn unknown_monitor_is_rejected() {
    let dir = TempDir::new("monitor-unknown");
    write_script(dir.path(), "xrandr", XRANDR_OUTPUT);
    let output_path = dir.join("out.mp4");

    let output = run(
        dir.path(),
        &[
            "--monitor",
            "3",
            "--output",
            output_path.to_str().expect("utf-8 path"),
            "--duration",
            "1",
        ],
    );

    assert_eq!(output.status.code(), Some(2), "stderr: {}", stderr(&output));
    assert!(stderr(&output).contains("does not exist"));
}

#[test]
fn region_outside_the_desktop_is_rejected() {
    let dir = TempDir::new("region-bounds");
    write_script(dir.path(), "xrandr", XRANDR_OUTPUT);
    let output_path = dir.join("out.mp4");

    let output = run(
        dir.path(),
        &[
            "--region",
            "3000x3000+0+0",
            "--output",
            output_path.to_str().expect("utf-8 path"),
            "--duration",
            "1",
        ],
    );

    assert_eq!(output.status.code(), Some(2), "stderr: {}", stderr(&output));
    assert!(stderr(&output).contains("does not fit"));
}

#[test]
fn region_inside_the_desktop_is_recorded() {
    let dir = TempDir::new("region-ok");
    write_script(dir.path(), "xrandr", XRANDR_OUTPUT);
    write_script(dir.path(), "ffmpeg", FFMPEG_WRITES_FILE);
    write_script(dir.path(), "ffprobe", FFPROBE_SUCCEEDS);

    let output_path = dir.join("out.mp4");
    let output = run(
        dir.path(),
        &[
            "--region",
            "640x480+100+80",
            "--output",
            output_path.to_str().expect("utf-8 path"),
            "--duration",
            "1",
        ],
    );

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert!(stderr(&output).contains("recording finished"));
}

#[test]
fn interactive_selection_passes_the_flag() {
    let dir = TempDir::new("select-region");
    let args_file = dir.join("ffmpeg-args.txt");
    let fake_ffmpeg = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nlast=\"\"\nfor argument in \"$@\"; do last=\"$argument\"; done\nprintf 'payload' > \"$last\"\nexit 0\n",
        args_file.display()
    );
    write_script(dir.path(), "ffmpeg", &fake_ffmpeg);
    write_script(dir.path(), "ffprobe", FFPROBE_SUCCEEDS);

    let output_path = dir.join("out.mp4");
    let output = run(
        dir.path(),
        &[
            "--select-region",
            "--output",
            output_path.to_str().expect("utf-8 path"),
            "--duration",
            "1",
        ],
    );

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));

    let argv = fs::read_to_string(&args_file).expect("read fake ffmpeg argv");
    assert!(argv.contains("-select_region\n1\n"), "argv: {argv}");
    assert!(!argv.contains("-video_size"), "argv: {argv}");
}

#[test]
fn lists_audio_devices() {
    let dir = TempDir::new("audio-devices");
    write_script(dir.path(), "pactl", PACTL_OUTPUT);

    let output = run(dir.path(), &["--list-audio-devices"]);

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Monitor of Built-in Audio Analog Stereo"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("Type: system audio"), "stdout: {stdout}");
    assert!(stdout.contains("Type: microphone"), "stdout: {stdout}");
    assert!(
        stdout.contains("alsa_input.pci-0000_00_1f.3.analog-stereo"),
        "stdout: {stdout}"
    );
}

#[test]
fn missing_pactl_is_reported() {
    let dir = TempDir::new("no-pactl");
    let output = run(dir.path(), &["--list-audio-devices"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("could not start pactl"));
}

#[test]
fn records_with_the_requested_microphone() {
    let dir = TempDir::new("mic");
    write_script(dir.path(), "pactl", PACTL_OUTPUT);

    let args_file = dir.join("ffmpeg-args.txt");
    let fake_ffmpeg = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nlast=\"\"\nfor argument in \"$@\"; do last=\"$argument\"; done\nprintf 'payload' > \"$last\"\nexit 0\n",
        args_file.display()
    );
    write_script(dir.path(), "ffmpeg", &fake_ffmpeg);
    write_script(dir.path(), "ffprobe", FFPROBE_SUCCEEDS);

    let output_path = dir.join("out.mp4");
    let output = run(
        dir.path(),
        &[
            "--audio",
            "mic",
            "--microphone-device",
            "alsa_input.pci-0000_00_1f.3.analog-stereo",
            "--output",
            output_path.to_str().expect("utf-8 path"),
            "--duration",
            "1",
        ],
    );

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));

    let argv = fs::read_to_string(&args_file).expect("read fake ffmpeg argv");
    let lines: Vec<&str> = argv.lines().collect();
    let has = |flag: &str, value: &str| {
        lines
            .windows(2)
            .any(|pair| pair[0] == flag && pair[1] == value)
    };

    assert!(has("-f", "pulse"), "argv: {argv}");
    assert!(
        has("-i", "alsa_input.pci-0000_00_1f.3.analog-stereo"),
        "argv: {argv}"
    );
    assert!(has("-map", "1:a:0"), "argv: {argv}");
}

#[test]
fn unknown_microphone_is_rejected() {
    let dir = TempDir::new("mic-unknown");
    write_script(dir.path(), "pactl", PACTL_OUTPUT);

    let output_path = dir.join("out.mp4");
    let output = run(
        dir.path(),
        &[
            "--audio",
            "mic",
            "--microphone-device",
            "missing_input",
            "--output",
            output_path.to_str().expect("utf-8 path"),
            "--duration",
            "1",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(stderr(&output).contains("was not found"));
}

#[test]
fn defaults_to_the_first_microphone() {
    let dir = TempDir::new("mic-default");
    write_script(dir.path(), "pactl", PACTL_OUTPUT);

    let args_file = dir.join("ffmpeg-args.txt");
    let fake_ffmpeg = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nlast=\"\"\nfor argument in \"$@\"; do last=\"$argument\"; done\nprintf 'payload' > \"$last\"\nexit 0\n",
        args_file.display()
    );
    write_script(dir.path(), "ffmpeg", &fake_ffmpeg);
    write_script(dir.path(), "ffprobe", FFPROBE_SUCCEEDS);

    let output_path = dir.join("out.mp4");
    let output = run(
        dir.path(),
        &[
            "--audio",
            "mic",
            "--output",
            output_path.to_str().expect("utf-8 path"),
            "--duration",
            "1",
        ],
    );

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));

    let argv = fs::read_to_string(&args_file).expect("read fake ffmpeg argv");
    let lines: Vec<&str> = argv.lines().collect();
    assert!(
        lines.windows(2).any(|pair| {
            pair[0] == "-i" && pair[1] == "alsa_input.pci-0000_00_1f.3.analog-stereo"
        }),
        "argv: {argv}"
    );
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
