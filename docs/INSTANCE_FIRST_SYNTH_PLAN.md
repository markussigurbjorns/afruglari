# Instance-First Synth Architecture Plan

## Purpose

This document defines the next architectural direction for Drone Garden after
the current family-based, macro-centered system.

The goal is to move toward a composition model where the user:

- creates named instrument instances
- selects one instrument at a time
- edits that instrument directly
- automates that instrument directly
- builds a piece from a collection of synth instances rather than from one
  shared macro state

This plan replaces the current assumption that composition should primarily be
driven through global macros such as `density`, `brightness`, `space`, and
family level sliders.

Those controls may survive later as optional performance or mix controls, but
they should no longer be the core editing model.

## Working Goal

The system should support a workflow like this:

1. add a new instrument
2. choose a synth type
3. name the instrument instance
4. select that instance in the editor
5. shape only that instrument's parameters
6. automate that instrument over time
7. combine several instances into a piece

Examples:

- `drone_main`
- `drone_high`
- `pulse_low`
- `sampler_glass`
- `noise_wash_a`

The editor should feel like a composition environment for synth instances, not
like a global patch editor.

## Why Change Direction

The current system has become strong enough to generate sound, render pieces,
and edit arrangements, but the composition model is still centered on
shared/global controls:

- `GardenControls`
- family level sliders
- family-addressed activity state
- family-addressed parameter storage

That is useful for a fixed built-in patch, but it creates several problems for
the next stage:

- the user cannot clearly think in terms of individual instruments
- sound design is constrained by what the global macros happen to expose
- "add instrument" in the GUI does not yet create a true playback instance
- adding new synth types requires touching too many hardcoded family paths
- GUI layout is driven by hardcoded families instead of registered synth
  metadata

The desired system should instead be:

- instance-first
- patch-driven
- metadata-driven
- registry-backed

## Design Principles

### 1. Instrument Instances Are Primary

The primary object in a project should be a named instrument instance, not a
family bucket and not a global macro state.

Examples:

- `drone_main`
- `drone_shadow`
- `pulse_low`
- `texture_bus` later if shared buses become editable objects

### 2. Direct Instrument Control

The main editing surface should target the selected instrument instance only.

The user should not need to think:

- "what does this macro do to all layers?"

The user should be able to think:

- "what does this synth do when I change this parameter?"

### 3. Synth Definitions Should Be Data-Described

Built-in synths should register themselves through a shared definition format:

- id
- label
- category
- parameter schema
- defaults
- UI metadata
- runtime factory

This enables:

- metadata-driven GUI generation
- less hardcoded family branching
- easier addition of new synth types
- eventual plugin/module style expansion

### 4. Automation Targets Instances and Parameters

Timeline and arrangement data should target:

- `instance_id`
- `parameter_id`

not:

- family enum
- global macro field

### 5. Global Macros Become Optional

Global macros may still exist later as:

- performance controls
- mix controls
- broad composition gestures

but they should not be required to create or edit a piece.

## Target Architecture

Conceptually, the project should move toward:

```text
Project
  InstrumentRegistry
    SynthDefinition(drone_basic)
    SynthDefinition(harmonic_pad)
    SynthDefinition(pulse_basic)
    SynthDefinition(sampler)
  InstrumentInstances
    drone_main -> drone_basic
    drone_high -> drone_basic
    pulse_low -> pulse_basic
    sampler_glass -> sampler
  Arrangement
    Sections
    Transport
    Automation
      instance_id + parameter_id targets
  RuntimeHost
    InstrumentRuntime(drone_main)
    InstrumentRuntime(drone_high)
    InstrumentRuntime(pulse_low)
    InstrumentRuntime(sampler_glass)
  Shared FX
    delay
    texture bus
```

## Core Types

The final names may change, but the system should have concepts equivalent to
these.

### `InstrumentDefinition`

Describes a synth type.

Suggested contents:

```rust
pub struct InstrumentDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub category: InstrumentCategory,
    pub parameters: &'static [ParameterSpec],
    pub create: fn(InstrumentBuildContext) -> Box<dyn InstrumentRuntime>,
}
```

Responsibilities:

- identify a synth type
- describe its editable parameters
- describe how the GUI should render those parameters
- create runtime synth instances

