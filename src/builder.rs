use crate::constraints::{
    AntiRepeatWindow, AtLeastCollisions, DifferentAdjacent, ExactCount, MaxCount, MaxRun, MinCount,
    MinDensityWindow, MoreTrueThan, PhaseResponse, SlowChange,
};
use crate::csp::{Engine, Value};
use crate::grid::{Grid, Param};
use crate::presets::piece_from_preset;
use crate::workflow::{ConstraintConfig, GenerateError, GenerationConfig, PieceConfig};
use crate::{Implication, Literal};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StepRange {
    start: usize,
    end: usize,
}

pub(crate) fn build_piece(config: &GenerationConfig) -> Result<(Grid, Engine), GenerateError> {
    if config.piece.is_none() && config.constraints.is_empty() {
        return Ok(piece_from_preset(config.preset));
    }

    let mut piece = config.piece.clone().unwrap_or_default();
    piece.sections = config.sections.clone();
    let grid = Grid::new(piece.voices, piece.steps);
    let mut engine = Engine::new(grid.domains(piece.registers, piece.timbres, piece.intensities));

    add_inactive_param_defaults(&mut engine, &grid, &piece);

    for constraint in &config.constraints {
        add_configured_constraint(&mut engine, &grid, &piece, constraint)?;
    }

    Ok((grid, engine))
}

fn add_inactive_param_defaults(engine: &mut Engine, grid: &Grid, piece: &PieceConfig) {
    for voice in 0..piece.voices {
        for step in 0..piece.steps {
            let inactive = Literal {
                var: grid.var(voice, step, Param::Active),
                value: Value::Bool(false),
            };
            for param in [Param::Register, Param::Timbre, Param::Intensity] {
                engine.add_constraint(Implication::new(
                    inactive,
                    Literal {
                        var: grid.var(voice, step, param),
                        value: Value::Int(0),
                    },
                ));
            }
        }
    }
}

fn add_configured_constraint(
    engine: &mut Engine,
    grid: &Grid,
    piece: &PieceConfig,
    constraint: &ConstraintConfig,
) -> Result<(), GenerateError> {
    match constraint.required("type")? {
        "max-run" => {
            let voice = required_usize(constraint, "voice")?;
            let param = optional_param(constraint, "param", Param::Active)?;
            let len = required_usize(constraint, "len")?;
            engine.add_constraint(MaxRun::new(
                voice_param_scope(grid, piece, constraint, voice, param)?,
                len,
            ));
        }
        "exact-count" => {
            let param = optional_param(constraint, "param", Param::Active)?;
            let scope = constraint_scope(grid, piece, constraint, param)?;
            let value = required_value(constraint, "value")?;
            let count = count_or_density(constraint, scope.len())?;
            engine.add_constraint(ExactCount::new(scope, value, count));
        }
        "min-count" => {
            let param = optional_param(constraint, "param", Param::Active)?;
            let scope = constraint_scope(grid, piece, constraint, param)?;
            let value = required_value(constraint, "value")?;
            let count = count_or_density(constraint, scope.len())?;
            engine.add_constraint(MinCount::new(scope, value, count));
        }
        "max-count" => {
            let param = optional_param(constraint, "param", Param::Active)?;
            let scope = constraint_scope(grid, piece, constraint, param)?;
            let value = required_value(constraint, "value")?;
            let count = count_or_density(constraint, scope.len())?;
            engine.add_constraint(MaxCount::new(scope, value, count));
        }
        "min-density-window" => {
            let param = optional_param(constraint, "param", Param::Active)?;
            let scope = constraint_scope(grid, piece, constraint, param)?;
            let window = required_usize(constraint, "window")?;
            let min = required_usize_any(constraint, &["min", "count"])?;
            engine.add_constraint(MinDensityWindow::new(scope, window, min));
        }
        "different-adjacent" => {
            let voice = required_usize(constraint, "voice")?;
            let param = required_param(constraint, "param")?;
            engine.add_constraint(DifferentAdjacent::new(voice_param_scope(
                grid, piece, constraint, voice, param,
            )?));
        }
        "anti-repeat-window" => {
            let voice = required_usize(constraint, "voice")?;
            let param = required_param(constraint, "param")?;
            let window = required_usize(constraint, "window")?;
            let max_repeats = required_usize_any(constraint, &["max_repeats", "max"])?;
            engine.add_constraint(AntiRepeatWindow::new(
                voice_param_scope(grid, piece, constraint, voice, param)?,
                window,
                max_repeats,
            ));
        }
        "slow-change" => {
            let voice = required_usize(constraint, "voice")?;
            let param = required_param(constraint, "param")?;
            let window = required_usize(constraint, "window")?;
            engine.add_constraint(SlowChange::new(
                voice_param_scope(grid, piece, constraint, voice, param)?,
                window,
            ));
        }
        "at-least-collisions" => {
            let voice_a = required_usize_any(constraint, &["voice_a", "a"])?;
            let voice_b = required_usize_any(constraint, &["voice_b", "b"])?;
            let count = required_usize(constraint, "count")?;
            let range = constraint_range(piece, constraint)?;
            let pairs = (range.start..range.end)
                .map(|step| {
                    (
                        grid.var(voice_a, step, Param::Active),
                        grid.var(voice_b, step, Param::Active),
                    )
                })
                .collect();
            engine.add_constraint(AtLeastCollisions::new(pairs, count));
        }
        "phase-response" => {
            let voice_a = required_usize_any(constraint, &["voice_a", "a"])?;
            let voice_b = required_usize_any(constraint, &["voice_b", "b"])?;
            let offset = required_usize(constraint, "offset")?;
            let count = required_usize_any(constraint, &["min", "count"])?;
            let pairs = phase_pairs(grid, piece, constraint, voice_a, voice_b, offset)?;
            engine.add_constraint(PhaseResponse::new(pairs, count));
        }
        "more-true-than" => {
            let left_voice = required_usize_any(constraint, &["left_voice", "voice_a", "a"])?;
            let right_voice = required_usize_any(constraint, &["right_voice", "voice_b", "b"])?;
            let param = optional_param(constraint, "param", Param::Active)?;
            engine.add_constraint(MoreTrueThan::new(
                voice_param_scope(grid, piece, constraint, left_voice, param)?,
                voice_param_scope(grid, piece, constraint, right_voice, param)?,
            ));
        }
        other => {
            return Err(GenerateError::Config(format!(
                "unsupported constraint type '{other}'"
            )));
        }
    }

    Ok(())
}

