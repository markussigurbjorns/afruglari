use crate::builder::build_piece;
use crate::csp::solve_with_seed;
use crate::grid::{Event, events_from_grid_with_durations};
use crate::metadata::GenerationMetadata;
use crate::presets::PiecePreset;
use crate::render::{
    AccentPattern, RenderConfig, RenderMode, RenderOverride, RenderSection, RenderVoice,
    parse_accent_pattern, parse_event_duration_mode, parse_render_mode,
    render_events_to_wav_with_automation, render_mode_name, render_preset,
};
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
    pub section_renders: Vec<SectionRenderConfig>,
    pub voice_renders: Vec<VoiceRenderConfig>,
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SectionRenderConfig {
    pub section: String,
    pub preset: Option<String>,
    pub mode: Option<RenderMode>,
    pub stereo_width: Option<f32>,
    pub accent_pattern: Option<AccentPattern>,
    pub accent_amount: Option<f32>,
    pub drive: Option<f32>,
    pub brightness: Option<f32>,
    pub roughness: Option<f32>,
    pub sustain: Option<f32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VoiceRenderConfig {
    pub voice: usize,
    pub preset: Option<String>,
    pub mode: Option<RenderMode>,
    pub stereo_width: Option<f32>,
    pub accent_pattern: Option<AccentPattern>,
    pub accent_amount: Option<f32>,
    pub drive: Option<f32>,
    pub brightness: Option<f32>,
    pub roughness: Option<f32>,
    pub sustain: Option<f32>,
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

    pub(crate) fn required(&self, key: &str) -> Result<&str, GenerateError> {
        self.fields
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| GenerateError::Config(format!("constraint missing '{key}'")))
    }

    pub(crate) fn optional(&self, key: &str) -> Option<&str> {
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
            section_renders: Vec::new(),
            voice_renders: Vec::new(),
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
        let mut current_section_render = None;
        let mut current_voice_render = None;

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
                        current_section_render = None;
                        current_voice_render = None;
                    }
                    "section" => {
                        config.sections.push(SectionConfig {
                            name: String::new(),
                            start: 0,
                            end: 0,
                        });
                        current_section = Some(config.sections.len() - 1);
                        current_constraint = None;
                        current_section_render = None;
                        current_voice_render = None;
                    }
                    "section_render" => {
                        config.section_renders.push(SectionRenderConfig::default());
                        current_section_render = Some(config.section_renders.len() - 1);
                        current_constraint = None;
                        current_section = None;
                        current_voice_render = None;
                    }
                    "voice_render" => {
                        config.voice_renders.push(VoiceRenderConfig::default());
                        current_voice_render = Some(config.voice_renders.len() - 1);
                        current_constraint = None;
                        current_section = None;
                        current_section_render = None;
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
                current_section_render = None;
                current_voice_render = None;
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

            if section == "section_render" {
                let Some(index) = current_section_render else {
                    return Err(GenerateError::Config(format!(
                        "line {} uses [section_render]; use [[section_render]]",
                        line_index + 1
                    )));
                };
                let section_render = &mut config.section_renders[index];
                match key.trim() {
                    "section" => section_render.section = parse_string(value)?,
                    "preset" => section_render.preset = Some(parse_string(value)?),
                    "mode" | "render_mode" => {
                        section_render.mode =
                            Some(parse_render_mode(&parse_string(value)?).ok_or_else(|| {
                                GenerateError::Config(format!(
                                    "unknown render mode '{}'",
                                    parse_string(value).unwrap_or_default()
                                ))
                            })?);
                    }
                    "stereo_width" => section_render.stereo_width = Some(parse_f32(value)?),
                    "accent_pattern" => {
                        section_render.accent_pattern =
                            Some(parse_accent(value, "section_render accent_pattern")?)
                    }
                    "accent_amount" => section_render.accent_amount = Some(parse_f32(value)?),
                    "drive" => section_render.drive = Some(parse_f32(value)?),
                    "brightness" => section_render.brightness = Some(parse_f32(value)?),
                    "roughness" => section_render.roughness = Some(parse_f32(value)?),
                    "sustain" => section_render.sustain = Some(parse_f32(value)?),
                    other => {
                        return Err(GenerateError::Config(format!(
                            "unknown section_render key '{other}'"
                        )));
                    }
                }
                continue;
            }

            if section == "voice_render" {
                let Some(index) = current_voice_render else {
                    return Err(GenerateError::Config(format!(
                        "line {} uses [voice_render]; use [[voice_render]]",
                        line_index + 1
                    )));
                };
                let voice_render = &mut config.voice_renders[index];
                match key.trim() {
                    "voice" => voice_render.voice = parse_usize(value)?,
                    "preset" => voice_render.preset = Some(parse_string(value)?),
                    "mode" | "render_mode" => {
                        voice_render.mode =
                            Some(parse_render_mode(&parse_string(value)?).ok_or_else(|| {
                                GenerateError::Config(format!(
                                    "unknown render mode '{}'",
                                    parse_string(value).unwrap_or_default()
                                ))
                            })?);
                    }
                    "stereo_width" => voice_render.stereo_width = Some(parse_f32(value)?),
                    "accent_pattern" => {
                        voice_render.accent_pattern =
                            Some(parse_accent(value, "voice_render accent_pattern")?)
                    }
                    "accent_amount" => voice_render.accent_amount = Some(parse_f32(value)?),
                    "drive" => voice_render.drive = Some(parse_f32(value)?),
                    "brightness" => voice_render.brightness = Some(parse_f32(value)?),
                    "roughness" => voice_render.roughness = Some(parse_f32(value)?),
                    "sustain" => voice_render.sustain = Some(parse_f32(value)?),
                    other => {
                        return Err(GenerateError::Config(format!(
                            "unknown voice_render key '{other}'"
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
                "render.accent_pattern" | "accent_pattern" => {
                    config.render.accent_pattern = parse_accent(value, "accent_pattern")?;
                }
                "render.accent_amount" | "accent_amount" => {
                    config.render.accent_amount = parse_f32(value)?;
                }
                "render.event_duration_mode" | "event_duration_mode" => {
                    config.render.event_duration_mode =
                        parse_event_duration_mode(&parse_string(value)?).ok_or_else(|| {
                            GenerateError::Config(format!(
                                "unknown event_duration_mode '{}'",
                                parse_string(value).unwrap_or_default()
                            ))
                        })?;
                }
                "render.max_event_duration_steps" | "max_event_duration_steps" => {
                    config.render.max_event_duration_steps = parse_usize(value)?;
                }
                "render.pump_amount" | "pump_amount" => {
                    config.render.pump_amount = parse_f32(value)?;
                }
                "render.pump_release" | "pump_release" => {
                    config.render.pump_release = parse_f32(value)?;
                }
                "render.pump_lowpass_hz" | "pump_lowpass_hz" => {
                    config.render.pump_lowpass_hz = parse_f32(value)?;
                }
                "render.pump_key_voice" | "pump_key_voice" => {
                    config.render.pump_key_voice = parse_pump_key_voice(value)?;
                }
                "render.drive" | "drive" => {
                    config.render.drive = parse_f32(value)?;
                }
                "render.brightness" | "brightness" => {
                    config.render.brightness = parse_f32(value)?;
                }
                "render.roughness" | "roughness" => {
                    config.render.roughness = parse_f32(value)?;
                }
                "render.sustain" | "sustain" => {
                    config.render.sustain = parse_f32(value)?;
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

#[derive(Clone, Debug)]
pub struct GenerateResult {
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

    let events = split_events_at_section_boundaries(
        events_from_grid_with_durations(
            &engine,
            &grid,
            config.render.event_duration_mode,
            config.render.max_event_duration_steps,
        ),
        &config.sections,
    );
    ensure_parent_dir(&config.output)?;
    let render_voices = resolve_render_voices(config)?;
    let render_sections = resolve_render_sections(config)?;
    render_events_to_wav_with_automation(
        &events,
        &config.output,
        config.render.clone(),
        &render_voices,
        &render_sections,
    )?;

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
            section_renders: Vec::new(),
            voice_renders: Vec::new(),
            constraints: Vec::new(),
            seed,
            output,
            render,
        };
        results.push(generate_one(&config)?);
    }

    Ok(results)
}

pub fn generate_batch_from_config(
    path: impl AsRef<Path>,
    count: usize,
    output_dir: impl AsRef<Path>,
) -> Result<Vec<GenerateResult>, GenerateError> {
    let base_config = GenerationConfig::from_file(path.as_ref())?;
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir)?;

    let stem = path
        .as_ref()
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_else(|| piece_name(&base_config));

    let mut results = Vec::with_capacity(count);
    for offset in 0..count as u64 {
        let seed = base_config.seed + offset;
        let mut config = base_config.clone();
        config.seed = seed;
        config.output = output_dir.join(format!("{stem}-seed-{seed:03}.wav"));
        results.push(generate_one(&config)?);
    }

    Ok(results)
}

fn resolve_render_voices(config: &GenerationConfig) -> Result<Vec<RenderVoice>, GenerateError> {
    config
        .voice_renders
        .iter()
        .map(|voice_render| {
            Ok(RenderVoice {
                voice: voice_render.voice,
                overrides: voice_render_overrides(voice_render)?,
            })
        })
        .collect()
}

fn resolve_render_sections(config: &GenerationConfig) -> Result<Vec<RenderSection>, GenerateError> {
    let mut sections = Vec::with_capacity(config.section_renders.len());

    for section_render in &config.section_renders {
        let section = config
            .sections
            .iter()
            .find(|section| section.name == section_render.section)
            .ok_or_else(|| {
                GenerateError::Config(format!(
                    "section_render references unknown section '{}'",
                    section_render.section
                ))
            })?;

        sections.push(RenderSection {
            start_step: section.start,
            end_step: section.end,
            overrides: section_render_overrides(section_render)?,
        });
    }

    Ok(sections)
}

fn section_render_overrides(
    section_render: &SectionRenderConfig,
) -> Result<RenderOverride, GenerateError> {
    let mut overrides = preset_override(section_render.preset.as_deref())?;
    apply_explicit_render_fields(
        &mut overrides,
        section_render.mode,
        section_render.stereo_width,
        section_render.accent_pattern.as_ref(),
        section_render.accent_amount,
        section_render.drive,
        section_render.brightness,
        section_render.roughness,
        section_render.sustain,
    );
    Ok(overrides)
}

fn voice_render_overrides(
    voice_render: &VoiceRenderConfig,
) -> Result<RenderOverride, GenerateError> {
    let mut overrides = preset_override(voice_render.preset.as_deref())?;
    apply_explicit_render_fields(
        &mut overrides,
        voice_render.mode,
        voice_render.stereo_width,
        voice_render.accent_pattern.as_ref(),
        voice_render.accent_amount,
        voice_render.drive,
        voice_render.brightness,
        voice_render.roughness,
        voice_render.sustain,
    );
    Ok(overrides)
}

fn preset_override(preset: Option<&str>) -> Result<RenderOverride, GenerateError> {
    let Some(preset) = preset else {
        return Ok(RenderOverride::default());
    };
    render_preset(preset)
        .ok_or_else(|| GenerateError::Config(format!("unknown render preset '{preset}'")))
}

fn apply_explicit_render_fields(
    overrides: &mut RenderOverride,
    mode: Option<RenderMode>,
    stereo_width: Option<f32>,
    accent_pattern: Option<&AccentPattern>,
    accent_amount: Option<f32>,
    drive: Option<f32>,
    brightness: Option<f32>,
    roughness: Option<f32>,
    sustain: Option<f32>,
) {
    if mode.is_some() {
        overrides.mode = mode;
    }
    if stereo_width.is_some() {
        overrides.stereo_width = stereo_width;
    }
    if let Some(accent_pattern) = accent_pattern {
        overrides.accent_pattern = Some(accent_pattern.clone());
    }
    if accent_amount.is_some() {
        overrides.accent_amount = accent_amount;
    }
    if drive.is_some() {
        overrides.drive = drive;
    }
    if brightness.is_some() {
        overrides.brightness = brightness;
    }
    if roughness.is_some() {
        overrides.roughness = roughness;
    }
    if sustain.is_some() {
        overrides.sustain = sustain;
    }
}

fn split_events_at_section_boundaries(
    events: Vec<Event>,
    sections: &[SectionConfig],
) -> Vec<Event> {
    if sections.is_empty() {
        return events;
    }

    let mut boundaries = sections
        .iter()
        .map(|section| section.end)
        .collect::<Vec<_>>();
    boundaries.sort_unstable();
    boundaries.dedup();

    let mut split = Vec::with_capacity(events.len());
    for event in events {
        let mut step = event.step;
        let event_end = event.step + event.duration_steps;

        while step < event_end {
            let next_boundary = boundaries
                .iter()
                .copied()
                .find(|boundary| *boundary > step && *boundary < event_end)
                .unwrap_or(event_end);
            split.push(Event {
                voice: event.voice,
                step,
                duration_steps: next_boundary - step,
                register: event.register,
                timbre: event.timbre,
                intensity: event.intensity,
            });
            step = next_boundary;
        }
    }

    split
}

fn voice_render_summaries(config: &GenerationConfig) -> Vec<String> {
    config
        .voice_renders
        .iter()
        .map(|voice_render| {
            format!(
                "voice {}{}",
                voice_render.voice,
                render_override_summary(
                    voice_render.preset.as_deref(),
                    voice_render_overrides(voice_render).unwrap_or_default(),
                )
            )
        })
        .collect()
}

fn section_render_summaries(config: &GenerationConfig) -> Vec<String> {
    config
        .section_renders
        .iter()
        .map(|section_render| {
            format!(
                "section {}{}",
                section_render.section,
                render_override_summary(
                    section_render.preset.as_deref(),
                    section_render_overrides(section_render).unwrap_or_default(),
                )
            )
        })
        .collect()
}

fn render_override_summary(preset: Option<&str>, overrides: RenderOverride) -> String {
    let mut fields = Vec::new();
    if let Some(preset) = preset {
        fields.push(format!("preset={preset}"));
    }
    if let Some(mode) = overrides.mode {
        fields.push(format!("mode={}", render_mode_name(mode)));
    }
    if let Some(stereo_width) = overrides.stereo_width {
        fields.push(format!("stereo_width={stereo_width:.2}"));
    }
    if let Some(accent_pattern) = &overrides.accent_pattern {
        fields.push(format!(
            "accent_pattern={}",
            accent_pattern_name(accent_pattern)
        ));
    }
    if let Some(accent_amount) = overrides.accent_amount {
        fields.push(format!("accent_amount={accent_amount:.2}"));
    }
    if let Some(drive) = overrides.drive {
        fields.push(format!("drive={drive:.2}"));
    }
    if let Some(brightness) = overrides.brightness {
        fields.push(format!("brightness={brightness:.2}"));
    }
    if let Some(roughness) = overrides.roughness {
        fields.push(format!("roughness={roughness:.2}"));
    }
    if let Some(sustain) = overrides.sustain {
        fields.push(format!("sustain={sustain:.2}"));
    }

    if fields.is_empty() {
        String::new()
    } else {
        format!(" ({})", fields.join(", "))
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
        voice_render_count: config.voice_renders.len(),
        section_render_count: config.section_renders.len(),
        voice_renders: voice_render_summaries(config),
        section_renders: section_render_summaries(config),
    }
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

fn accent_pattern_name(pattern: &AccentPattern) -> String {
    match pattern {
        AccentPattern::Constant => "constant".to_string(),
        AccentPattern::Steps(values) => values
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join("-"),
    }
}

fn parse_accent(value: &str, context: &str) -> Result<AccentPattern, GenerateError> {
    let parsed = parse_string(value)?;
    parse_accent_pattern(&parsed).ok_or_else(|| {
        GenerateError::Config(format!(
            "{context} expects a named pattern or space/comma separated percentages"
        ))
    })
}

fn parse_pump_key_voice(value: &str) -> Result<Option<usize>, GenerateError> {
    let parsed = parse_string(value)?;
    match parsed.as_str() {
        "none" | "master" => Ok(None),
        _ => parsed.parse::<usize>().map(Some).map_err(|_| {
            GenerateError::Config(format!(
                "pump_key_voice must be 'none', 'master', or a voice index, got '{parsed}'"
            ))
        }),
    }
}

fn parse_string(value: &str) -> Result<String, GenerateError> {
    let trimmed = value.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        Ok(trimmed[1..trimmed.len() - 1].to_string())
    } else {
        Ok(trimmed.to_string())
    }
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
