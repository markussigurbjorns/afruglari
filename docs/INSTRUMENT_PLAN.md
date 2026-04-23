# Drone Garden Instrument Plan

## Purpose

This document defines the next phase of Drone Garden after the current
layered ambient engine.

The goal is to turn the project from a single hard-coded generative patch
into a composition-oriented sound engine with:

- multiple instrument types
- explicit tuning control
- one-shot sample playback
- pulsing / repeating material
- a path toward a future GUI composer

MIDI input is intentionally out of scope for this phase.

## Current Refactor Roadmap

The current implementation is in a transition phase between a fixed built-in
layer stack and a true instrument-instance composition engine.

What is already in place:

- `Garden` is thinner and delegates source orchestration to `InstrumentRack`
- `InstrumentFamily` is shared across GUI, arrangement, timeline, and engine
- timeline active/override state is family-addressable rather than hard-coded
- instrument params are stored through a family-addressable typed parameter
  store

What still remains fixed:

- playback still assumes one built-in instance per family
- arrangement section control lines still target families, not named instance ids
- GUI editing still renders a fixed family editor layout
- shared FX such as texture are still partially special-cased outside the rack

Target end state:

- project model owns named instrument instances such as `drone_main`,
  `pulse_low`, or `sampler_glass`
- instrument type and instrument instance are separate concepts
- arrangement and timeline events target instrument ids rather than family
  buckets
- `Garden` owns transport, timing, and shared FX while `InstrumentRack` owns
  source instances
- adding a new instrument should be mostly local to the rack + GUI/editor
  registration, not a whole-engine rewrite

Planned steps from here:

1. introduce named instrument instances in the arrangement/project model
2. keep backward-compatible fixed-family playback while that model lands
3. migrate arrangement and timeline controls from family targets to instance ids
4. let the rack instantiate and route multiple instances of the same family
5. separate shared FX/buses from true instrument instances cleanly

Immediate next step:

- add explicit named instrument instances to arrangement files and GUI project
  state as groundwork for id-targeted sequencing

## Working Goal

Drone Garden should support building a piece from several different sound
sources rather than from one fixed garden texture.

A useful next version should let a piece combine:

- several pad / drone instruments with distinct timbres
- one-shot sample accents
- pulsing tonal or noisy movement
- per-instrument tuning and register behavior
- timeline-driven arrangement of which instruments are active and how they
  evolve

The system should still support:

- realtime playback
- offline WAV rendering
- deterministic seeded behavior where appropriate
- a simple text-based composition workflow for now

## Current Constraints

The current engine is strong for one evolving ambient patch, but it has some
structural limits:

- `Garden` owns a fixed set of layers directly
- tuning is mostly global
- the timeline primarily interpolates global macro state
- there is no instrument identity beyond the built-in layers
- there is no sample playback path yet
- there is no dedicated pulse engine

This is enough for one sound world, but not yet enough for composing richer
pieces with contrast and recurring material.

## Design Direction

The next phase should separate these responsibilities:

- engine / transport
- instruments
- tuning model
- arrangement / timeline
- shared FX and output

The important change is that the top-level engine should stop thinking in
terms of a few fixed layers and start thinking in terms of named instruments
or tracks.

That does not mean the current layers are wrong. It means they should become:

- instrument implementations
- or internal building blocks used by instruments

## Target Architecture

Conceptually, the project should move toward:

```text
GardenEngine
  InstrumentRack
    PadInstrument A
    PadInstrument B
    PulseInstrument
    SamplerInstrument
  Shared FX
    Texture Bus
    Stereo Delay
  Arrangement
    Timeline
    Scenes / sections later
```

Suggested module direction:

```text
src/
  audio/
    engine.rs
  composition/
    arrangement.rs
    timeline.rs
    tuning.rs
    scene.rs
  instruments/
    mod.rs
    pad_basic.rs
    pad_shimmer.rs
    pulse.rs
    sampler.rs
  dsp/
    envelope.rs
    lfo.rs
    sample_player.rs
    delay.rs
    filter.rs
```

This does not all need to land at once. The main point is to carve out an
`instruments/` layer and introduce a structured tuning model.

## Instrument Model

The engine should support multiple named instruments at the same time.

Each instrument should have:

- a sound generator
- macro controls
- tuning / register settings
- an active state
- optional trigger behavior

An initial interface could look like:

```rust
pub trait Instrument {
    fn next_stereo(&mut self) -> StereoSample;
    fn set_controls(&mut self, controls: InstrumentControls);
    fn set_tuning(&mut self, tuning: InstrumentTuning);
    fn set_active(&mut self, active: bool);
}
```