### `ParameterSpec`

Describes one editable parameter.

Suggested contents:

```rust
pub struct ParameterSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: ParameterKind,
    pub default: ParameterValue,
    pub ui: ParameterUi,
}
```

Responsibilities:

- stable parameter identity
- default value
- range / enum choice metadata
- widget metadata for GUI generation

### `ParameterValue`

Represents a stored parameter value.

Suggested initial shape:

```rust
pub enum ParameterValue {
    Bool(bool),
    Float(f32),
    Int(i32),
    Choice(u32),
}
```

### `ParameterStore`

Stores parameter values for one instrument instance.

Responsibilities:

- override defaults from the definition
- provide values to runtime synths
- provide values to GUI editors
- provide automation targets

### `InstrumentInstance`

Represents one named synth in a project.

Suggested contents:

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

Responsibilities:

- identify a project-side synth instance
- point to the synth definition it uses
- store its patch values
- store its tuning and mix state

### `InstrumentRuntime`

Represents the live DSP object.

Suggested trait:

```rust
pub trait InstrumentRuntime: Send {
    fn set_enabled(&mut self, enabled: bool);
    fn apply_patch(&mut self, params: &ParameterStore);
    fn apply_tuning(&mut self, tuning: &InstrumentTuning);
    fn next_stereo(&mut self) -> StereoSample;
}
```

Responsibilities:

- generate audio
- accept patch changes
- accept tuning/register changes
- stay independent from hardcoded GUI/family logic

### `InstrumentRegistry`

Stores all available synth definitions.

Responsibilities:

- register built-in synth definitions at startup
- provide lookup by `definition_id`
- provide GUI metadata for synth creation/editor rendering
- provide runtime factories for host instantiation

## UX Direction

The editor should become instrument-first.

The default workflow should be:

1. create/select an instrument instance
2. edit that instance's parameters
3. hear only that instrument change
4. place or automate that instrument in the piece
5. move to the next instrument

The main panel should no longer be centered around global macros.

Instead, the main flow should be something like:

- left: instrument list and piece structure
- top: arrangement timeline
- center: selected instrument editor
- bottom: automation lanes for selected instrument

The user should be able to answer these questions quickly:

- what instruments are in this piece?
- which one am I editing right now?
- what synth type is it?
- what parameters define its sound?
- where is it active in the arrangement?
- what automation exists for it?

## Migration Strategy

This migration should be staged. Do not try to remove the old system all at
once.

The safest path is:

1. add the new architecture in parallel
2. wrap existing synth families in the new model
3. migrate one synth fully
4. migrate GUI/editor flow
5. remove obsolete family/macro scaffolding later

## Recommended Phases

### Phase 0: Freeze The Direction

Before implementation starts, align on these decisions:

- instrument instances are the primary project object
- arrangement/timeline will target instance ids
- parameter metadata will drive GUI controls
- global macros will stop being the main composition surface
- external dynamic plugin loading is out of scope for the first migration

### Phase 1: Add A Definition Registry

Goal:

- introduce `InstrumentDefinition` and `InstrumentRegistry`
- register current built-in synths through the registry
- do not change runtime behavior yet

Expected result:

- the codebase gains a central source of truth for synth types
- the GUI and project model can begin querying definitions instead of
  switching on hardcoded families everywhere

Recommended first built-in definitions:

- `drone_basic`
- `harmonic_pad`
- `pulse_basic`
- `noise_layer`
- `event_layer`
- `sampler`

### Phase 2: Add Generic Parameter Metadata And Storage

Goal:

- replace hardcoded family param enums as the long-term model
- add `ParameterSpec`, `ParameterValue`, and `ParameterStore`

Expected result:

- synth parameters become data-described
- GUI editors can be generated from metadata
- project files can store patch values generically

Keep the current family-specific structs temporarily as compatibility bridges
if needed.

### Phase 3: Add True Instrument Instances To The Project Model

Goal:

- make named instrument instances the real project objects
- stop treating GUI-side instrument ids as placeholders

Expected result:

- `drone_main` and `drone_alt` become separate project objects
- instance definitions reference a synth definition id
- instances own their own parameter stores and tuning state

This phase should make project state authoritative even if runtime still uses
some transitional mapping internally.

