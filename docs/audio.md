# Audio subsystem design (Phase 4)

Status: design for M4.1. No audio code ships in this milestone.

This document defines the audio model, discovery strategy, CLI contract, FFmpeg
integration, validation rules, and testing plan. It is deliberately written
before code so the later milestones share one vocabulary.

## Scope

In scope:

- Discover audio capture sources exposed by PulseAudio-compatible systems
  (PulseAudio, and PipeWire's PulseAudio compatibility layer).
- Distinguish **microphones** (sound entering the computer) from **output
  monitors** (sound the computer is playing).
- Let the user list sources and choose them by stable identifier.
- Record video with `none`, `mic`, `system`, or `both` audio.
- Mix microphone and system audio into one track (first audio release).

Out of scope for Phase 4:

- Separate audio tracks per source (later; useful for post-production).
- Acoustic capture through speakers or headphones (we capture the digital
  stream routed to an output).
- Bluetooth-specific quirks beyond what PulseAudio exposes.
- GUI, Wayland, streaming.
- Per-source volume control beyond FFmpeg defaults.

## Modes

```rust
pub enum AudioMode {
    None,         // default; preserves current behaviour
    Microphone,   // one microphone source
    System,       // one output-monitor source
    Both,         // microphone + output monitor, mixed
}
```

Rules:

- `None` is the default. Audio is never recorded accidentally.
- `Both` requires both a microphone and a monitor source; it is an error if
  either cannot be found.
- Mode is validated before FFmpeg starts; no partial fallback (never silently
  record `mic` when `both` was requested).

## Sources and metadata

```rust
pub enum AudioSourceKind {
    Microphone,
    SystemOutput,
}

pub struct AudioSource {
    /// PulseAudio source name, e.g. `alsa_input.pci-…`.
    pub id: String,
    /// Human-readable description, e.g. "Built-in Audio Analog Stereo".
    pub description: String,
    pub kind: AudioSourceKind,
    pub is_default: bool,
}
```

- **`id` is the stable identifier** used on the command line and in FFmpeg
  arguments. Indices change when devices reconnect; names are far more stable,
  though not guaranteed forever.
- `description` comes from `pactl list sources` when available; otherwise it
  falls back to `id`.
- Classification: a source whose name ends in `.monitor` is a `SystemOutput`;
  everything else is a `Microphone`. This is the PulseAudio convention and is
  also used by PipeWire's compatibility layer.

## Discovery

Commands:

```bash
pactl list short sources   # index, name, driver, sample spec, state
pactl get-default-source   # may be unsupported on older versions
```

- Discovery must not require audio playback or a running recording.
- If `pactl` is missing, return an actionable error naming the package
  (`pulseaudio-utils` / `pipewire-pulse`).
- If it exits non-zero, surface its stderr.
- Parse defensively: skip malformed lines, keep valid sources, and never panic.
- `get-default-source` failure is non-fatal; fall back to the first
  microphone / first monitor, and report which one was chosen.

Example (illustrative, not from this machine):

```text
Audio sources:

[0] Microphone
    Type: microphone
    ID: alsa_input.pci-0000_00_1f.3.analog-stereo

[1] Built-in Audio Output Monitor
    Type: system audio
    ID: alsa_output.pci-0000_00_1f.3.analog-stereo.monitor
```

## CLI contract

```text
--audio <none|mic|system|both>     default: none
--list-audio-devices              list sources and exit
--microphone-device <ID>          source name for microphone mode
--system-audio-device <ID>        source name for system-audio mode
```

Rules:

- `--microphone-device`/`--system-audio-device` require the matching mode
  (`mic`/`both` and `system`/`both`); otherwise reject as a usage error.
- Unknown device IDs are rejected before FFmpeg starts, with the available IDs
  in the message.
- `--list-audio-devices` needs no `--output` and does not start a recording.
- Device identifiers are never replaced silently; if the requested source is
  gone, fail with a clear message.

## FFmpeg integration

Inputs (order matters):

```text
0: x11grab video
1: pulse microphone (if mode needs it)
2: pulse system monitor (if mode needs it)
```

Examples:

```bash
# microphone
ffmpeg … -i :1 -f pulse -i alsa_input… -c:v libx264 -pix_fmt yuv420p \
       -map 0:v:0 -map 1:a:0 -c:a aac -b:a 192k …

# both, mixed
ffmpeg … -i :1 -f pulse -i MIC -f pulse -i MONITOR \
       -filter_complex "[1:a][2:a]amix=inputs=2:duration=longest:dropout_transition=0[a]" \
       -map 0:v:0 -map "[a]" -c:a aac -b:a 192k …
```

Synchronisation:

- Both audio inputs and the video input run on the same wall clock; FFmpeg
  timestamps all streams.
- Resample audio to 48 kHz stereo (`-ar 48000 -ac 2`) for predictable mixing.
- Consider `aresample=async=1:first_pts=0` if drift is observed in long
  recordings; do not add it until measured.
- `amix` with `duration=longest` avoids truncating one source; the recording
  still stops via `-t` or Ctrl-C.

Containers:

- MP4: AAC audio (`-c:a aac`), broadly compatible.
- MKV: AAC is fine; Opus could be considered later.

## Validation and failure modes

| Situation | Behaviour |
| --- | --- |
| No audio requested | No audio inputs or mapping; current behaviour unchanged |
| `pactl` missing | Exit 1 with an install hint |
| No monitor sources | `system`/`both` exit 2 with an explanatory message |
| Requested ID not found | Exit 2, list available IDs |
| Device disappears mid-recording | FFmpeg reports the failure; exit 1 |
| Microphone permission denied | Surface FFmpeg/PulseAudio stderr |
| Duplicate mic/monitor ID | Allowed but worth a warning; mixed with itself is rarely intended |

## Privacy

- Never log audio samples or device content.
- Device names and IDs are logged; they can contain hardware model names, which
  is acceptable for diagnostics.
- Audio is off by default, so the tool cannot record conversations by accident.

## Testing strategy

- **Unit:** parse representative `pactl list short sources` output — mic only,
  monitor only, both, empty, malformed lines, unusual names.
- **Unit:** classification (`.monitor` suffix), mode parsing, device/mode
  consistency errors.
- **Unit:** command construction for each mode (input order, mapping, mixing).
- **Integration:** fake `pactl` script on `PATH` + fake `ffmpeg`/`ffprobe`, so
  CI never touches audio hardware.
- **Manual:** record 5 s with a microphone, with system audio, and with both;
  confirm with `ffprobe -show_streams` that the expected streams exist and that
  audio/video stay aligned.
- **CI must never depend on a physical microphone or a running sound server.**

## Milestone mapping

| Milestone | Deliverable |
| --- | --- |
| M4.1 | This document |
| M4.2 | `pactl` discovery + `AudioSource` parsing |
| M4.3 | Classification (microphone vs output monitor) |
| M4.4 | `--list-audio-devices` |
| M4.5 | Explicit source selection and ID validation |
| M5.1 | `--audio` mode parsing and validation |
| M5.2 | Microphone capture |
| M5.3 | System-audio capture |
| M5.4 | Mixed microphone + system audio |
| M5.5 | Audio-aware `ffprobe` verification |
| M5.6 | Integration tests for missing/selected devices |

## Open questions

1. Prefer `pactl get-default-source`, or always require an explicit ID?
   (Proposal: default, but print which source was chosen.)
2. Should `both` use `amix` normalization (`normalize=1`) or preserve levels?
   (Proposal: start with FFmpeg's default and document it.)
3. PipeWire native API vs PulseAudio compatibility: keep PulseAudio for now;
   revisit only if compatibility proves insufficient.
4. Separate tracks: defer to Phase 6, after mixed audio is proven.
