# Composing Pieces

Afruglari generates experimental audio by solving a constraint problem, then rendering the solved event grid into sound.

The important idea is:

```text
constraints shape structure
render modes turn structure into sound
```

You are not writing notes. You are describing density, silence, collisions, repetition, asymmetry, and timbral behavior.

## Quick Start

Render an existing config:

```bash
cargo run -- --config pieces/long-noise-field.toml
```

Render a preset directly:

```bash
cargo run -- sparse-cracks 42 target/sparse-42.wav broken-radio
```

Batch-render a preset:

```bash
cargo run -- --batch sparse-cracks 8 target/renders/sparse broken-radio
```

Scan rendered metadata:

```bash
cargo run -- --scan target/renders/sparse
```

Every render writes:

```text
output.wav
output.json
```

The JSON metadata includes event count, collisions, and per-voice density.

## Piece Files

Piece files use a small TOML-style format.

Minimal custom piece:

```toml
[piece]
voices = 3
steps = 128
registers = 5
timbres = 8
intensities = 6
seed = 23
output = "target/pieces/my-piece.wav"
render_mode = "noise-organ"

[render]
sample_rate = 44100
step_seconds = 0.22
tail_seconds = 2.5
stereo_width = 0.8
delay_mix = 0.15
delay_feedback = 0.30
delay_seconds = 0.37
drive = 1.2
brightness = 1.0
roughness = 1.2
sustain = 1.4

[[constraint]]
type = "exact-count"
param = "active"
value = true
density = 0.24
```

Render it with:

```bash
cargo run -- --config path/to/piece.toml
```

## Duration

Duration is approximately:

```text
steps * step_seconds + tail_seconds
```

Examples:

```text
64 steps  * 0.20 seconds + 2.0 tail = 14.8 seconds
128 steps * 0.22 seconds + 2.5 tail = 30.66 seconds
240 steps * 0.24 seconds + 3.0 tail = 60.6 seconds
```

Longer pieces usually need more steps, not just longer tails.

## Density

The event grid has:

```text
voices * steps
```

possible active slots.

For 3 voices and 128 steps:

```text
3 * 128 = 384 slots
```

Use `density` to avoid manually calculating event counts:

```toml
[[constraint]]
type = "exact-count"
param = "active"
value = true
density = 0.24
```

This produces:

```text
round(384 * 0.24) = 92 active events
```

Useful density ranges:

```text
0.05 - 0.15  sparse, cracked, pointillistic
0.20 - 0.35  active but open
0.40 - 0.60  dense, collision-heavy
0.70+        saturated; may need looser constraints
```

You can also use a fixed count:

```toml
[[constraint]]
type = "exact-count"
param = "active"
value = true
count = 38
```

Prefer `density` for long pieces.

## Render Modes

Available render modes:

```text
percussive
drone
broken-radio
metallic
noise-organ
granular-dust
sub-machine
glass-harmonics
```

General character:

```text
percussive     short FM pulses, metallic hits, noisy clouds
drone          longer overlapping tones and unstable resonance
broken-radio   gated noise, crushed signal, unstable synthetic radio
metallic       inharmonic struck partials
noise-organ    banded noisy tones with organ-like overlap
granular-dust  tiny noisy grains and unstable spark bursts
sub-machine    low pulse trains, thumps, and mechanical wobble
glass-harmonics brittle ringing partials with sharp glassy decay
```

Set the mode in `[piece]`:

```toml
render_mode = "noise-organ"
```

Render polish lives in `[render]`:

```toml
[render]
stereo_width = 0.8
delay_mix = 0.15
delay_feedback = 0.30
delay_seconds = 0.37
drive = 1.2
brightness = 1.0
roughness = 1.2
sustain = 1.4
```

Field meanings:

```text
stereo_width     0.0 is mono-ish, 1.0 is wide voice panning
delay_mix        how much cross-channel delay is added
delay_feedback   how strongly delayed material feeds forward
delay_seconds    delay time
drive            tanh saturation amount before writing the WAV
brightness       shifts resonators and carriers upward or downward
roughness        increases noise, modulation, folding, and gating
sustain          stretches event tails and envelope decay
```

You can override the event-level render settings inside named sections:

```toml
[[section]]
name = "opening"
start = 0
end = 48

[[section]]
name = "rupture"
start = 48
end = 96

[[section_render]]
section = "opening"
brightness = 0.65
roughness = 0.75
sustain = 2.40
stereo_width = 0.60

[[section_render]]
section = "rupture"
mode = "broken-radio"
brightness = 1.75
roughness = 2.45
sustain = 0.55
stereo_width = 1.00
drive = 1.70
```