### Phase 4: Migrate Timeline And Arrangement To Instance Targets

Goal:

- automation and activity should target `instance_id + parameter_id`
- section state should stop being primarily family-based

Expected result:

- the arrangement model becomes aligned with the actual UX
- users can automate one synth instance without editing a family bucket

Likely new automation target shape:

```rust
pub enum AutomationTarget {
    InstrumentEnabled { instance_id: String },
    InstrumentGain { instance_id: String },
    InstrumentPan { instance_id: String },
    InstrumentParam { instance_id: String, param_id: String },
}
```

### Phase 5: Replace The Fixed Rack With A Runtime Host

Goal:

- refactor the runtime from fixed family fields to a dynamic collection of
  runtime instances

Expected result:

- current `InstrumentRack` becomes a generic host
- runtime instances are built from registered definitions
- multiple instances of the same synth type become normal

This is the phase where the project truly becomes synth-instance driven.

### Phase 6: Make The GUI Metadata-Driven And Instance-First

Goal:

- remove hardcoded family editor assumptions from the main composition flow

Expected result:

- the user selects an instance from a list
- the editor panel renders controls from the selected definition metadata
- adding a new synth type does not require hand-building a new GUI section

The GUI should focus on:

- selected instrument
- arrangement role
- automation visibility
- fast auditioning

not:

- one global macro slab
- one hardcoded family form per synth type

### Phase 7: Remove Macro-Centered Editing As The Primary Model

Goal:

- demote or remove `GardenControls` from being the central composition state

Expected result:

- macros become optional helpers or are phased out
- composition is built from per-instance editing and automation

This phase should happen only after instance editing is good enough to replace
the current experience.

## Recommended Pilot Migration

Use the drone synth as the first full pilot.

Reasons:

- it is central to the current sound world
- it already has enough parameters to prove the model
- it tests tuning/register behavior
- it is a good candidate for multiple instances of the same synth type

Pilot goal:

- create a true `drone_basic` definition
- create real project instances such as `drone_main` and `drone_shadow`
- edit those instances directly in the GUI
- instantiate multiple drone runtimes in playback
- automate their instance-local parameters

Once that path works, migrate `pulse`, `harmonic`, `noise`, `events`, and
`sampler` into the same system.

## File-Level Direction

This is not a final file list, but the likely direction is:

```text
src/
  instruments/
    mod.rs
    registry.rs
    definition.rs
    runtime.rs
    host.rs
    builtins/
      drone_basic.rs
      harmonic_pad.rs
      pulse_basic.rs
      noise_layer.rs
      event_layer.rs
      sampler.rs
  composition/
    project.rs
    arrangement.rs
    automation.rs
    tuning.rs
  gui/
    mod.rs
    instruments.rs
    timeline.rs
    inspectors.rs
```

The exact module split can vary. The important part is the separation of:

- synth definitions
- project instances
- runtime host
- GUI rendering from metadata

## Non-Goals For The First Refactor

Do not include these in the first pass:

- external dynamic plugin loading
- scripting systems
- a fully modular synth graph editor
- user-authored DSP modules
- replacing shared FX buses with per-instance FX everywhere

The first goal is internal plugin-style architecture, not third-party plugin
support.

## Key Decisions To Lock Early

### Parameter Ids

Use stable string ids for:

- file format readability
- GUI mapping
- automation targets

### Schema Versioning

Synth definitions and saved patches will need a versioning story eventually.

At minimum:

- plan for missing parameters
- plan for renamed parameters
- plan for default fallback when loading old projects

### Shared FX

Keep shared FX global for the first migration:

- delay
- texture bus if still shared

Do not block instance-first synth work on a full FX routing overhaul.

### Macros

Treat macros as compatibility scaffolding during migration.

The target is:

- instance-local parameter editing first
- optional macros later if they still feel useful

## Immediate Next Steps

Recommended first implementation steps:

1. add `InstrumentDefinition` and `InstrumentRegistry`
2. wrap current built-in synths as registered definitions
3. add generic `ParameterSpec` / `ParameterValue` / `ParameterStore`
4. define a real `InstrumentInstance` project model
5. migrate `Drone` as the first instance-first synth path

That is the smallest path that proves the architecture without attempting a
full rewrite in one step.
