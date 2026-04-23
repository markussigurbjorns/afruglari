# Phase 1 Refactor Plan

## Scope

Phase 1 is the smallest refactor that introduces a reusable tuning model
without changing the external behavior of Drone Garden.

This phase should not add:

- new instruments
- sample playback
- pulse generation
- new CLI flags
- a new timeline syntax

This phase should only do enough work to make later instrument work clean.

## Phase 1 Goal

Move tuning and register behavior out of scattered layer-specific fields and
into shared composition types that can later be reused by multiple
instruments.

At the end of Phase 1:

- the current garden should still sound the same by default
- `DroneLayer` and `EventLayer` should use the same tuning abstractions
- `PitchField` should remain usable, but no longer be the only tuning concept
- future instrument work should have a clear place to plug in

## Current State

Right now tuning logic is split across several places:

- [`src/composition/pitch.rs`](/home/maggi/Documents/workspace/afruglariV2/src/composition/pitch.rs:1)
  owns `PitchField`
- [`src/composition/garden.rs`](/home/maggi/Documents/workspace/afruglariV2/src/composition/garden.rs:59)
  stores root, voice count, octave range, event attack/decay, and retune timing
- [`src/composition/layers/drone.rs`](/home/maggi/Documents/workspace/afruglariV2/src/composition/layers/drone.rs:11)
  owns octave range and retune timing
- [`src/composition/layers/events.rs`](/home/maggi/Documents/workspace/afruglariV2/src/composition/layers/events.rs:11)
  owns octave range and pitch-field use
- [`src/composition/timeline.rs`](/home/maggi/Documents/workspace/afruglariV2/src/composition/timeline.rs:12)
  stores root, voice count, octave range, attack/decay range, and retune time in
  `TimelineState`

That works for the current single-engine model, but it will become awkward once
multiple instruments need different tuning and register behavior.

## Refactor Strategy

Phase 1 should introduce one new module and then make a small set of targeted
changes:

1. add a shared tuning module
2. define explicit composition-level tuning structs
3. update current layers to consume those structs
4. keep existing public behavior stable

The principle is:

- centralize shared tuning concepts
- leave sound generation mostly untouched
- avoid broad renames unless they make later phases cleaner

## Files To Add First

### 1. `src/composition/tuning.rs`

This is the core new file for Phase 1.

It should define the shared tuning and register types used by the rest of the
composition engine.

Recommended initial contents:

```rust
use crate::composition::pitch::PitchField;

#[derive(Clone, Debug)]
pub struct TuningConfig {
    pub pitch_field: PitchField,
}

#[derive(Clone, Copy, Debug)]
pub struct RegisterRange {
    pub octave_min: i32,
    pub octave_max: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct DegreeSelection {
    pub enabled: [bool; 7],
}

#[derive(Clone, Copy, Debug)]
pub struct InstrumentTuning {
    pub register: RegisterRange,
    pub degree_selection: DegreeSelection,
    pub detune_cents: f32,
    pub retune_seconds: Option<f32>,
}
```

Not all fields must be used immediately. The important part is to establish a
home for shared tuning concepts now.

Minimal first version is acceptable:

```rust
#[derive(Clone, Debug)]
pub struct TuningConfig {
    pub pitch_field: PitchField,
}

#[derive(Clone, Copy, Debug)]
pub struct RegisterRange {
    pub octave_min: i32,
    pub octave_max: i32,
}
```

If you want the smallest safe refactor, start there and add
`DegreeSelection` / `InstrumentTuning` in the same PR only if they are used
immediately.

## Files To Update In Phase 1

### 2. `src/composition/mod.rs`

Add:

```rust
pub mod tuning;
```

This is the only required module wiring change in this phase.

### 3. `src/composition/pitch.rs`

Keep `PitchField`, but make it fit the new tuning layer better.

Recommended changes:

- add simple getters if needed:
  - `root_hz()`
  - `ratios()`
- derive `Debug` in addition to `Clone`
- keep `default_just()` and `frequency()` unchanged

Recommended resulting shape:

```rust
#[derive(Clone, Debug)]
pub struct PitchField { ... }
```

This file should remain focused on pitch-field math, not instrument tuning
policy.

### 4. `src/composition/garden.rs`

This file should stop owning raw register fields as the only source of truth
for tuning-related state.

Recommended new structs to add near `GardenConfig`:

```rust
#[derive(Clone, Copy, Debug)]
pub struct EventShapeConfig {
    pub attack_min_seconds: f32,
    pub attack_max_seconds: f32,
    pub decay_min_seconds: f32,
    pub decay_max_seconds: f32,
}
```

Recommended field changes inside `Garden`:

- replace:
  - `root_hz: f32`
  - `octave_min: i32`
  - `octave_max: i32`
- with:
  - `tuning: TuningConfig`
  - `register: RegisterRange`

Keep these as-is for now:

- `voice_count`
- `event_attack_min`
- `event_attack_max`
- `event_decay_min`
- `event_decay_max`
- `drone_retune_seconds`

If you want a slightly cleaner Phase 1, also replace the attack/decay fields
with:

```rust
event_shape: EventShapeConfig
```

That is optional. The hard requirement for Phase 1 is the tuning/register
split, not full envelope configuration cleanup.

Recommended new helper methods on `Garden`:

```rust
fn pitch_field(&self) -> &PitchField
fn register(&self) -> RegisterRange
```

Recommended method adjustments:

- `set_root_hz()` should rebuild `self.tuning.pitch_field`
- `set_octave_range()` should update `self.register`
- `Garden::new()` should construct `TuningConfig` and `RegisterRange` first,
  then pass them into `DroneLayer` and `EventLayer`

### 5. `src/composition/layers/drone.rs`

`DroneLayer` should consume shared tuning types instead of raw octave values
where possible.

Recommended signature changes:

```rust
pub fn new(
    sample_rate: f32,
    pitch_field: PitchField,
    register: RegisterRange,
    voice_count: usize,
    seed: u64,
    controls: GardenControls,
) -> Self
```

Replace the two fields:

- `octave_min: i32`
- `octave_max: i32`

with:

- `register: RegisterRange`

Replace:

```rust
pub fn set_octave_range(&mut self, octave_min: i32, octave_max: i32)
```

with:

```rust
pub fn set_register(&mut self, register: RegisterRange)
```

Implementation detail:

- `redistribute_octaves()` should read from `self.register`
- `retune_voices()` can stay structurally the same

Do not rewrite the voice logic in Phase 1. Only move it onto the shared
register abstraction.

### 6. `src/composition/layers/events.rs`

`EventLayer` should follow the same pattern as `DroneLayer`.

Recommended signature changes:

```rust
pub fn new(
    sample_rate: f32,
    pitch_field: PitchField,
    register: RegisterRange,
    seed: u64,
    controls: GardenControls,
) -> Self
```

Replace the two fields:

- `octave_min: i32`
- `octave_max: i32`

with:

- `register: RegisterRange`

Replace:

```rust
pub fn set_octave_range(&mut self, octave_min: i32, octave_max: i32)
```

with:

```rust
pub fn set_register(&mut self, register: RegisterRange)
```

In `trigger_voice()`, octave choice should read from `self.register`.

### 7. `src/composition/timeline.rs`

This file does not need a format redesign yet. The Phase 1 goal here is
internal cleanup only.

Recommended additions:

- helper conversion from `TimelineState` to `RegisterRange`
- possibly helper conversion from `TimelineState` to `TuningConfig` later

Recommended method to add:

```rust
impl TimelineState {
    pub fn register(self) -> RegisterRange
}
```

This lets `Garden::apply_timeline_controls()` stop passing raw octave pairs
around.

No syntax changes are required in this phase.

## Exact Structs To Add First

If you want the most conservative sequence, add these in this order.

### Step 1: Add `RegisterRange`

File:
[`src/composition/tuning.rs`](/home/maggi/Documents/workspace/afruglariV2/src/composition/tuning.rs)

```rust
#[derive(Clone, Copy, Debug)]
pub struct RegisterRange {
    pub octave_min: i32,
    pub octave_max: i32,
}
```