`[[section_render]]` changes how events in that step range are synthesized. It currently affects `mode`, `stereo_width`, `drive`, `brightness`, `roughness`, and `sustain`; timing and delay remain global in `[render]`.

You can also give each voice its own sound role:

```toml
[[voice_render]]
voice = 0
preset = "buried-engine"

[[voice_render]]
voice = 1
preset = "glass-insects"

[[voice_render]]
voice = 2
preset = "static-ash"
roughness = 2.40
```

Available presets:

```text
buried-engine
glass-insects
static-ash
radio-wound
organ-fog
metal-splinters
low-ritual
distant-drone
```

Preset fields can be overridden locally. `preset = "static-ash"` plus `roughness = 2.40` keeps the preset sound but makes it harsher.

Render settings are layered in this order:

```text
[render] -> [[voice_render]] -> [[section_render]]
```

That means voice mappings define the normal sound identity of each layer, and section automation can still force a change of state across the whole form.

Batch a composed config across several seeds with:

```bash
cargo run -- --batch-config pieces/ritual-machines.toml 8 target/renders/ritual-machines
```

Each render writes a WAV and matching JSON metadata. The metadata includes event counts, collisions, voice density, and summaries of `[[voice_render]]` and `[[section_render]]` mappings so scan output can distinguish structural results from sound-design choices.

## Parameters

Each voice and step has these parameters:

```text
active     bool; whether an event happens
register   small integer; pitch/frequency region
timbre     small integer; synthesis color
intensity  small integer; loudness/energy
```

The renderer interprets these differently depending on render mode.

## Constraints

### Max Run

Limits consecutive values in one voice.

Most often used to prevent a voice from staying active too long:

```toml
[[constraint]]
type = "max-run"
voice = 0
param = "active"
len = 3
```

Meaning: voice 0 cannot have more than 3 active steps in a row.

### Exact Count

Sets an exact number, or density, of a value.

Global active density:

```toml
[[constraint]]
type = "exact-count"
param = "active"
value = true
density = 0.24
```

Per-voice active count:

```toml
[[constraint]]
type = "exact-count"
voice = 2
param = "active"
value = true
count = 40
```

Rare timbre:

```toml
[[constraint]]
type = "exact-count"
voice = 1
param = "timbre"
value = 8
count = 5
```

### Min Density Window

Prevents long empty stretches.

```toml
[[constraint]]
type = "min-density-window"
param = "active"
window = 18
min = 1
```

The global active scope is ordered by time across voices. With 3 voices, `window = 18` means roughly 6 time steps.

### At Least Collisions

Forces two voices to be active at the same step a minimum number of times.

```toml
[[constraint]]
type = "at-least-collisions"
voice_a = 1
voice_b = 2
count = 8
```

Collisions are useful for generating structural tension.

### Slow Change

Forces a parameter to remain constant in blocks.

```toml
[[constraint]]
type = "slow-change"
voice = 2
param = "timbre"
window = 8
```

Meaning: voice 2 changes timbre only every 8 steps.

This is useful for long-form shape.

### More True Than

Makes one voice or region denser than another.

```toml
[[constraint]]
type = "more-true-than"
voice_a = 2
voice_b = 0
param = "active"
```

Meaning: voice 2 must have more active events than voice 0.

## A 30-Second Template

```toml
[piece]
voices = 3
steps = 128
registers = 5
timbres = 8
intensities = 6
seed = 23
output = "target/pieces/long-noise-field.wav"
render_mode = "noise-organ"

[render]
sample_rate = 44100
step_seconds = 0.22
tail_seconds = 2.5
stereo_width = 0.8
delay_mix = 0.16
delay_feedback = 0.30
delay_seconds = 0.41
drive = 1.15
brightness = 0.75
roughness = 1.60
sustain = 2.20

[[constraint]]
type = "max-run"
voice = 0
param = "active"
len = 4

[[constraint]]
type = "max-run"
voice = 1
param = "active"
len = 5

[[constraint]]
type = "exact-count"
param = "active"
value = true
density = 0.24

[[constraint]]
type = "min-density-window"
param = "active"
window = 18
min = 1

[[constraint]]
type = "at-least-collisions"
voice_a = 1
voice_b = 2
count = 8

[[constraint]]
type = "slow-change"
voice = 2
param = "timbre"
window = 8

[[constraint]]
type = "slow-change"
voice = 0
param = "intensity"
window = 8
```

## Compositional Workflow

1. Pick a duration.

   Choose `steps` and `step_seconds`.

