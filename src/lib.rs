pub mod constraints;
pub mod csp;
pub mod grid;
pub mod presets;
pub mod render;
pub mod workflow;

pub use constraints::{
    AntiRepeatWindow, AtLeastCollisions, DifferentAdjacent, ExactCount, Implication, Literal,
    MaxCount, MaxRun, MinCount, MinDensityWindow, MoreTrueThan, PhaseResponse, SlowChange,
};
pub use csp::{Conflict, Constraint, Domain, Engine, Value, VarId, solve, solve_with_seed};
pub use grid::{Event, Grid, Param, events_from_grid};
pub use presets::{PiecePreset, example_piece, piece_from_preset, preset_names};
pub use render::{
    RenderConfig, RenderMode, RenderOverride, RenderSection, RenderVoice, render_events_to_wav,
    render_events_to_wav_with_automation, render_events_to_wav_with_sections,
};
pub use workflow::{
    ConstraintConfig, GenerateError, GenerateResult, GenerationConfig, GenerationMetadata,
    MetadataFilter, PieceConfig, ScanEntry, SectionConfig, SectionRenderConfig, VoiceRenderConfig,
    generate_batch, generate_batch_from_config, generate_from_config, generate_one, scan_metadata,
};
