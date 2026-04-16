# Project TODO

## Done

- CSP core with propagation and backtracking.
- Initial constraints:
  - `MaxRun`
  - `ExactCount`
  - `MinDensityWindow`
  - `Implication`
  - `SlowChange`
- Additional constraints:
  - `MinCount`
  - `MaxCount`
  - `DifferentAdjacent`
  - `AntiRepeatWindow`
  - `PhaseResponse`
  - `MoreTrueThan`
  - `AtLeastCollisions`
- Config-driven pieces.
- Longer pieces via `steps`, `step_seconds`, and `tail_seconds`.
- Section-scoped constraints.
- Density-based counts.
- Metadata JSON output.
- Metadata scan command.
- Batch rendering by preset.
- Batch rendering by config with `--batch-config`.
- Composition docs.
- Config validation pass for sections, voice references, render preset names, and render values.
- Solver diagnostics:
  - variable count
  - constraint count
  - solve time
  - node / decision / backtrack counts
- Refactor into modules:
  - `csp`
  - `constraints`
  - `grid`
  - `presets`
  - `workflow`
  - `builder`
  - `metadata`
  - `render/*`
- Stereo rendering.
- Delay, drive, brightness, roughness, and sustain controls.
- Section-based render automation.
- Per-voice sound mapping.
- Render modes:
  - `percussive`
  - `impact-kit`
  - `techno-pulse`
  - `drone`
  - `broken-radio`
  - `metallic`
  - `noise-organ`
  - `granular-dust`
  - `sub-machine`
  - `glass-harmonics`
- Named sound presets:
  - `buried-engine`
  - `glass-insects`
  - `static-ash`
  - `radio-wound`
  - `organ-fog`
  - `metal-splinters`
  - `low-ritual`
  - `distant-drone`
- Example pieces:
  - `pieces/ritual-machines.toml`
  - `pieces/collapse-atlas.toml`
  - `pieces/shard-rain.toml`
  - `pieces/frost-letters.toml`
  - `pieces/underforge.toml`
  - `pieces/strike-garden.toml`
  - `pieces/impact-assembly.toml`
  - `pieces/pressure-loop.toml`
  - `pieces/floor-burn.toml`
  - `pieces/pump-column.toml`
  - `pieces/reservoir-bloom.toml`
  - `pieces/breach-cascade.toml`

## Good Next Work

1. Scan Improvements
   - Add section-aware analysis:
     - events per section
     - collisions per section
     - voice density per section
     - longest silence
   - This is the best next step because batching is useful only if scan can help choose interesting renders.

2. Preset Listing CLI
   - Add:
     ```bash
     cargo run -- --list-presets
     cargo run -- --list-render-presets
     cargo run -- --list-render-modes
     ```
   - This makes the tool easier to use without opening docs.

3. More Constraints
   - `MaxGlobalSilence`
   - `VoiceFollows`
   - `Alternation`
   - `NoRepeatExactTuple`
   - `RegisterContour`
   - `TimbrePalettePerSection`
   - `Min/MaxCollisionsPerSection`

4. Render Output Controls
   - Add output normalization options:
     - `limit`
     - `peak-normalize`
     - `raw`
   - Add master gain.
   - Maybe add 24-bit or float WAV later.

5. More Sound Presets
   - Preset families:
     - low/sub
     - brittle/glass
     - dust/noise
     - radio/crushed
     - drone/fog

6. MIDI / OSC / Event Export
   - Export solved event data as:
     - JSON event list
     - MIDI
     - OSC-style JSON
   - Useful for driving external synths later.

7. Solver Improvements
   - Prefer `active` vars before parameter vars during branching.
   - Add optional solve budget / timeout reporting.
   - Add per-constraint or per-scope diagnostics for hard configs.

## Recommended Order

1. Scan Improvements
2. Preset Listing CLI
3. Solver Improvements