fn constraint_scope(
    grid: &Grid,
    piece: &PieceConfig,
    constraint: &ConstraintConfig,
    param: Param,
) -> Result<Vec<crate::VarId>, GenerateError> {
    let range = constraint_range(piece, constraint)?;
    if let Some(voice) = constraint.optional("voice") {
        let voice = parse_usize(voice)?;
        return Ok((range.start..range.end)
            .map(|step| grid.var(voice, step, param))
            .collect());
    }

    let mut vars = Vec::with_capacity(piece.voices * (range.end - range.start));
    for step in range.start..range.end {
        for voice in 0..piece.voices {
            vars.push(grid.var(voice, step, param));
        }
    }
    Ok(vars)
}

fn voice_param_scope(
    grid: &Grid,
    piece: &PieceConfig,
    constraint: &ConstraintConfig,
    voice: usize,
    param: Param,
) -> Result<Vec<crate::VarId>, GenerateError> {
    let range = constraint_range(piece, constraint)?;
    Ok((range.start..range.end)
        .map(|step| grid.var(voice, step, param))
        .collect())
}

fn phase_pairs(
    grid: &Grid,
    piece: &PieceConfig,
    constraint: &ConstraintConfig,
    voice_a: usize,
    voice_b: usize,
    offset: usize,
) -> Result<Vec<(crate::VarId, crate::VarId)>, GenerateError> {
    let range = constraint_range(piece, constraint)?;
    if offset == 0 {
        return Err(GenerateError::Config(
            "phase-response offset must be greater than 0".to_string(),
        ));
    }
    if range.start + offset >= range.end {
        return Err(GenerateError::Config(
            "phase-response offset leaves no pairs in scope".to_string(),
        ));
    }

    Ok((range.start..range.end - offset)
        .map(|step| {
            (
                grid.var(voice_a, step, Param::Active),
                grid.var(voice_b, step + offset, Param::Active),
            )
        })
        .collect())
}

