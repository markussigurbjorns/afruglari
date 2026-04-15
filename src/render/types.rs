#[derive(Clone, Copy, Debug)]
pub struct RenderConfig {
    pub sample_rate: u32,
    pub step_seconds: f32,
    pub tail_seconds: f32,
    pub mode: RenderMode,
    pub stereo_width: f32,
    pub delay_mix: f32,
    pub delay_feedback: f32,
    pub delay_seconds: f32,
    pub drive: f32,
    pub brightness: f32,
    pub roughness: f32,
    pub sustain: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct RenderSection {
    pub start_step: usize,
    pub end_step: usize,
    pub overrides: RenderOverride,
}

#[derive(Clone, Copy, Debug)]
pub struct RenderVoice {
    pub voice: usize,
    pub overrides: RenderOverride,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderOverride {
    pub mode: Option<RenderMode>,
    pub stereo_width: Option<f32>,
    pub drive: Option<f32>,
    pub brightness: Option<f32>,
    pub roughness: Option<f32>,
    pub sustain: Option<f32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderMode {
    Percussive,
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

pub fn render_mode_name(mode: RenderMode) -> &'static str {
    match mode {
        RenderMode::Percussive => "percussive",
        RenderMode::Drone => "drone",
        RenderMode::BrokenRadio => "broken-radio",
        RenderMode::Metallic => "metallic",
        RenderMode::NoiseOrgan => "noise-organ",
        RenderMode::GranularDust => "granular-dust",
        RenderMode::SubMachine => "sub-machine",
        RenderMode::GlassHarmonics => "glass-harmonics",
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
            drive: 1.15,
            brightness: 1.0,
            roughness: 1.0,
            sustain: 1.0,
        }
    }
}

impl RenderOverride {
    pub(crate) fn apply_to(self, config: &mut RenderConfig) {
        if let Some(mode) = self.mode {
            config.mode = mode;
        }
        if let Some(stereo_width) = self.stereo_width {
            config.stereo_width = stereo_width;
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