Not every instrument needs every capability immediately. For example:

- pad instruments are usually continuous
- sampler instruments need trigger events
- pulse instruments need rate / gate / pattern settings

The trait can evolve later, but the design principle should remain:
`Garden` hosts instruments instead of directly owning only fixed layer types.

## First Instrument Families

The first expansion should focus on a small number of clearly distinct musical
roles.

### 1. Pad / Drone Instruments

Add at least two more pad families beyond the current drone behavior.

Recommended first set:

- `basic_drone_pad`
  - current Drone Garden tonal bed
  - slow retuning
  - just-intonation pitch field
- `harmonic_pad`
  - richer overtone content
  - slower attack / bloom
  - smoother, wider tone
- `bowed_noise_pad`
  - tonal core plus filtered noise
  - darker and more unstable

These should sound meaningfully different, not just expose more parameters on
the same oscillator design.

### 2. Pulse Instrument

Add a dedicated pulse engine instead of stretching the current event layer to
cover repeating movement.

Initial pulse behaviors should support:

- pulse rate in seconds
- gate length
- amplitude
- pitch field degree selection
- octave range
- optional skip probability
- optional brightness modulation

Musically this should cover:

- low throbs
- repeating tonal pulses
- filtered noise pumping
- gentle rhythmic motion in ambient pieces

### 3. Sampler Instrument

Add one-shot WAV playback for composition accents and found sound.

Initial sampler capabilities:

- load mono or stereo WAV
- one-shot playback
- gain
- pan
- playback rate
- start offset
- optional reverse later
- optional lowpass / highpass later

The first version does not need slicing, granular playback, or a library UI.

## Tuning Plan

Tuning should become a first-class concept instead of staying implicit inside
the current `PitchField`.

There are three levels of tuning to support.

### Global Tuning

Applies to the piece or section:

- root frequency
- pitch field / scale / ratio set
- default octave policy

### Instrument Tuning

Applies to one instrument:

- enabled pitch degrees or preferred degrees
- octave range
- detune spread
- retune rate
- transpose offset

### Timeline Tuning Changes

Applies over time:

- root movement between sections
- register changes
- instrument-specific tuning changes
- pulse pitch behavior changes

A useful initial data model might look like:

```rust
pub struct PitchField {
    pub root_hz: f32,
    pub ratios: Vec<f32>,
}

pub struct InstrumentTuning {
    pub degree_mask: Vec<bool>,
    pub octave_min: i32,
    pub octave_max: i32,
    pub transpose_semitones: f32,
    pub detune_cents: f32,
    pub retune_seconds: Option<f32>,
}
```

The exact shape can change, but the system should support per-instrument
register and pitch behavior cleanly.

## Timeline Evolution

The current timeline is good for macro interpolation. It should remain
supported, but the next phase should let it address instruments explicitly.

The timeline should eventually cover two kinds of change:

- continuous automation
- discrete arrangement actions

### Continuous Automation

Examples:

- `brightness`
- `space`
- instrument level
- root frequency
- pulse rate
- filter amount

### Discrete Arrangement Actions

Examples:

- enable / disable an instrument
- switch an instrument type
- trigger a one-shot sample
- change a tuning profile
- start or stop a pulse voice

An eventual format might look like:

```text
0 root=110 pad_a.type=basic_drone pad_a.level=0.8 pad_a.octave_min=1 pad_a.octave_max=2
45 pad_b.type=harmonic pad_b.level=0.35
90 pulse_a.level=0.25 pulse_a.rate=1.2
120 sample.trigger=glass-hit.wav sample.gain=0.4 sample.pan=-0.2
180 root=82.41 pad_a.level=0.5 pad_b.level=0.55
```

This should not be implemented all at once. The next timeline step should be
small and practical:

1. keep the current text timeline
2. add instrument-addressed parameters
3. add a minimal trigger syntax for one-shots

## Shared FX Direction

The current shared delay and texture behavior are still useful.

The likely near-term structure is:

- per-instrument dry generation
- mix to a shared bus
- optional per-instrument send later
- shared texture / memory path
- shared stereo delay

This lets new instruments integrate without each one rebuilding the same FX.

Do not move immediately to a complex mixer with many send buses unless the
music actually demands it.

## Compatibility Strategy

The current `Garden` behavior should keep working while the new system lands.

Compatibility goal:

- existing CLI flags still render a usable piece
- existing timelines still parse
- the original garden sound remains available as one instrument preset or
  default rack setup

This reduces risk and keeps the project listenable throughout the refactor.