fn constraint_range(
    piece: &PieceConfig,
    constraint: &ConstraintConfig,
) -> Result<StepRange, GenerateError> {
    if let Some(section_name) = constraint.optional("section") {
        let section = piece
            .sections
            .iter()
            .find(|section| section.name == section_name)
            .ok_or_else(|| {
                GenerateError::Config(format!("unknown section '{section_name}' in constraint"))
            })?;
        return Ok(StepRange {
            start: section.start,
            end: section.end,
        });
    }

    let start = constraint
        .optional("start")
        .map(parse_usize)
        .transpose()?
        .unwrap_or(0);
    let end = constraint
        .optional("end")
        .map(parse_usize)
        .transpose()?
        .unwrap_or(piece.steps);
    if start > end || end > piece.steps {
        return Err(GenerateError::Config(format!(
            "invalid step range {start}..{end} for piece with {} steps",
            piece.steps
        )));
    }

    Ok(StepRange { start, end })
}

fn required_param(constraint: &ConstraintConfig, key: &str) -> Result<Param, GenerateError> {
    parse_param(constraint.required(key)?)
}

fn optional_param(
    constraint: &ConstraintConfig,
    key: &str,
    default: Param,
) -> Result<Param, GenerateError> {
    constraint
        .optional(key)
        .map(parse_param)
        .transpose()
        .map(|param| param.unwrap_or(default))
}

fn parse_param(value: &str) -> Result<Param, GenerateError> {
    match value {
        "active" => Ok(Param::Active),
        "register" => Ok(Param::Register),
        "timbre" => Ok(Param::Timbre),
        "intensity" => Ok(Param::Intensity),
        _ => Err(GenerateError::Config(format!("unknown param '{value}'"))),
    }
}

fn required_value(constraint: &ConstraintConfig, key: &str) -> Result<Value, GenerateError> {
    parse_value(constraint.required(key)?)
}

fn count_or_density(
    constraint: &ConstraintConfig,
    scope_len: usize,
) -> Result<usize, GenerateError> {
    if let Some(count) = constraint.optional("count") {
        return parse_usize(count);
    }
    if let Some(density) = constraint.optional("density") {
        let density = parse_f32(density)?;
        if !(0.0..=1.0).contains(&density) {
            return Err(GenerateError::Config(format!(
                "density must be between 0.0 and 1.0, got {density}"
            )));
        }
        return Ok((scope_len as f32 * density).round() as usize);
    }
    Err(GenerateError::Config(
        "constraint requires 'count' or 'density'".to_string(),
    ))
}

fn parse_value(value: &str) -> Result<Value, GenerateError> {
    match value {
        "true" => Ok(Value::Bool(true)),
        "false" => Ok(Value::Bool(false)),
        _ => value
            .parse::<u8>()
            .map(Value::Int)
            .map_err(|_| GenerateError::Config(format!("unknown value '{value}'"))),
    }
}

fn required_usize(constraint: &ConstraintConfig, key: &str) -> Result<usize, GenerateError> {
    parse_usize(constraint.required(key)?)
}

fn required_usize_any(
    constraint: &ConstraintConfig,
    keys: &[&str],
) -> Result<usize, GenerateError> {
    for key in keys {
        if let Some(value) = constraint.optional(key) {
            return parse_usize(value);
        }
    }
    Err(GenerateError::Config(format!(
        "constraint missing one of '{}'",
        keys.join("', '")
    )))
}

fn parse_usize(value: &str) -> Result<usize, GenerateError> {
    value
        .parse()
        .map_err(|_| GenerateError::Config(format!("expected unsigned integer, got '{value}'")))
}

fn parse_f32(value: &str) -> Result<f32, GenerateError> {
    value
        .parse()
        .map_err(|_| GenerateError::Config(format!("expected float, got '{value}'")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csp::Value;
    use crate::workflow::GenerationConfig;

    #[test]
    fn inactive_cells_force_params_to_zero() {
        let config = GenerationConfig {
            piece: Some(PieceConfig {
                voices: 1,
                steps: 2,
                registers: 4,
                timbres: 4,
                intensities: 4,
                sections: Vec::new(),
            }),
            ..GenerationConfig::default()
        };

        let (grid, mut engine) = build_piece(&config).unwrap();
        engine
            .assign(grid.var(0, 0, Param::Active), Value::Bool(false))
            .unwrap();
        engine.propagate_all().unwrap();

        assert_eq!(
            engine.value(grid.var(0, 0, Param::Register)),
            Some(Value::Int(0))
        );
        assert_eq!(
            engine.value(grid.var(0, 0, Param::Timbre)),
            Some(Value::Int(0))
        );
        assert_eq!(
            engine.value(grid.var(0, 0, Param::Intensity)),
            Some(Value::Int(0))
        );
    }
}
