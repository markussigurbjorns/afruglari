# Drone Garden Project Goal

## Working Target

Build a seedable ambient/experimental Rust instrument called **Drone Garden**.

The first useful version should run as a realtime stereo audio program, producing a slowly evolving drone piece from a small set of synthesis voices. The output should be listenable immediately, but the internal structure should leave room for granular clouds, sample loops, feedback systems, and recording later.

## V1 Status

V1 is complete.

Run it with:

```text
cargo run -- [OPTIONS]
```

Current defaults:

- `--seed 12101854522218779061`
- `--root 110.0`
- `--voices 3`
- `--duration`: run until interrupted
- `--density 0.35`
- `--brightness 0.45`
- `--space 0.65`
- `--instability 0.25`
- `--drone 1.0`
- `--noise 0.0`
- `--events 0.0`
- `--texture 0.0`
- `--output`: realtime playback

## Recommended Starting Commands

Realtime listening:

```text
cargo run -- --drone 0.8 --noise 0.2 --events 0.4 --texture 0.5 --space 0.8
cargo run -- --root 82.41 --voices 5 --density 0.6 --instability 0.45 --noise 0.3 --texture 0.6
```

Offline renders:

```text
cargo run -- --duration 300 --output renders/tape-memory.wav --drone 0.8 --noise 0.25 --events 0.5 --texture 0.6 --space 0.85
cargo run -- --duration 600 --output renders/dark-field.wav --root 82.41 --voices 5 --brightness 0.25 --space 0.9 --noise 0.4 --texture 0.7
```

## V2 Target

V2 is complete.

Turn Drone Garden from one drone patch into a layered ambient instrument with macro controls.

V2 should keep the v1 realtime output path, pitch-field tuning, seeded generation, and filtered delay, while adding multiple independently evolving sound layers:

- drone layer: existing tonal bed
- noise layer: filtered air, dust, wind, tape hiss, breath
- event layer: sparse tuned bells, plucks, clicks, or resonant tones

Macro controls should shape the whole system without exposing every tiny DSP parameter.

```rust
pub struct GardenControls {
    pub density: f32,
    pub brightness: f32,
    pub space: f32,
    pub instability: f32,
    pub drone_level: f32,
    pub noise_level: f32,
    pub event_level: f32,
    pub texture_level: f32,
}
```

Default macro values:

- `density: 0.35`
- `brightness: 0.45`
- `space: 0.65`
- `instability: 0.25`
- `drone_level: 1.0`
- `noise_level: 0.0`
- `event_level: 0.0`
- `texture_level: 0.0`

Intended mappings:

- `density`: event probability, active voice count, layer activity
- `brightness`: noise cutoff, delay feedback filter cutoff, event sharpness
- `space`: delay wet amount, feedback, stereo width
- `instability`: detune amount, retune probability, pan drift
- `drone_level`: tonal bed level
- `noise_level`: filtered noise layer level
- `event_level`: sparse foreground event level
- `texture_level`: tape-memory layer level

V2 implementation order:

1. Add `GardenControls` with default macro values. Done.
2. Refactor current drone behavior into a drone layer. Done.
3. Add a filtered noise layer. Done.
4. Add CLI flags for macro controls and layer levels. Done.
5. Add a sparse pitch-field event layer. Done.
6. Map `brightness` and `space` into the delay. Done.

Example commands:

```text
cargo run -- --duration 120 --drone 0.8 --noise 0.25 --events 0.5 --density 0.55 --space 0.8
cargo run -- --root 82.41 --voices 5 --noise 0.4 --events 0.7 --brightness 0.35 --instability 0.5
cargo run -- --seed 42 --duration 300 --density 0.8 --space 0.95 --brightness 0.2
```

## V3 Target

Make Drone Garden render itself.

V3 should add offline WAV output so generated pieces can be captured without external audio routing or a realtime audio device.
Presets are intentionally out of scope for now; the preferred workflow is direct tuning through CLI controls, with each render saving a sidecar metadata file for recall.

Run an offline render with:

```text
cargo run -- --duration 300 --output renders/piece.wav
```

If `--output` is provided without `--duration`, the renderer uses 60 seconds.
Each render writes a `.txt` metadata sidecar beside the WAV with the full config and a reproduce command.
Offline rendering prints 10% progress updates and a final summary.

V3 implementation order:

1. Add `hound` dependency. Done.
2. Add `--output PATH` CLI flag. Done.
3. Add offline WAV renderer using the same `Garden` source path. Done.
4. Write render metadata sidecars. Done.
5. Add render progress output and final render summary. Done.
6. Document render examples.

## V4 Target

Add a tape-memory texture layer.

V4 starts a richer sound-design phase by adding a `TextureLayer` that records the generated dry mix into a circular stereo buffer and reads delayed, drifting taps back into the shared delay path.

Run with tape memory enabled:

```text
cargo run -- --texture 0.5 --space 0.8 --noise 0.2 --events 0.4
```

Example texture renders:

```text
cargo run -- --duration 180 --output renders/texture-soft.wav --texture 0.45 --space 0.75 --noise 0.2 --events 0.35
cargo run -- --duration 300 --output renders/texture-wide.wav --texture 0.7 --space 0.95 --brightness 0.35 --instability 0.5
cargo run -- --duration 420 --output renders/texture-low-root.wav --root 73.42 --voices 6 --texture 0.65 --noise 0.35 --events 0.45
```

V4 implementation order:

1. Add `--texture` macro control. Done.
2. Add circular stereo tape buffer. Done.
3. Add delayed drifting read taps. Done.
4. Feed texture into the shared delay path. Done.
5. Shorten the first texture tap for earlier audible memory. Done.
6. Add conservative texture feedback into the tape buffer. Done.