## Implementation Phases

### Phase 1: Tuning Abstraction

Goal:
introduce a reusable tuning model without breaking current audio behavior.

Tasks:

1. Add `composition/tuning.rs`.
2. Move or wrap current `PitchField` logic behind a clearer tuning API.
3. Add `InstrumentTuning` with octave range and preferred degree support.
4. Update existing drone and event behavior to read from the new tuning model.

Success criteria:

- current garden still sounds the same by default
- tuning and register logic are no longer scattered across several modules

### Phase 2: Instrument Boundary

Goal:
make `Garden` host instruments instead of directly hard-coding only layers.

Tasks:

1. Add `src/instruments/mod.rs`.
2. Define a minimal `Instrument` trait or enum-driven interface.
3. Wrap the current drone behavior as a first instrument implementation.
4. Decide whether noise / events remain separate instruments or internal parts
   of the default rack.

Success criteria:

- the top-level engine can host at least one named instrument cleanly
- current render and realtime paths still use the same source engine

### Phase 3: New Pad Instruments

Goal:
increase the available tonal palette.

Tasks:

1. Add `harmonic_pad`.
2. Add `bowed_noise_pad` or another contrasting pad voice.
3. Add configuration for instrument type selection.
4. Add a default rack with more than one pad option available.

Success criteria:

- at least three clearly different sustained tonal instruments exist
- instrument identity can be selected explicitly

### Phase 4: Pulse Instrument

Goal:
add repeating motion suitable for composition.

Tasks:

1. Add a dedicated pulse instrument.
2. Support rate, gate, level, octave range, and brightness.
3. Allow pulse activity to be set from the timeline.

Success criteria:

- the engine can generate stable repeating material separate from sparse event
  bells
- pulse parts can be arranged over time

### Phase 5: Sampler Instrument

Goal:
add one-shot audio accents and sample-based material.

Tasks:

1. Add WAV loading support.
2. Add a basic in-memory sample asset representation.
3. Add one-shot playback with voice pooling.
4. Add timeline trigger support for sample playback.

Success criteria:

- a piece can trigger one-shot samples during realtime playback and offline
  render
- sample accents are deterministic in offline render mode

### Phase 6: Instrument-Aware Timeline

Goal:
turn the timeline into a composition tool instead of only a global macro lane.

Tasks:

1. Extend timeline parsing for instrument-scoped keys.
2. Add minimal discrete trigger support.
3. Keep interpolation for continuous controls.
4. Preserve compatibility with current timeline files.

Success criteria:

- the timeline can activate instruments, change their levels and tuning, and
  trigger one-shots

### Phase 7: GUI Preparation

Goal:
shape the internal model so a GUI can later edit it directly.

Tasks:

1. Introduce structured arrangement data types.
2. Make the text timeline an import / export layer rather than the only model.
3. Keep engine state serializable in a future-friendly way.

Success criteria:

- future GUI work can target structured Rust data rather than rewriting parser
  internals

## First Concrete Deliverable

The best immediate milestone is:

`instrument architecture + tuning abstraction + one new pad instrument`

That gives the highest leverage because it:

- keeps the current project stable
- opens the door for more sounds
- makes future pulse and sampler work easier
- avoids overcommitting to a GUI or event system too early

Recommended order for the first implementation batch:

1. add `tuning.rs`
2. add `instruments/`
3. wrap the current drone system as an instrument
4. add one new pad type
5. make the timeline capable of selecting or addressing that instrument

## Things To Avoid For Now

The following are intentionally not priorities in this phase:

- MIDI input
- live performance features
- a full DAW-style sequencer
- preset browser / patch library UI
- granular synthesis
- plugin architecture

These may all become useful later, but they are not the current bottleneck.
The current bottleneck is sound palette, tuning structure, and arrangement
control.

## Open Questions

These decisions can wait until implementation starts, but they should be kept
visible:

- Should the current noise and event layers remain standalone instruments, or
  should they become optional subcomponents of instrument presets?
- Should samples be loaded only at startup / render start, or should the
  system support dynamic asset loading later?
- Should instrument selection be represented by a Rust enum first, or by a
  registry-like string key system?
- How much of the current macro model should remain global versus becoming
  per-instrument?

## Definition Of Success

This phase is successful when Drone Garden can do all of the following in one
piece:

- run two or more distinct pad / drone instruments
- tune those instruments differently or place them in different registers
- add a pulsing layer
- trigger one-shot sample accents
- render the result offline with the same behavior heard in realtime
- express the arrangement in a text composition format that can later map into
  a GUI model
