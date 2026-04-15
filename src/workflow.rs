use crate::constraints::{
    AntiRepeatWindow, AtLeastCollisions, DifferentAdjacent, ExactCount, MaxCount, MaxRun, MinCount,
    MinDensityWindow, MoreTrueThan, PhaseResponse, SlowChange,
};
use crate::csp::{Engine, Value, solve_with_seed};
use crate::grid::{Event, Grid, Param, events_from_grid};
use crate::presets::{PiecePreset, piece_from_preset};
use crate::render::{RenderConfig, RenderMode, render_events_to_wav};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct GenerationConfig {
    pub preset: PiecePreset,
    pub piece: Option<PieceConfig>,
    pub sections: Vec<SectionConfig>,
    pub constraints: Vec<ConstraintConfig>,
    pub seed: u64,
    pub output: PathBuf,
    pub render: RenderConfig,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PieceConfig {
    pub voices: usize,
    pub steps: usize,
    pub registers: u8,
    pub timbres: u8,
    pub intensities: u8,
    pub sections: Vec<SectionConfig>,
}

impl Default for PieceConfig {
    fn default() -> Self {
        Self {
            voices: 3,
            steps: 32,
            registers: 4,
            timbres: 6,
            intensities: 5,
            sections: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SectionConfig {
    pub name: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StepRange {
    start: usize,
    end: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstraintConfig {
    pub fields: BTreeMap<String, String>,
}

impl ConstraintConfig {
    fn new() -> Self {
        Self {
            fields: BTreeMap::new(),
        }
    }

    fn required(&self, key: &str) -> Result<&str, GenerateError> {
        self.fields
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| GenerateError::Config(format!("constraint missing '{key}'")))
    }

    fn optional(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }
}

impl Default for GenerationConfig {
    fn default() -> Self {
        let preset = PiecePreset::Example;
        let seed = 0;
        Self {
            preset,
            piece: None,
            sections: Vec::new(),
            constraints: Vec::new(),
            seed,
            output: default_output_path(preset, seed),
            render: RenderConfig::default(),
        }
    }
}

impl GenerationConfig {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, GenerateError> {
        let source = fs::read_to_string(path.as_ref()).map_err(GenerateError::Io)?;
        Self::parse(&source)
    }

    pub fn parse(source: &str) -> Result<Self, GenerateError> {
        let mut config = Self::default();
        let mut output_was_set = false;
        let mut section = String::new();
        let mut current_constraint = None;
        let mut current_section = None;

        for (line_index, raw_line) in source.lines().enumerate() {
            let without_comment = raw_line.split_once('#').map_or(raw_line, |(left, _)| left);
            let line = without_comment.trim();
            if line.is_empty() {
                continue;
            }

            if line.starts_with("[[") && line.ends_with("]]") {
                section = line[2..line.len() - 2].trim().to_string();
                match section.as_str() {
                    "constraint" => {
                        config.constraints.push(ConstraintConfig::new());
                        current_constraint = Some(config.constraints.len() - 1);
                        current_section = None;
                    }
                    "section" => {
                        config.sections.push(SectionConfig {
                            name: String::new(),
                            start: 0,
                            end: 0,
                        });
                        current_section = Some(config.sections.len() - 1);
                        current_constraint = None;
                    }
                    _ => {
                        return Err(GenerateError::Config(format!(
                            "unsupported array section '[[{section}]]'"
                        )));
                    }
                }
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].trim().to_string();
                current_constraint = None;
                current_section = None;
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                return Err(GenerateError::Config(format!(
                    "line {} is not a key/value pair",
                    line_index + 1
                )));
            };
            let full_key = if section.is_empty() {
                key.trim().to_string()
            } else {
                format!("{}.{}", section, key.trim())
            };
            let value = value.trim();

            if section == "constraint" {
                let Some(index) = current_constraint else {
                    return Err(GenerateError::Config(format!(
                        "line {} uses [constraint]; use [[constraint]]",
                        line_index + 1
                    )));
                };
                config.constraints[index]
                    .fields
                    .insert(key.trim().to_string(), parse_string(value)?);
                continue;
            }

            if section == "section" {
                let Some(index) = current_section else {
                    return Err(GenerateError::Config(format!(
                        "line {} uses [section]; use [[section]]",
                        line_index + 1
                    )));
                };
                match key.trim() {
                    "name" => config.sections[index].name = parse_string(value)?,
                    "start" => config.sections[index].start = parse_usize(value)?,
                    "end" => config.sections[index].end = parse_usize(value)?,
                    other => {
                        return Err(GenerateError::Config(format!(
                            "unknown section key '{other}'"
                        )));
                    }
                }
                continue;
            }

            match full_key.as_str() {
                "piece.preset" | "preset" => {
                    config.preset = PiecePreset::parse(&parse_string(value)?).ok_or_else(|| {
                        GenerateError::Config(format!(
                            "unknown preset '{}'",
                            parse_string(value).unwrap_or_default()
                        ))
                    })?;
                }
                "piece.seed" | "seed" => {
                    config.seed = parse_u64(value)?;
                }
                "piece.voices" | "voices" => {
                    custom_piece(&mut config).voices = parse_usize(value)?;
                }
                "piece.steps" | "steps" => {
                    custom_piece(&mut config).steps = parse_usize(value)?;
                }
                "piece.registers" | "registers" => {
                    custom_piece(&mut config).registers = parse_u8(value)?;
                }
                "piece.timbres" | "timbres" => {
                    custom_piece(&mut config).timbres = parse_u8(value)?;
                }
                "piece.intensities" | "intensities" => {
                    custom_piece(&mut config).intensities = parse_u8(value)?;
                }
                "piece.output" | "output" => {
                    config.output = PathBuf::from(parse_string(value)?);
                    output_was_set = true;
                }
                "piece.render_mode" | "render.mode" | "render_mode" => {
                    config.render.mode =
                        parse_render_mode(&parse_string(value)?).ok_or_else(|| {
                            GenerateError::Config(format!(
                                "unknown render mode '{}'",
                                parse_string(value).unwrap_or_default()
                            ))
                        })?;
                }
                "render.sample_rate" | "sample_rate" => {
                    config.render.sample_rate = parse_u32(value)?;
                }
                "render.step_seconds" | "step_seconds" => {
                    config.render.step_seconds = parse_f32(value)?;
                }
                "render.tail_seconds" | "tail_seconds" => {
                    config.render.tail_seconds = parse_f32(value)?;
                }
                "render.stereo_width" | "stereo_width" => {
                    config.render.stereo_width = parse_f32(value)?;
                }
                "render.delay_mix" | "delay_mix" => {
                    config.render.delay_mix = parse_f32(value)?;
                }
                "render.delay_feedback" | "delay_feedback" => {
                    config.render.delay_feedback = parse_f32(value)?;
                }
                "render.delay_seconds" | "delay_seconds" => {
                    config.render.delay_seconds = parse_f32(value)?;
                }
                "render.drive" | "drive" => {
                    config.render.drive = parse_f32(value)?;
                }
                other => {
                    return Err(GenerateError::Config(format!(
                        "unknown config key '{other}'"
                    )));
                }
            }
        }

        if !output_was_set {
            config.output = default_output_path(config.preset, config.seed);
        }

        Ok(config)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationMetadata {
    pub piece: String,
    pub preset: PiecePreset,
    pub seed: u64,
    pub render_mode: RenderMode,
    pub output: PathBuf,
    pub events: usize,
    pub collisions: usize,
    pub voice_density: Vec<usize>,
}

impl GenerationMetadata {
    pub fn json_path(&self) -> PathBuf {
        self.output.with_extension("json")
    }

    pub fn to_json(&self) -> String {
        let densities = self
            .voice_density
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{{\n  \"preset\": \"{}\",\n  \"seed\": {},\n  \"render_mode\": \"{}\",\n  \"output\": \"{}\",\n  \"events\": {},\n  \"collisions\": {},\n  \"voice_density\": [{}]\n}}\n",
            escape_json(&self.piece),
            self.seed,
            render_mode_name(self.render_mode),
            escape_json(&self.output.display().to_string()),
            self.events,
            self.collisions,
            densities
        )
    }

    pub fn parse_json(source: &str) -> Result<Self, GenerateError> {
        let piece = json_string(source, "preset")?;
        let seed = json_u64(source, "seed")?;
        let render_mode = parse_render_mode(&json_string(source, "render_mode")?)
            .ok_or_else(|| GenerateError::Config("metadata has unknown render_mode".to_string()))?;
        let output = PathBuf::from(json_string(source, "output")?);
        let events = json_usize(source, "events")?;
        let collisions = json_usize(source, "collisions")?;
        let voice_density = json_usize_array(source, "voice_density")?;
        let preset = PiecePreset::parse(&piece).unwrap_or(PiecePreset::Example);

        Ok(Self {
            piece,
            preset,
            seed,
            render_mode,
            output,
            events,
            collisions,
            voice_density,
        })
    }
}

#[derive(Clone, Debug)]
pub struct GenerateResult {
    pub metadata: GenerationMetadata,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MetadataFilter {
    pub min_collisions: Option<usize>,
    pub max_collisions: Option<usize>,
    pub min_events: Option<usize>,
    pub max_events: Option<usize>,
    pub voice_dominates: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanEntry {
    pub metadata_path: PathBuf,
    pub metadata: GenerationMetadata,
}

#[derive(Debug)]
pub enum GenerateError {
    Config(String),
    Io(io::Error),
    NoSolution { piece: String, seed: u64 },
}

impl fmt::Display for GenerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(message) => write!(f, "config error: {message}"),
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::NoSolution { piece, seed } => {
                write!(f, "no solution for piece '{piece}' with seed {seed}")
            }
        }
    }
}

impl std::error::Error for GenerateError {}

impl From<io::Error> for GenerateError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn generate_from_config(path: impl AsRef<Path>) -> Result<GenerateResult, GenerateError> {
    let config = GenerationConfig::from_file(path)?;
    generate_one(&config)
}

pub fn generate_one(config: &GenerationConfig) -> Result<GenerateResult, GenerateError> {
    let (grid, mut engine) = build_piece(config)?;

    if !solve_with_seed(&mut engine, config.seed) {
        return Err(GenerateError::NoSolution {
            piece: piece_name(config).to_string(),
            seed: config.seed,
        });
    }

    let events = events_from_grid(&engine, &grid);
    ensure_parent_dir(&config.output)?;
    render_events_to_wav(&events, &config.output, config.render)?;

    let metadata = metadata_for_events(config, &events);
    let metadata_path = metadata.json_path();
    ensure_parent_dir(&metadata_path)?;
    fs::write(&metadata_path, metadata.to_json())?;

    Ok(GenerateResult { metadata })
}

pub fn generate_batch(
    preset: PiecePreset,
    count: usize,
    output_dir: impl AsRef<Path>,
    render_mode: Option<RenderMode>,
) -> Result<Vec<GenerateResult>, GenerateError> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir)?;

    let mut results = Vec::with_capacity(count);
    for seed in 0..count as u64 {
        let output = output_dir.join(format!("{}-seed-{:03}.wav", preset.name(), seed));
        let mut render = RenderConfig::default();
        render.mode = render_mode.unwrap_or_else(|| default_mode_for_preset(preset));
        let config = GenerationConfig {
            preset,
            piece: None,
            sections: Vec::new(),
            constraints: Vec::new(),
            seed,
            output,
            render,
        };
        results.push(generate_one(&config)?);
    }

    Ok(results)
}

pub fn scan_metadata(
    dir: impl AsRef<Path>,
    filter: &MetadataFilter,
) -> Result<Vec<ScanEntry>, GenerateError> {
    let mut files = Vec::new();
    collect_metadata_files(dir.as_ref(), &mut files)?;
    files.sort();

    let mut entries = Vec::new();
    for path in files {
        let source = fs::read_to_string(&path)?;
        let metadata = GenerationMetadata::parse_json(&source)?;
        if metadata_matches(&metadata, filter) {
            entries.push(ScanEntry {
                metadata_path: path,
                metadata,
            });
        }
    }

    Ok(entries)
}

pub fn parse_render_mode(name: &str) -> Option<RenderMode> {
    match name {
        "percussive" => Some(RenderMode::Percussive),
        "drone" => Some(RenderMode::Drone),
        "broken-radio" | "radio" => Some(RenderMode::BrokenRadio),
        "metallic" => Some(RenderMode::Metallic),
        "noise-organ" | "organ" => Some(RenderMode::NoiseOrgan),
        _ => None,
    }
}

pub fn render_mode_name(mode: RenderMode) -> &'static str {
    match mode {
        RenderMode::Percussive => "percussive",
        RenderMode::Drone => "drone",
        RenderMode::BrokenRadio => "broken-radio",
        RenderMode::Metallic => "metallic",
        RenderMode::NoiseOrgan => "noise-organ",
    }
}

pub fn default_mode_for_preset(preset: PiecePreset) -> RenderMode {
    match preset {
        PiecePreset::Example => RenderMode::Percussive,
        PiecePreset::SparseCracks => RenderMode::BrokenRadio,
        PiecePreset::DenseCollisionField => RenderMode::Percussive,
        PiecePreset::SlowNoiseBlocks => RenderMode::Drone,
        PiecePreset::MetallicSwarm => RenderMode::Metallic,
    }
}

fn metadata_for_events(config: &GenerationConfig, events: &[Event]) -> GenerationMetadata {
    let voices = events
        .iter()
        .map(|event| event.voice)
        .max()
        .map_or(0, |voice| voice + 1);
    let mut voice_density = vec![0; voices];
    for event in events {
        voice_density[event.voice] += 1;
    }

    let collisions = events
        .iter()
        .enumerate()
        .map(|(index, event)| {
            events[index + 1..]
                .iter()
                .filter(|other| other.step == event.step && other.voice != event.voice)
                .count()
        })
        .sum();

    GenerationMetadata {
        piece: piece_name(config).to_string(),
        preset: config.preset,
        seed: config.seed,
        render_mode: config.render.mode,
        output: config.output.clone(),
        events: events.len(),
        collisions,
        voice_density,
    }
}

fn collect_metadata_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), GenerateError> {
    if dir.is_file() {
        if dir.extension().is_some_and(|extension| extension == "json") {
            files.push(dir.to_path_buf());
        }
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_metadata_files(&path, files)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            files.push(path);
        }
    }

    Ok(())
}

fn metadata_matches(metadata: &GenerationMetadata, filter: &MetadataFilter) -> bool {
    if filter
        .min_collisions
        .is_some_and(|min| metadata.collisions < min)
    {
        return false;
    }
    if filter
        .max_collisions
        .is_some_and(|max| metadata.collisions > max)
    {
        return false;
    }
    if filter.min_events.is_some_and(|min| metadata.events < min) {
        return false;
    }
    if filter.max_events.is_some_and(|max| metadata.events > max) {
        return false;
    }
    if let Some(voice) = filter.voice_dominates {
        let Some(&target) = metadata.voice_density.get(voice) else {
            return false;
        };
        if metadata
            .voice_density
            .iter()
            .enumerate()
            .any(|(index, &count)| index != voice && count >= target)
        {
            return false;
        }
    }

    true
}

fn build_piece(config: &GenerationConfig) -> Result<(Grid, Engine), GenerateError> {
    if config.piece.is_none() && config.constraints.is_empty() {
        return Ok(piece_from_preset(config.preset));
    }

    let mut piece = config.piece.clone().unwrap_or_default();
    piece.sections = config.sections.clone();
    let grid = Grid::new(piece.voices, piece.steps);
    let mut engine = Engine::new(grid.domains(piece.registers, piece.timbres, piece.intensities));

    for constraint in &config.constraints {
        add_configured_constraint(&mut engine, &grid, &piece, constraint)?;
    }

    Ok((grid, engine))
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
    if range.start + offset >= range.end {
        return Ok(Vec::new());
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
    let mut range = if let Some(section_name) = constraint.optional("section") {
        piece
            .sections
            .iter()
            .find(|section| section.name == section_name)
            .map(|section| StepRange {
                start: section.start,
                end: section.end,
            })
            .ok_or_else(|| GenerateError::Config(format!("unknown section '{section_name}'")))?
    } else {
        StepRange {
            start: 0,
            end: piece.steps,
        }
    };

    if let Some(start) = constraint.optional("start") {
        range.start = parse_usize(start)?;
    }
    if let Some(end) = constraint.optional("end") {
        range.end = parse_usize(end)?;
    }

    if range.start >= range.end || range.end > piece.steps {
        return Err(GenerateError::Config(format!(
            "invalid step range {}..{} for {} steps",
            range.start, range.end, piece.steps
        )));
    }

    Ok(range)
}

fn custom_piece(config: &mut GenerationConfig) -> &mut PieceConfig {
    config.piece.get_or_insert_with(PieceConfig::default)
}

fn piece_name(config: &GenerationConfig) -> &str {
    if config.piece.is_some() || !config.constraints.is_empty() {
        "custom"
    } else {
        config.preset.name()
    }
}

fn required_param(constraint: &ConstraintConfig, key: &str) -> Result<Param, GenerateError> {
    parse_param(constraint.required(key)?)
}

fn optional_param(
    constraint: &ConstraintConfig,
    key: &str,
    default: Param,
) -> Result<Param, GenerateError> {
    constraint.optional(key).map_or(Ok(default), parse_param)
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

    let density = constraint
        .optional("density")
        .ok_or_else(|| GenerateError::Config("constraint missing 'count' or 'density'".to_string()))
        .and_then(parse_f32)?;

    if !(0.0..=1.0).contains(&density) {
        return Err(GenerateError::Config(format!(
            "density must be between 0.0 and 1.0, got {density}"
        )));
    }

    Ok((scope_len as f32 * density).round() as usize)
}

fn parse_value(value: &str) -> Result<Value, GenerateError> {
    match value {
        "true" => Ok(Value::Bool(true)),
        "false" => Ok(Value::Bool(false)),
        _ => Ok(Value::Int(parse_u8(value)?)),
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
        keys.join(", ")
    )))
}

