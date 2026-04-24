# Post-Garden Architecture Plan

## Purpose

This document defines the near-term architecture after the current
`Garden`-centered model.

The goal is not to build a full DAW immediately.

The goal is to stop treating the application as one global ambient instrument
with shared macros, and instead treat it as a project made from named synth
instances.

This is the smallest architectural shift that:

- removes `Garden` from the center of the design
- supports direct per-instrument editing
- supports multiple named synth instances
- creates a clean path toward richer composition tooling later

## Working Goal

The application should move from this mental model:

```text
Garden
  global controls
  fixed instrument families
  shared timeline state
```

to this one:

```text
Project
  instruments
  arrangement
  transport
  shared fx
```

The main object should become `Project`, not `Garden`.

## Why Move Away From `Garden`

`Garden` made sense when the application was primarily one evolving ambient
patch.

It is now creating friction because it still implies:

- one global musical organism
- one shared set of macros
- one family-collapsed playback model
- one editor centered on the whole patch rather than on selected instruments

That conflicts with the desired workflow:

- create a synth
- name it
- select it
- edit only that synth
- automate only that synth
- build a piece from several instances

Even without aiming for a full DAW, that workflow needs a project model rather
than a garden model.

## New Center Of Gravity

The new center of gravity should be:

- `Project`
- `InstrumentDefinition`
- `InstrumentInstance`
- `Automation`
- `Transport`
- `SharedFx`

This means:

- project state becomes primary
- synth instances become explicit objects
- automation targets explicit instances and parameters
- runtime becomes a host of instances rather than a fixed family rack

## Target Model

Conceptually, the application should look like this:

```text
Project
  instrument_instances
    drone_main
    drone_shadow
    pulse_low
    sampler_glass
  arrangement
    sections
    automation
  transport
    playback state
    loop state
    position
  shared_fx
    delay
    texture
```

And runtime should look like:

```text
Engine
  RuntimeHost
    InstrumentRuntime(drone_main)
    InstrumentRuntime(drone_shadow)
    InstrumentRuntime(pulse_low)
    InstrumentRuntime(sampler_glass)
  SharedFx
```

The important change is that runtime playback should be driven by project-side
instrument instances, not by a fixed hardcoded set of families.

## What Replaces `Garden`

`Garden` is currently doing too many jobs:

- top-level project state holder
- control/macro store
- instrument rack owner
- texture owner
- delay owner
- timeline application surface
- sample trigger transport surface

Those jobs should be split.

Recommended replacements:

### `Project`

Owns persistent composition data:

- instrument instances
- arrangement/sections
- automation
- sample assets
- project metadata later

### `Engine`

Owns realtime/offline playback state:

- transport
- runtime host
- shared fx
- sample clock

### `RuntimeHost`

Owns runtime synth instances created from project data.

Responsibilities:

- instantiate synth runtimes
- apply parameter/tuning changes
- mix audio from instances

### `SharedFx`

Owns shared non-instrument processing:

- delay
- texture if still global

### `Automation`

Owns changes over time targeted at:

- instrument enabled state
- instrument gain/pan
- instrument parameters
- possibly shared FX later

## Transitional Rule

Do not evolve `Garden` further as the main architecture.

From this point, treat it as legacy scaffolding.

That means:

- avoid adding new major features directly to `Garden`
- avoid expanding `GardenControls` as the primary authoring model
- avoid deepening family-specific GUI assumptions

New architecture work should land under new project/runtime concepts.

## Immediate Design Direction

The application does not need tracks, clips, or a full mixer yet.

The near-term model can stay intentionally small:

### `Project`

Suggested shape:

```rust
pub struct Project {
    pub instruments: Vec<InstrumentInstance>,
    pub arrangement: Arrangement,
    pub sample_assets: Vec<SampleAssetSpec>,
}
```

### `InstrumentInstance`

Suggested shape:

```rust
pub struct InstrumentInstance {
    pub id: String,
    pub definition_id: String,
    pub enabled: bool,
    pub gain: f32,
    pub pan: f32,
    pub params: ParameterStore,
    pub tuning: InstrumentTuning,
}
```

### `Engine`

Suggested shape:

```rust
pub struct Engine {
    runtime_host: RuntimeHost,
    shared_fx: SharedFx,
    transport: Transport,
}
```

### `AutomationTarget`

Suggested shape:

```rust
pub enum AutomationTarget {
    InstrumentEnabled { instance_id: String },
    InstrumentGain { instance_id: String },
    InstrumentPan { instance_id: String },
    InstrumentParam { instance_id: String, param_id: String },
}
```

This is already enough to support a much cleaner composition model than
`Garden`.

## What Can Stay Global For Now

Not everything needs to become per-instrument immediately.

These can remain global in the first post-garden phase:

- transport
- master delay
- texture bus if still shared
- render settings
- sample library

