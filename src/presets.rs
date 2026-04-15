use crate::constraints::{
    AtLeastCollisions, ExactCount, MaxRun, MinDensityWindow, MoreTrueThan, SlowChange,
};
use crate::csp::{Engine, Value};
use crate::grid::{Grid, Param};

pub fn example_piece() -> (Grid, Engine) {
    piece_from_preset(PiecePreset::Example)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PiecePreset {
    Example,
    SparseCracks,
    DenseCollisionField,
    SlowNoiseBlocks,
    MetallicSwarm,
}

impl PiecePreset {
    pub fn name(self) -> &'static str {
        match self {
            Self::Example => "example",
            Self::SparseCracks => "sparse-cracks",
            Self::DenseCollisionField => "dense-collision-field",
            Self::SlowNoiseBlocks => "slow-noise-blocks",
            Self::MetallicSwarm => "metallic-swarm",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "example" => Some(Self::Example),
            "sparse-cracks" | "sparse" => Some(Self::SparseCracks),
            "dense-collision-field" | "dense" => Some(Self::DenseCollisionField),
            "slow-noise-blocks" | "slow" => Some(Self::SlowNoiseBlocks),
            "metallic-swarm" | "metallic" => Some(Self::MetallicSwarm),
            _ => None,
        }
    }
}

pub fn preset_names() -> &'static [&'static str] {
    &[
        "example",
        "sparse-cracks",
        "dense-collision-field",
        "slow-noise-blocks",
        "metallic-swarm",
    ]
}

pub fn piece_from_preset(preset: PiecePreset) -> (Grid, Engine) {
    match preset {
        PiecePreset::Example => example_piece_inner(),
        PiecePreset::SparseCracks => sparse_cracks(),
        PiecePreset::DenseCollisionField => dense_collision_field(),
        PiecePreset::SlowNoiseBlocks => slow_noise_blocks(),
        PiecePreset::MetallicSwarm => metallic_swarm(),
    }
}

fn example_piece_inner() -> (Grid, Engine) {
    let grid = Grid::new(3, 32);
    let mut engine = Engine::new(grid.domains(4, 6, 5));

    for voice in 0..3 {
        engine.add_constraint(MaxRun::new(grid.voice_param(voice, Param::Active), 3));
    }

    engine.add_constraint(MinDensityWindow::new(grid.all_active(), 9, 1));
    engine.add_constraint(ExactCount::new(grid.all_active(), Value::Bool(true), 28));

    let collisions = (0..32)
        .map(|step| {
            (
                grid.var(1, step, Param::Active),
                grid.var(2, step, Param::Active),
            )
        })
        .collect();
    engine.add_constraint(AtLeastCollisions::new(collisions, 2));
    engine.add_constraint(SlowChange::new(grid.voice_param(2, Param::Timbre), 4));

    let voice_0_first_half = (0..16)
        .map(|step| grid.var(0, step, Param::Active))
        .collect();
    let voice_0_second_half = (16..32)
        .map(|step| grid.var(0, step, Param::Active))
        .collect();
    engine.add_constraint(MoreTrueThan::new(voice_0_first_half, voice_0_second_half));

    (grid, engine)
}

fn sparse_cracks() -> (Grid, Engine) {
    let grid = Grid::new(3, 48);
    let mut engine = Engine::new(grid.domains(5, 8, 6));

    for voice in 0..3 {
        engine.add_constraint(MaxRun::new(grid.voice_param(voice, Param::Active), 1));
        engine.add_constraint(SlowChange::new(grid.voice_param(voice, Param::Register), 6));
    }

    engine.add_constraint(MinDensityWindow::new(grid.all_active(), 18, 1));
    engine.add_constraint(ExactCount::new(grid.all_active(), Value::Bool(true), 20));
    engine.add_constraint(ExactCount::new(
        grid.voice_param(1, Param::Timbre),
        Value::Int(8),
        5,
    ));

    let collisions = (0..48)
        .map(|step| {
            (
                grid.var(0, step, Param::Active),
                grid.var(2, step, Param::Active),
            )
        })
        .collect();
    engine.add_constraint(AtLeastCollisions::new(collisions, 1));

    (grid, engine)
}

fn dense_collision_field() -> (Grid, Engine) {
    let grid = Grid::new(3, 32);
    let mut engine = Engine::new(grid.domains(4, 7, 6));

    for voice in 0..3 {
        engine.add_constraint(MaxRun::new(grid.voice_param(voice, Param::Active), 6));
    }

    engine.add_constraint(MinDensityWindow::new(grid.all_active(), 6, 2));
    engine.add_constraint(ExactCount::new(grid.all_active(), Value::Bool(true), 58));

    for pair in [(0, 1), (1, 2)] {
        let collisions = (0..32)
            .map(|step| {
                (
                    grid.var(pair.0, step, Param::Active),
                    grid.var(pair.1, step, Param::Active),
                )
            })
            .collect();
        engine.add_constraint(AtLeastCollisions::new(collisions, 10));
    }

    let first_half = (0..16)
        .map(|step| grid.var(1, step, Param::Active))
        .collect();
    let second_half = (16..32)
        .map(|step| grid.var(1, step, Param::Active))
        .collect();
    engine.add_constraint(MoreTrueThan::new(first_half, second_half));
    engine.add_constraint(SlowChange::new(grid.voice_param(2, Param::Timbre), 2));

    (grid, engine)
}

fn slow_noise_blocks() -> (Grid, Engine) {
    let grid = Grid::new(3, 32);
    let mut engine = Engine::new(grid.domains(5, 6, 6));

    for voice in 0..3 {
        engine.add_constraint(MaxRun::new(grid.voice_param(voice, Param::Active), 8));
        engine.add_constraint(SlowChange::new(grid.voice_param(voice, Param::Timbre), 8));
        engine.add_constraint(SlowChange::new(
            grid.voice_param(voice, Param::Intensity),
            8,
        ));
    }

    engine.add_constraint(MinDensityWindow::new(grid.all_active(), 24, 1));
    engine.add_constraint(ExactCount::new(grid.all_active(), Value::Bool(true), 58));
    engine.add_constraint(ExactCount::new(
        grid.voice_param(0, Param::Active),
        Value::Bool(true),
        12,
    ));
    engine.add_constraint(ExactCount::new(
        grid.voice_param(1, Param::Active),
        Value::Bool(true),
        22,
    ));
    engine.add_constraint(ExactCount::new(
        grid.voice_param(2, Param::Active),
        Value::Bool(true),
        24,
    ));

    (grid, engine)
}

fn metallic_swarm() -> (Grid, Engine) {
    let grid = Grid::new(3, 32);
    let mut engine = Engine::new(grid.domains(6, 9, 5));

    for voice in 0..3 {
        engine.add_constraint(MaxRun::new(grid.voice_param(voice, Param::Active), 2));
    }

    engine.add_constraint(MinDensityWindow::new(grid.all_active(), 6, 1));
    engine.add_constraint(ExactCount::new(grid.all_active(), Value::Bool(true), 44));
    engine.add_constraint(ExactCount::new(
        grid.voice_param(1, Param::Active),
        Value::Bool(true),
        18,
    ));
    engine.add_constraint(ExactCount::new(
        grid.voice_param(1, Param::Timbre),
        Value::Int(9),
        7,
    ));

    for pair in [(0, 1), (1, 2)] {
        let collisions = (0..32)
            .map(|step| {
                (
                    grid.var(pair.0, step, Param::Active),
                    grid.var(pair.1, step, Param::Active),
                )
            })
            .collect();
        engine.add_constraint(AtLeastCollisions::new(collisions, 6));
    }

    (grid, engine)
}