2. Pick a density.

   Start with `density = 0.20` to `0.30` for long pieces.

3. Add silence control.

   Use `min-density-window` to prevent dead spans.

4. Add collision behavior.

   Use `at-least-collisions` to force voices to interfere.

5. Add slow parameter changes.

   Use `slow-change` on `timbre`, `register`, or `intensity`.

6. Render several seeds.

   Change only `seed` first. If the family is not interesting, change constraints.

7. Scan the batch.

   Use metadata filters to find pieces worth auditioning:

   ```bash
   cargo run -- --scan target/renders/sparse --min-collisions 5
   cargo run -- --scan target/renders/sparse --voice-dominates 2
   cargo run -- --scan target/renders/sparse --min-events 20 --max-events 40
   ```

## Scanning Batches

The scanner reads generated `.json` metadata files recursively:

```bash
cargo run -- --scan target/renders/sparse
```

Output:

```text
metadata                                     events collisions voice_density    output
target/renders/sparse/sparse-cracks-000.json     20          3 [9, 8, 3]        target/renders/sparse/sparse-cracks-000.wav
```

Supported filters:

```text
--min-collisions N
--max-collisions N
--min-events N
--max-events N
--voice-dominates VOICE
```

Examples:

```bash
cargo run -- --scan target/renders/sparse --min-collisions 5
cargo run -- --scan target/renders/sparse --voice-dominates 2
cargo run -- --scan target/renders/sparse --min-events 20 --max-events 40
```

`--voice-dominates 2` means voice 2 has strictly more events than every other voice.

## Additional Constraints

### Min Count and Max Count

Use these when exact counts are too rigid.

```toml
[[constraint]]
type = "min-count"
voice = 2
param = "active"
value = true
count = 30
```

```toml
[[constraint]]
type = "max-count"
voice = 0
param = "active"
value = true
count = 18
```

Both also support `density`:

```toml
[[constraint]]
type = "max-count"
param = "active"
value = true
density = 0.45
```

### Different Adjacent

Prevents direct repetition in one voice.

```toml
[[constraint]]
type = "different-adjacent"
voice = 1
param = "timbre"
```

This is useful for unstable timbral motion.

### Anti Repeat Window

Limits how often the same value can appear inside a moving window.

```toml
[[constraint]]
type = "anti-repeat-window"
voice = 1
param = "timbre"
window = 4
max_repeats = 2
```

Meaning: in any 4-step window, no timbre value can appear more than twice.

### Phase Response

Makes one voice answer another after an offset.

```toml
[[constraint]]
type = "phase-response"
voice_a = 0
voice_b = 2
offset = 3
min = 6
```

Meaning: at least 6 times, voice 2 is active 3 steps after voice 0 is active.

## Sections

Longer pieces can be divided into named time ranges:

```toml
[[section]]
name = "opening"
start = 0
end = 32

[[section]]
name = "body"
start = 32
end = 72

[[section]]
name = "rupture"
start = 72
end = 96
```

Then add `section = "name"` to a constraint:

```toml
[[constraint]]
type = "exact-count"
section = "opening"
param = "active"
value = true
density = 0.18
```

```toml
[[constraint]]
type = "phase-response"
section = "body"
voice_a = 0
voice_b = 2
offset = 4
min = 6
```

```toml
[[constraint]]
type = "anti-repeat-window"
section = "rupture"
voice = 1
param = "timbre"
window = 4
max_repeats = 2
```

Sections use half-open step ranges:

```text
start = 32
end = 72
```

This means steps `32` through `71`.

You can also use direct ranges on a constraint:

```toml
[[constraint]]
type = "exact-count"
start = 16
end = 48
param = "active"
value = true
density = 0.30
```

## Troubleshooting

### The solver is slow

Try:

```text
lower density
use fewer steps
loosen max-run constraints
lower collision count
avoid density near 0.50 on large grids
```

Exact counts near the middle of the search space can be harder than sparse or very dense counts.

### No solution

Your constraints contradict each other.

Common causes:

```text
exact-count is too high with max-run limits
collision count is too high
min-density-window is too strict
per-voice counts do not add up with global counts
```

### The piece is too empty

Increase:

```text
density
min-density-window min
collision count
```

Or use a denser render mode such as `drone` or `noise-organ`.

### The piece is too busy

Decrease:

```text
density
collision count
step_seconds
```

Or add stricter `max-run` constraints.

## Current Limitations

The config format is intentionally small. It currently supports only the constraints listed above. More compositional constraints can be added later, such as anti-repeat windows, phase responses, value palettes, and arcs.