Methods:

```rust
impl RegisterRange {
    pub fn new(octave_min: i32, octave_max: i32) -> Self;
    pub fn clamped(self) -> Self;
}
```

Reason:
this is the lowest-risk shared type and immediately removes duplicate octave
range policy from multiple modules.

### Step 2: Add `TuningConfig`

File:
[`src/composition/tuning.rs`](/home/maggi/Documents/workspace/afruglariV2/src/composition/tuning.rs)

```rust
#[derive(Clone, Debug)]
pub struct TuningConfig {
    pub pitch_field: PitchField,
}
```

Methods:

```rust
impl TuningConfig {
    pub fn default_just(root_hz: f32) -> Self;
    pub fn root_hz(&self) -> f32;
}
```

Reason:
this gives the codebase a tuning object without prematurely designing the full
future model.

### Step 3: Add `EventShapeConfig` Optional

File:
[`src/composition/garden.rs`](/home/maggi/Documents/workspace/afruglariV2/src/composition/garden.rs)

```rust
#[derive(Clone, Copy, Debug)]
pub struct EventShapeConfig {
    pub attack_min_seconds: f32,
    pub attack_max_seconds: f32,
    pub decay_min_seconds: f32,
    pub decay_max_seconds: f32,
}
```

Reason:
this is not mandatory for the tuning refactor, but it reduces the number of
parallel scalar fields in `Garden`.

## Exact Method Changes To Make First

These are the first method changes worth making after the structs exist.

### `Garden::new`

Current responsibility:

- creates `PitchField`
- creates layers
- sets raw octave and envelope fields

New responsibility:

- creates `TuningConfig`
- creates `RegisterRange`
- passes shared tuning state into layers

### `Garden::set_root_hz`

Current behavior:

- rebuilds a `PitchField`
- passes it to drone and event layers

New behavior:

- updates `self.tuning`
- passes `self.tuning.pitch_field.clone()` to affected layers

### `Garden::set_octave_range`

Current behavior:

- stores `octave_min` / `octave_max`
- passes them to drone and event layers

New behavior:

- stores a `RegisterRange`
- passes it via `set_register()`

### `DroneLayer::set_octave_range`

Rename to:

```rust
set_register(&mut self, register: RegisterRange)
```

### `EventLayer::set_octave_range`

Rename to:

```rust
set_register(&mut self, register: RegisterRange)
```

## Recommended Implementation Order

This is the exact order I would use for the refactor.

1. Add `src/composition/tuning.rs` with `RegisterRange` only.
2. Export `tuning` from `src/composition/mod.rs`.
3. Update `DroneLayer` to use `RegisterRange`.
4. Update `EventLayer` to use `RegisterRange`.
5. Update `Garden` to store and pass `RegisterRange`.
6. Run `cargo check`.
7. Add `TuningConfig` around `PitchField`.
8. Update `Garden` to store `TuningConfig`.
9. Add timeline helper conversions.
10. Run `cargo check` and `cargo test`.

That order keeps the refactor shallow and makes breakage easier to isolate.

## Explicit Non-Goals

Do not do these in the same change unless required for compilation:

- creating `src/instruments/`
- adding instrument traits
- changing CLI argument names
- changing timeline file syntax
- adding new DSP voices
- removing the existing layer modules

Those are Phase 2 and later.

## Verification Checklist

After Phase 1 lands, verify all of the following:

- `cargo check` passes
- timeline tests still pass
- current example render commands still run
- realtime playback still starts
- `set_root_hz()` still updates drone and event pitch behavior
- octave range timeline changes still affect both drone and events

## Expected Diff Shape

The Phase 1 diff should mainly touch:

- one new file in `src/composition/`
- `composition/mod.rs`
- `composition/pitch.rs`
- `composition/garden.rs`
- `composition/layers/drone.rs`
- `composition/layers/events.rs`
- `composition/timeline.rs`

If the diff starts spreading into rendering, audio backend, or CLI parsing in a
major way, the refactor is likely doing too much for Phase 1.