This keeps the refactor bounded while still removing the main conceptual
problem.

## What Should Stop Being Global

These should stop being the primary top-level composition surface:

- `GardenControls`
- family-wide level sliders
- family-wide active state
- family-wide parameter enums
- family-addressed timeline editing

They may survive temporarily as compatibility layers, but not as the future
model.

## UX Direction

The GUI should shift from:

- "edit the garden"

to:

- "edit the selected instrument inside the project"

That means the editor should orient around:

1. project instrument list
2. selected instrument panel
3. arrangement overview
4. automation for selected instrument

The user should feel:

- "I am editing `drone_main`"

not:

- "I am editing some global state that affects a family somewhere"

## Migration Strategy

This migration should be staged. Keep the app working during the transition.

Recommended approach:

1. define the new post-garden types
2. build a new runtime path beside `Garden`
3. migrate one synth end to end
4. move the GUI to the new selected-instrument flow
5. retire `Garden` once the new path is proven

## Recommended Phases

### Phase 1: Add New Top-Level Types

Add the new architectural center beside existing code:

- `Project`
- `InstrumentDefinition`
- `InstrumentInstance`
- `ParameterStore`
- `RuntimeHost`
- `Engine`

At this stage:

- existing playback can remain on the current path
- the new types are allowed to be only partially connected

Goal:

- create the replacement vocabulary for `Garden`

### Phase 2: Wrap Existing Built-Ins As Definitions

Register current synth types through a common definition interface.

Examples:

- `drone_basic`
- `harmonic_pad`
- `pulse_basic`
- `noise_layer`
- `event_layer`
- `sampler`

Goal:

- synth types become registry-backed instead of family-hardcoded at every call
  site

### Phase 3: Build A New Runtime Host

Add a new dynamic runtime host that instantiates synth runtimes from project
instrument instances.

Goal:

- prove playback without going through the fixed `Garden` path

This is the key structural seam.

### Phase 4: Migrate One Synth Fully

Use `Drone` as the pilot.

Goal:

- `drone_main` becomes a true instrument instance
- the editor can select and edit it directly
- playback creates its runtime from project data
- automation targets that instance directly

Once this works, the new architecture is real.

### Phase 5: Move GUI To Project/Instrument Editing

Reshape the GUI around:

- instrument list
- selected instrument inspector
- arrangement overview
- transport

At this stage, the main panel should no longer lead with macro editing.

### Phase 6: Retire Family-Collapsed State

Once multiple instance-based synths are migrated:

- stop using family-based state as the primary project model
- remove or demote `GardenControls`
- remove or demote family-based parameter stores

### Phase 7: Remove `Garden`

Only remove `Garden` after:

- realtime playback works on the new engine path
- offline rendering works on the new engine path
- arrangement loading/saving works
- the GUI no longer depends on garden-specific concepts

At that point:

- `Garden` can be deleted
- or kept only behind a compatibility layer temporarily if needed

## Pilot Migration Recommendation

Start with this narrow success case:

1. `Project` with one or two drone instances
2. new engine path that plays those drone instances
3. GUI panel that edits selected drone instance only
4. simple automation targeting one drone parameter

Do not try to migrate every synth and every editor feature at once.

If this pilot works, the rest of the migration becomes mostly repetition and
polish.

## File-Level Direction

Likely new modules:

```text
src/
  project/
    mod.rs
    model.rs
    automation.rs
  engine/
    mod.rs
    runtime_host.rs
    transport.rs
    shared_fx.rs
  instruments/
    definition.rs
    registry.rs
    runtime.rs
```

Possible transition path:

- keep existing `audio/engine.rs` temporarily
- keep existing `composition/garden.rs` temporarily
- add new modules beside them
- move call sites gradually

The exact file names can change. The important part is making sure new work
does not continue to accumulate inside `Garden`.

## Non-Goals

The post-garden refactor should not try to solve everything at once.

Out of scope for the first phase:

- full DAW track model
- clip launching
- mixer buses everywhere
- external plugin loading
- modular node graph editing
- full undo/redo system

Those may come later. They are not required to leave the garden model behind.

## Decision Summary

The near-term direction should be:

- move from `Garden` to `Project`
- move from family control to instance control
- move from fixed rack playback to runtime-host playback
- move from macro-centered UI to selected-instrument UI

This is the smallest architectural change that aligns the system with the
desired composition workflow without requiring a full DAW rewrite.

## Immediate Next Steps

Recommended next implementation steps:

1. add `Project` and `InstrumentInstance` types
2. add definition/registry/runtime traits for synths
3. build a new runtime host beside `Garden`
4. migrate the drone synth as the first post-garden instrument
5. build a GUI panel for selected-instrument editing

After that, the codebase will have a real post-garden path, and the remaining
work becomes migration rather than rethinking the whole architecture again.