fn parse_string(value: &str) -> Result<String, GenerateError> {
    let trimmed = value.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        Ok(trimmed[1..trimmed.len() - 1].to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

fn json_string(source: &str, key: &str) -> Result<String, GenerateError> {
    let value = json_value(source, key)?;
    let value = value.trim();
    if !value.starts_with('"') {
        return Err(GenerateError::Config(format!(
            "metadata key '{key}' is not a string"
        )));
    }

    let mut escaped = false;
    let mut output = String::new();
    for character in value[1..].chars() {
        if escaped {
            output.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Ok(output);
        } else {
            output.push(character);
        }
    }

    Err(GenerateError::Config(format!(
        "metadata key '{key}' has unterminated string"
    )))
}

fn json_u64(source: &str, key: &str) -> Result<u64, GenerateError> {
    json_value(source, key)?
        .trim_matches(',')
        .trim()
        .parse()
        .map_err(|_| GenerateError::Config(format!("metadata key '{key}' is not an integer")))
}

fn json_usize(source: &str, key: &str) -> Result<usize, GenerateError> {
    json_value(source, key)?
        .trim_matches(',')
        .trim()
        .parse()
        .map_err(|_| GenerateError::Config(format!("metadata key '{key}' is not an integer")))
}

fn json_usize_array(source: &str, key: &str) -> Result<Vec<usize>, GenerateError> {
    let value = json_value(source, key)?;
    let start = value
        .find('[')
        .ok_or_else(|| GenerateError::Config(format!("metadata key '{key}' is not an array")))?;
    let end = value
        .find(']')
        .ok_or_else(|| GenerateError::Config(format!("metadata key '{key}' is not an array")))?;
    let inner = value[start + 1..end].trim();
    if inner.is_empty() {
        return Ok(Vec::new());
    }

    inner
        .split(',')
        .map(|value| {
            value.trim().parse().map_err(|_| {
                GenerateError::Config(format!("metadata key '{key}' contains a non-integer"))
            })
        })
        .collect()
}

fn json_value<'a>(source: &'a str, key: &str) -> Result<&'a str, GenerateError> {
    let needle = format!("\"{key}\"");
    let key_start = source
        .find(&needle)
        .ok_or_else(|| GenerateError::Config(format!("metadata missing key '{key}'")))?;
    let after_key = &source[key_start + needle.len()..];
    let colon = after_key
        .find(':')
        .ok_or_else(|| GenerateError::Config(format!("metadata key '{key}' has no value")))?;
    let after_colon = after_key[colon + 1..].trim_start();
    let line_end = after_colon.find('\n').unwrap_or(after_colon.len());
    Ok(after_colon[..line_end].trim().trim_end_matches(','))
}

fn parse_u64(value: &str) -> Result<u64, GenerateError> {
    value
        .parse()
        .map_err(|_| GenerateError::Config(format!("expected unsigned integer, got '{value}'")))
}

fn parse_usize(value: &str) -> Result<usize, GenerateError> {
    value
        .parse()
        .map_err(|_| GenerateError::Config(format!("expected unsigned integer, got '{value}'")))
}

fn parse_u8(value: &str) -> Result<u8, GenerateError> {
    value
        .parse()
        .map_err(|_| GenerateError::Config(format!("expected small integer, got '{value}'")))
}

fn parse_u32(value: &str) -> Result<u32, GenerateError> {
    value
        .parse()
        .map_err(|_| GenerateError::Config(format!("expected unsigned integer, got '{value}'")))
}

fn parse_f32(value: &str) -> Result<f32, GenerateError> {
    value
        .parse()
        .map_err(|_| GenerateError::Config(format!("expected number, got '{value}'")))
}

fn default_output_path(preset: PiecePreset, seed: u64) -> PathBuf {
    PathBuf::from(format!("target/{}-{}.wav", preset.name(), seed))
}

fn ensure_parent_dir(path: &Path) -> Result<(), GenerateError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
