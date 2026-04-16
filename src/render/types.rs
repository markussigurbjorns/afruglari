use crate::grid::EventDurationMode;

#[derive(Clone, Debug, PartialEq)]
pub struct RenderConfig {
    pub sample_rate: u32,
    pub step_seconds: f32,
    pub tail_seconds: f32,
    pub mode: RenderMode,
    pub stereo_width: f32,
    pub delay_mix: f32,
    pub delay_feedback: f32,
    pub delay_seconds: f32,
    pub pump_amount: f32,
    pub pump_release: f32,
    pub pump_lowpass_hz: f32,
    pub pump_key_voice: Option<usize>,
    pub accent_pattern: AccentPattern,
    pub accent_amount: f32,
    pub event_duration_mode: EventDurationMode,
    pub max_event_duration_steps: usize,
    pub drive: f32,
    pub brightness: f32,
    pub roughness: f32,
    pub sustain: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderSection {
    pub start_step: usize,
    pub end_step: usize,
    pub overrides: RenderOverride,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderVoice {
    pub voice: usize,
    pub overrides: RenderOverride,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderOverride {
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
pub enum AccentPattern {
    Constant,
    Steps(Vec<u8>),
}

impl AccentPattern {
    pub fn value_at(&self, step: usize) -> f32 {
        match self {
            Self::Constant => 1.0,
            Self::Steps(values) => {
                let value = values[step % values.len()] as f32;
                (value / 100.0).clamp(0.0, 2.0)
            }
        }
    }
}

impl Default for AccentPattern {
    fn default() -> Self {
        Self::Constant
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderMode {
    Percussive,
    ImpactKit,
    TechnoPulse,
    Drone,
    BrokenRadio,
    Metallic,
    NoiseOrgan,
    GranularDust,
    SubMachine,
    GlassHarmonics,
}

pub fn parse_render_mode(name: &str) -> Option<RenderMode> {
    match name {
        "percussive" => Some(RenderMode::Percussive),
        "impact-kit" | "impact" | "kit" => Some(RenderMode::ImpactKit),
        "techno-pulse" | "techno" | "pulse" => Some(RenderMode::TechnoPulse),
        "drone" => Some(RenderMode::Drone),
        "broken-radio" | "radio" => Some(RenderMode::BrokenRadio),
        "metallic" => Some(RenderMode::Metallic),
        "noise-organ" | "organ" => Some(RenderMode::NoiseOrgan),
        "granular-dust" | "dust" => Some(RenderMode::GranularDust),
        "sub-machine" | "sub" => Some(RenderMode::SubMachine),
        "glass-harmonics" | "glass" => Some(RenderMode::GlassHarmonics),
        _ => None,
    }
}

pub fn parse_accent_pattern(value: &str) -> Option<AccentPattern> {
    match value {
        "flat" | "none" | "constant" => Some(AccentPattern::Constant),
        "four-on-floor" | "four_floor" => Some(AccentPattern::Steps(vec![100, 62, 82, 62])),
        "offbeat" => Some(AccentPattern::Steps(vec![72, 112, 72, 112])),
        "backbeat" => Some(AccentPattern::Steps(vec![100, 72, 118, 74])),
        other => parse_custom_accent_pattern(other),
    }
}

pub fn parse_event_duration_mode(value: &str) -> Option<EventDurationMode> {
    match value {
        "single" | "single-step" | "step" => Some(EventDurationMode::SingleStep),
        "merge" | "merge-adjacent" | "legato" => Some(EventDurationMode::MergeAdjacent),
        _ => None,
    }
}

fn parse_custom_accent_pattern(value: &str) -> Option<AccentPattern> {
    let normalized = value.replace(',', " ");
    let steps = normalized
        .split_whitespace()
        .map(|part| part.parse::<u8>().ok())
        .collect::<Option<Vec<_>>>()?;
    if steps.is_empty() {
        None
    } else {
        Some(AccentPattern::Steps(steps))
    }
}

pub fn render_mode_name(mode: RenderMode) -> &'static str {
    match mode {
        RenderMode::Percussive => "percussive",
        RenderMode::ImpactKit => "impact-kit",
        RenderMode::TechnoPulse => "techno-pulse",
        RenderMode::Drone => "drone",
        RenderMode::BrokenRadio => "broken-radio",
        RenderMode::Metallic => "metallic",
        RenderMode::NoiseOrgan => "noise-organ",
        RenderMode::GranularDust => "granular-dust",
        RenderMode::SubMachine => "sub-machine",
        RenderMode::GlassHarmonics => "glass-harmonics",
    }
}

pub fn render_preset(name: &str) -> Option<RenderOverride> {
    match name {
        "buried-engine" => Some(RenderOverride {
            mode: Some(RenderMode::SubMachine),
            stereo_width: Some(0.42),
            drive: Some(1.30),
            brightness: Some(0.58),
            roughness: Some(0.95),
            sustain: Some(3.00),
            ..RenderOverride::default()
        }),
        "glass-insects" => Some(RenderOverride {
            mode: Some(RenderMode::GlassHarmonics),
            stereo_width: Some(0.95),
            drive: Some(1.18),
            brightness: Some(1.45),
            roughness: Some(1.70),
            sustain: Some(0.72),
            ..RenderOverride::default()
        }),
        "static-ash" => Some(RenderOverride {
            mode: Some(RenderMode::GranularDust),
            stereo_width: Some(1.00),
            drive: Some(1.45),
            brightness: Some(1.55),
            roughness: Some(2.25),
            sustain: Some(0.65),
            ..RenderOverride::default()
        }),
        "radio-wound" => Some(RenderOverride {
            mode: Some(RenderMode::BrokenRadio),
            stereo_width: Some(1.00),
            drive: Some(1.60),
            brightness: Some(1.45),
            roughness: Some(2.10),
            sustain: Some(0.55),
            ..RenderOverride::default()
        }),
        "organ-fog" => Some(RenderOverride {
            mode: Some(RenderMode::NoiseOrgan),
            stereo_width: Some(0.72),
            drive: Some(1.15),
            brightness: Some(0.82),
            roughness: Some(1.05),
            sustain: Some(2.15),
            ..RenderOverride::default()
        }),
        "metal-splinters" => Some(RenderOverride {
            mode: Some(RenderMode::Metallic),
            stereo_width: Some(0.92),
            drive: Some(1.22),
            brightness: Some(1.28),
            roughness: Some(1.65),
            sustain: Some(0.80),
            ..RenderOverride::default()
        }),
        "low-ritual" => Some(RenderOverride {
            mode: Some(RenderMode::SubMachine),
            stereo_width: Some(0.55),
            drive: Some(1.20),
            brightness: Some(0.72),
            roughness: Some(0.85),
            sustain: Some(2.40),
            ..RenderOverride::default()
        }),
        "distant-drone" => Some(RenderOverride {
            mode: Some(RenderMode::Drone),
            stereo_width: Some(0.58),
            drive: Some(1.05),
            brightness: Some(0.52),
            roughness: Some(0.58),
            sustain: Some(4.00),
            ..RenderOverride::default()
        }),
        _ => None,
    }
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44_100,
            step_seconds: 0.16,
            tail_seconds: 1.5,
            mode: RenderMode::Percussive,
            stereo_width: 0.75,
            delay_mix: 0.12,
            delay_feedback: 0.28,
            delay_seconds: 0.33,
            pump_amount: 0.0,
            pump_release: 0.18,
            pump_lowpass_hz: 180.0,
            pump_key_voice: None,
            accent_pattern: AccentPattern::Constant,
            accent_amount: 0.0,
            event_duration_mode: EventDurationMode::SingleStep,
            max_event_duration_steps: 1,
            drive: 1.15,
            brightness: 1.0,
            roughness: 1.0,
            sustain: 1.0,
        }
    }
}

impl RenderOverride {
    pub(crate) fn apply_to(&self, config: &mut RenderConfig) {
        if let Some(mode) = self.mode {
            config.mode = mode;
        }
        if let Some(stereo_width) = self.stereo_width {
            config.stereo_width = stereo_width;
        }
        if let Some(accent_pattern) = &self.accent_pattern {
            config.accent_pattern = accent_pattern.clone();
        }
        if let Some(accent_amount) = self.accent_amount {
            config.accent_amount = accent_amount;
        }
        if let Some(drive) = self.drive {
            config.drive = drive;
        }
        if let Some(brightness) = self.brightness {
            config.brightness = brightness;
        }
        if let Some(roughness) = self.roughness {
            config.roughness = roughness;
        }
        if let Some(sustain) = self.sustain {
            config.sustain = sustain;
        }
    }
}