## V5 Target

Add explicit macro automation timelines.

V5 should not add presets or named arcs. The goal is direct composition control: define exactly what macro values should be active at specific times, then let the engine interpolate between those points.

Proposed timeline format:

```text
0 root=110 voices=3 octave_min=1 octave_max=2 event_attack_min=0.02 event_attack_max=0.12 event_decay_min=2 event_decay_max=6 drone_retune_seconds=12 density=0.2 brightness=0.4 space=0.6 drone=1.0 noise=0.1 events=0.0 texture=0.0
120 root=82.41 voices=5 octave_min=2 octave_max=3 event_attack_min=0.08 event_attack_max=0.30 event_decay_min=4 event_decay_max=10 drone_retune_seconds=5 density=0.5 brightness=0.35 space=0.8 noise=0.3 events=0.4 texture=0.5
300 root=55 voices=2 octave_min=0 octave_max=1 event_attack_min=0.20 event_attack_max=0.60 event_decay_min=8 event_decay_max=14 drone_retune_seconds=20 density=0.25 brightness=0.2 space=0.95 drone=0.4 noise=0.1 events=0.0 texture=0.9
```

Proposed usage:

```text
cargo run -- --duration 420 --timeline timeline.txt --output renders/piece.wav
```

First timeline piece in the repo:

```text
cargo run -- --duration 720 --timeline timelines/first-piece.timeline --output renders/first-piece.wav
```

Timeline behavior:

- each non-empty line starts with seconds
- remaining tokens are `key=value` macro assignments
- omitted controls keep the previous value
- macro controls and `root` are linearly interpolated between time points
- `voices` is stepped and holds its previous value until the next point
- `octave_min` and `octave_max` are stepped and set the active register range
- `event_attack_min` and `event_attack_max` are stepped and shape event onset
- `event_decay_min` and `event_decay_max` are stepped and shape event ring length
- `drone_retune_seconds` is stepped and controls how often the drone layer retunes
- render sidecars should include the timeline path and copied timeline contents

V5 implementation order:

1. Make `GardenControls` updateable after construction. Done.
2. Add layer `set_controls` methods. Done.
3. Add mutable delay parameter setters. Done.
4. Add timeline data structures and interpolation. Done.
5. Add timeline text parser. Done.
6. Add `--timeline PATH` CLI flag and load/parse timeline files into app config. Done.
7. Apply timeline controls during realtime playback and offline rendering. Done.
8. Include timeline data in render sidecars. Done.

Timeline writing notes:

- write pieces as explicit control points in `timelines/*.timeline`
- keep them readable and hand-editable
- let omitted controls carry forward so each line only changes what matters
- prefer a few strong section changes over constant micromanagement

## MVP Experience

Running the program should:

- open the default audio output device with `cpal`
- generate continuous stereo audio
- create 2-4 active drone voices from a larger voice pool
- tune voices from a simple pitch field
- drift pitch, amplitude, and pan slowly over time
- pass the mix through spacious delay/reverb-style processing
- keep the piece evolving without abrupt parameter jumps

The initial program can run until interrupted.

## Musical Shape

The first sound world should be sparse, slow, and unstable in small ways.

Core traits:

- long sine or soft harmonic drones
- just-intonation pitch ratios
- very slow fades
- slight detuning and pitch wandering
- gentle stereo movement
- feedback ambience
- occasional retuning or voice replacement

Avoid building a full sequencer first. The system should feel more like an ecosystem than a timeline.

## Architecture

Keep the realtime audio path simple and allocation-light.

Suggested modules:

```text
src/
  main.rs
  audio/
    mod.rs
    engine.rs
  composition/
    mod.rs
    garden.rs
    pitch.rs
  dsp/
    mod.rs
    delay.rs
    random.rs
    voice.rs
```

Early responsibilities:

- `audio::engine`: own `cpal` setup and output stream
- `composition::garden`: own high-level evolving state
- `composition::pitch`: pitch fields and frequency selection
- `dsp::voice`: drone oscillator voice state
- `dsp::random`: seeded random walks and smoothed values
- `dsp::delay`: first spacious feedback effect

## Data Model Sketch

```rust
struct Garden {
    voices: Vec<DroneVoice>,
    pitch_field: PitchField,
    delay: StereoDelay,
    rng: StdRng,
}

struct DroneVoice {
    frequency_hz: SmoothedValue,
    amplitude: SmoothedValue,
    pan: SmoothedValue,
    phase: f32,
}

struct PitchField {
    root_hz: f32,
    ratios: Vec<f32>,
}
```

## Implementation Order

1. Replace the hello-world binary with a `cpal` output stream.
2. Generate a quiet stereo sine drone.
3. Add `DroneVoice` with phase, frequency, amplitude, and pan.
4. Add multiple voices mixed safely below clipping.
5. Add slow smoothed parameter targets.
6. Add a seeded random source.
7. Add a pitch field using frequency ratios.
8. Add slow retuning and fade behavior.
9. Add a simple stereo feedback delay.
10. Add CLI options for seed/root/duration once the core loop works.

## Constraints

- Prefer clear hand-written DSP until a real need for a graph abstraction appears.
- Use `fundsp` where it makes experimentation easier, but do not force it into the first version if direct sample generation is simpler.
- Keep audio-thread work predictable: no file I/O, no logging per sample, no unbounded allocation.
- Use `f32` audio internally unless there is a specific reason not to.
- Start with realtime playback before offline rendering.

## Later Ideas

- WAV recording with `hound`
- granular sample clouds
- circular tape buffers
- freeze delay/reverb
- noise through resonator banks
- scene transitions
- small `egui` control surface
- saved presets and reproducible seeds
