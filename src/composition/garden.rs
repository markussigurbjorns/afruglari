use crate::composition::arrangement::SampleTriggerEvent;
use crate::composition::layers::texture::TextureLayer;
use crate::composition::timeline::ControlTimeline;
use crate::composition::tuning::{RegisterRange, TuningConfig};
use crate::dsp::delay::StereoDelay;
use crate::dsp::sample::StereoSample;
use crate::dsp::source::StereoSource;
use crate::instruments::rack::InstrumentRack;
use crate::instruments::sampler::LoadedSampleAsset;
use crate::instruments::InstrumentFamily;

const DEFAULT_SEED: u64 = 0xA7F2_6C91_D04E_11B5;
const DEFAULT_EVENT_ATTACK_MIN_SECONDS: f32 = 0.015;
const DEFAULT_EVENT_ATTACK_MAX_SECONDS: f32 = 0.195;
const DEFAULT_EVENT_DECAY_MIN_SECONDS: f32 = 2.0;
const DEFAULT_EVENT_DECAY_MAX_SECONDS: f32 = 8.0;
const DEFAULT_DRONE_RETUNE_SECONDS: f32 = 9.0;

#[derive(Clone)]
pub struct GardenConfig {
    pub seed: u64,
    pub root_hz: f32,
    pub voice_count: usize,
    pub controls: GardenControls,
    pub sample_assets: Vec<LoadedSampleAsset>,
}

#[derive(Clone, Copy, Debug)]
pub struct InstrumentParams {
    pub(crate) values: [InstrumentParamValue; InstrumentFamily::COUNT],
}

#[derive(Clone, Copy, Debug)]
pub struct DroneParams {
    pub spread: f32,
    pub detune: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct HarmonicParams {
    pub mix: f32,
    pub shimmer: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct PulseParams {
    pub rate: f32,
    pub length: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct NoiseParams {
    pub motion: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SampleParams {
    pub auto_rate: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct TextureParams {
    pub drift: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EventParams;

#[derive(Clone, Copy, Debug)]
pub enum InstrumentParamValue {
    Drone(DroneParams),
    Harmonic(HarmonicParams),
    Pulse(PulseParams),
    Sample(SampleParams),
    Noise(NoiseParams),
    Events(EventParams),
    Texture(TextureParams),
}

#[derive(Clone, Copy, Debug)]
pub struct GardenControls {
    pub density: f32,
    pub brightness: f32,
    pub space: f32,
    pub instability: f32,
    pub drone_level: f32,
    pub harmonic_level: f32,
    pub pulse_level: f32,
    pub sample_level: f32,
    pub noise_level: f32,
    pub event_level: f32,
    pub texture_level: f32,
}

impl Default for GardenControls {
    fn default() -> Self {
        Self {
            density: 0.35,
            brightness: 0.45,
            space: 0.65,
            instability: 0.25,
            drone_level: 1.0,
            harmonic_level: 0.0,
            pulse_level: 0.0,
            sample_level: 0.0,
            noise_level: 0.0,
            event_level: 0.0,
            texture_level: 0.0,
        }
    }
}

impl Default for GardenConfig {
    fn default() -> Self {
        Self {
            seed: DEFAULT_SEED,
            root_hz: 110.0,
            voice_count: 3,
            controls: GardenControls::default(),
            sample_assets: Vec::new(),
        }
    }
}

impl Default for InstrumentParams {
    fn default() -> Self {
        Self {
            values: [
                InstrumentParamValue::Drone(DroneParams {
                    spread: 1.0,
                    detune: 1.0,
                }),
                InstrumentParamValue::Harmonic(HarmonicParams {
                    mix: 1.0,
                    shimmer: 1.0,
                }),
                InstrumentParamValue::Pulse(PulseParams {
                    rate: 1.0,
                    length: 1.0,
                }),
                InstrumentParamValue::Sample(SampleParams { auto_rate: 1.0 }),
                InstrumentParamValue::Noise(NoiseParams { motion: 1.0 }),
                InstrumentParamValue::Events(EventParams),
                InstrumentParamValue::Texture(TextureParams { drift: 1.0 }),
            ],
        }
    }
}

impl InstrumentParams {
    pub fn clamped(self) -> Self {
        let mut values = self.values;
        for family in InstrumentFamily::all() {
            values[family.index()] = values[family.index()].clamped();
        }
        Self { values }
    }

    pub fn value(self, family: InstrumentFamily) -> InstrumentParamValue {
        self.values[family.index()]
    }

    pub fn value_mut(&mut self, family: InstrumentFamily) -> &mut InstrumentParamValue {
        &mut self.values[family.index()]
    }

    pub fn drone(self) -> DroneParams {
        match self.value(InstrumentFamily::Drone) {
            InstrumentParamValue::Drone(params) => params,
            _ => unreachable!("drone params stored in wrong family slot"),
        }
    }

    pub fn drone_mut(&mut self) -> &mut DroneParams {
        match self.value_mut(InstrumentFamily::Drone) {
            InstrumentParamValue::Drone(params) => params,
            _ => unreachable!("drone params stored in wrong family slot"),
        }
    }

    pub fn harmonic(self) -> HarmonicParams {
        match self.value(InstrumentFamily::Harmonic) {
            InstrumentParamValue::Harmonic(params) => params,
            _ => unreachable!("harmonic params stored in wrong family slot"),
        }
    }

    pub fn harmonic_mut(&mut self) -> &mut HarmonicParams {
        match self.value_mut(InstrumentFamily::Harmonic) {
            InstrumentParamValue::Harmonic(params) => params,
            _ => unreachable!("harmonic params stored in wrong family slot"),
        }
    }

    pub fn pulse(self) -> PulseParams {
        match self.value(InstrumentFamily::Pulse) {
            InstrumentParamValue::Pulse(params) => params,
            _ => unreachable!("pulse params stored in wrong family slot"),
        }
    }

    pub fn pulse_mut(&mut self) -> &mut PulseParams {
        match self.value_mut(InstrumentFamily::Pulse) {
            InstrumentParamValue::Pulse(params) => params,
            _ => unreachable!("pulse params stored in wrong family slot"),
        }
    }

    pub fn sample(self) -> SampleParams {
        match self.value(InstrumentFamily::Sample) {
            InstrumentParamValue::Sample(params) => params,
            _ => unreachable!("sample params stored in wrong family slot"),
        }
    }

    pub fn sample_mut(&mut self) -> &mut SampleParams {
        match self.value_mut(InstrumentFamily::Sample) {
            InstrumentParamValue::Sample(params) => params,
            _ => unreachable!("sample params stored in wrong family slot"),
        }
    }

    pub fn noise(self) -> NoiseParams {
        match self.value(InstrumentFamily::Noise) {
            InstrumentParamValue::Noise(params) => params,
            _ => unreachable!("noise params stored in wrong family slot"),
        }
    }

    pub fn noise_mut(&mut self) -> &mut NoiseParams {
        match self.value_mut(InstrumentFamily::Noise) {
            InstrumentParamValue::Noise(params) => params,
            _ => unreachable!("noise params stored in wrong family slot"),
        }
    }

    pub fn events(self) -> EventParams {
        match self.value(InstrumentFamily::Events) {
            InstrumentParamValue::Events(params) => params,
            _ => unreachable!("events params stored in wrong family slot"),
        }
    }

    pub fn texture(self) -> TextureParams {
        match self.value(InstrumentFamily::Texture) {
            InstrumentParamValue::Texture(params) => params,
            _ => unreachable!("texture params stored in wrong family slot"),
        }
    }

    pub fn texture_mut(&mut self) -> &mut TextureParams {
        match self.value_mut(InstrumentFamily::Texture) {
            InstrumentParamValue::Texture(params) => params,
            _ => unreachable!("texture params stored in wrong family slot"),
        }
    }
}

impl InstrumentParamValue {
    pub fn clamped(self) -> Self {
        match self {
            Self::Drone(params) => Self::Drone(params.clamped()),
            Self::Harmonic(params) => Self::Harmonic(params.clamped()),
            Self::Pulse(params) => Self::Pulse(params.clamped()),
            Self::Sample(params) => Self::Sample(params.clamped()),
            Self::Noise(params) => Self::Noise(params.clamped()),
            Self::Events(params) => Self::Events(params),
            Self::Texture(params) => Self::Texture(params.clamped()),
        }
    }
}

impl DroneParams {
    pub fn clamped(self) -> Self {
        Self {
            spread: self.spread.clamp(0.0, 2.0),
            detune: self.detune.clamp(0.0, 2.0),
        }
    }
}

impl HarmonicParams {
    pub fn clamped(self) -> Self {
        Self {
            mix: self.mix.clamp(0.0, 2.0),
            shimmer: self.shimmer.clamp(0.0, 2.0),
        }
    }
}

impl PulseParams {
    pub fn clamped(self) -> Self {
        Self {
            rate: self.rate.clamp(0.25, 4.0),
            length: self.length.clamp(0.25, 4.0),
        }
    }
}

impl NoiseParams {
    pub fn clamped(self) -> Self {
        Self {
            motion: self.motion.clamp(0.0, 2.0),
        }
    }
}

impl SampleParams {
    pub fn clamped(self) -> Self {
        Self {
            auto_rate: self.auto_rate.clamp(0.25, 4.0),
        }
    }
}

impl TextureParams {
    pub fn clamped(self) -> Self {
        Self {
            drift: self.drift.clamp(0.0, 2.0),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EventShapeConfig {
    pub attack_min_seconds: f32,
    pub attack_max_seconds: f32,
    pub decay_min_seconds: f32,
    pub decay_max_seconds: f32,
}

impl EventShapeConfig {
    pub fn new(
        attack_min_seconds: f32,
        attack_max_seconds: f32,
        decay_min_seconds: f32,
        decay_max_seconds: f32,
    ) -> Self {
        Self {
            attack_min_seconds,
            attack_max_seconds,
            decay_min_seconds,
            decay_max_seconds,
        }
        .clamped()
    }

    pub fn clamped(self) -> Self {
        let attack_min_seconds = self.attack_min_seconds.max(0.001);
        let attack_max_seconds = self.attack_max_seconds.max(0.001).max(attack_min_seconds);
        let decay_min_seconds = self.decay_min_seconds.max(0.05);
        let decay_max_seconds = self.decay_max_seconds.max(0.05).max(decay_min_seconds);

        Self {
            attack_min_seconds,
            attack_max_seconds,
            decay_min_seconds,
            decay_max_seconds,
        }
    }
}

pub struct Garden {
    rack: InstrumentRack,
    texture: TextureLayer,
    delay: StereoDelay,
    timeline: Option<ControlTimeline>,
    sample_triggers: Vec<SampleTriggerEvent>,
    next_sample_trigger_index: usize,
    sample_rate: f32,
    sample_index: usize,
    tuning: TuningConfig,
    voice_count: usize,
    register: RegisterRange,
    event_shape: EventShapeConfig,
    drone_retune_seconds: f32,
    instrument_params: InstrumentParams,
}

impl Garden {
    pub fn new(
        sample_rate: f32,
        config: GardenConfig,
        timeline: Option<ControlTimeline>,
        sample_triggers: Vec<SampleTriggerEvent>,
    ) -> Self {
        let controls = config.controls.clamped();
        let tuning = TuningConfig::default_just(config.root_hz);
        let register = RegisterRange::new(1, 2);
        let event_shape = EventShapeConfig::new(
            DEFAULT_EVENT_ATTACK_MIN_SECONDS,
            DEFAULT_EVENT_ATTACK_MAX_SECONDS,
            DEFAULT_EVENT_DECAY_MIN_SECONDS,
            DEFAULT_EVENT_DECAY_MAX_SECONDS,
        );
        let rack = InstrumentRack::new(
            sample_rate,
            config.clone(),
            &tuning,
            register,
            !sample_triggers.is_empty(),
        );
        let texture = TextureLayer::new(sample_rate, controls);
        let delay = StereoDelay::new(
            sample_rate,
            0.73,
            0.97,
            map_range(controls.space, 0.20, 0.68),
            map_range(controls.space, 0.12, 0.55),
            map_range(controls.brightness, 600.0, 5_000.0),
        );

        let mut garden = Self {
            rack,
            texture,
            delay,
            timeline,
            sample_triggers,
            next_sample_trigger_index: 0,
            sample_rate,
            sample_index: 0,
            tuning,
            voice_count: config.voice_count.clamp(1, 12),
            register,
            event_shape,
            drone_retune_seconds: DEFAULT_DRONE_RETUNE_SECONDS,
            instrument_params: InstrumentParams::default(),
        };
        garden.set_octave_range(1, 2);
        garden.set_event_attack_range(
            DEFAULT_EVENT_ATTACK_MIN_SECONDS,
            DEFAULT_EVENT_ATTACK_MAX_SECONDS,
        );
        garden.set_event_decay_range(
            DEFAULT_EVENT_DECAY_MIN_SECONDS,
            DEFAULT_EVENT_DECAY_MAX_SECONDS,
        );
        garden.set_drone_retune_seconds(DEFAULT_DRONE_RETUNE_SECONDS);
        garden.set_instrument_params(InstrumentParams::default());
        garden.set_controls(controls);
        garden
    }

    pub fn voice_count(&self) -> usize {
        self.rack.voice_count()
    }

    pub fn set_controls(&mut self, controls: GardenControls) {
        let controls = controls.clamped();

        self.rack.set_controls(controls);
        self.texture.set_controls(controls);
        self.delay
            .set_feedback(map_range(controls.space, 0.20, 0.68));
        self.delay.set_wet(map_range(controls.space, 0.12, 0.55));
        self.delay
            .set_feedback_cutoff(map_range(controls.brightness, 600.0, 5_000.0));
    }

    pub fn set_root_hz(&mut self, root_hz: f32) {
        let root_hz = root_hz.max(1.0);
        if (self.tuning.root_hz() - root_hz).abs() <= f32::EPSILON {
            return;
        }

        self.tuning = TuningConfig::default_just(root_hz);
        self.rack.set_pitch_field(&self.tuning);
    }

    pub fn set_voice_count(&mut self, voice_count: usize) {
        let voice_count = voice_count.clamp(1, 12);
        if self.voice_count == voice_count {
            return;
        }

        self.voice_count = voice_count;
        self.rack.set_voice_count(voice_count);
    }

    pub fn set_octave_range(&mut self, octave_min: i32, octave_max: i32) {
        let register = RegisterRange::new(octave_min, octave_max);
        if self.register == register {
            return;
        }

        self.register = register;
        self.rack.set_register(register);
    }

    pub fn set_event_decay_range(&mut self, event_decay_min: f32, event_decay_max: f32) {
        let event_shape = EventShapeConfig::new(
            self.event_shape.attack_min_seconds,
            self.event_shape.attack_max_seconds,
            event_decay_min,
            event_decay_max,
        );
        if self.event_shape.decay_min_seconds == event_shape.decay_min_seconds
            && self.event_shape.decay_max_seconds == event_shape.decay_max_seconds
        {
            return;
        }

        self.event_shape = event_shape;
        self.rack.set_event_shape(event_shape);
    }

    pub fn set_event_attack_range(&mut self, event_attack_min: f32, event_attack_max: f32) {
        let event_shape = EventShapeConfig::new(
            event_attack_min,
            event_attack_max,
            self.event_shape.decay_min_seconds,
            self.event_shape.decay_max_seconds,
        );
        if self.event_shape.attack_min_seconds == event_shape.attack_min_seconds
            && self.event_shape.attack_max_seconds == event_shape.attack_max_seconds
        {
            return;
        }

        self.event_shape = event_shape;
        self.rack.set_event_shape(event_shape);
    }

    fn apply_timeline_control_overrides(
        &mut self,
        state: crate::composition::timeline::TimelineState,
    ) {
        let base_controls = state.controls.clamped();

        self.rack.apply_timeline_overrides(state);

        self.texture.set_controls(base_controls);
        self.delay
            .set_feedback(map_range(base_controls.space, 0.20, 0.68));
        self.delay
            .set_wet(map_range(base_controls.space, 0.12, 0.55));
        self.delay
            .set_feedback_cutoff(map_range(base_controls.brightness, 600.0, 5_000.0));
    }

    pub fn set_drone_active(&mut self, active: bool) {
        self.rack.set_active(InstrumentFamily::Drone, active);
    }

    pub fn set_harmonic_active(&mut self, active: bool) {
        self.rack.set_active(InstrumentFamily::Harmonic, active);
    }

    pub fn set_pulse_active(&mut self, active: bool) {
        self.rack.set_active(InstrumentFamily::Pulse, active);
    }

    pub fn set_sample_active(&mut self, active: bool) {
        self.rack.set_active(InstrumentFamily::Sample, active);
    }

    pub fn set_noise_active(&mut self, active: bool) {
        self.rack.set_active(InstrumentFamily::Noise, active);
    }

    pub fn set_events_active(&mut self, active: bool) {
        self.rack.set_active(InstrumentFamily::Events, active);
    }

    pub fn set_drone_retune_seconds(&mut self, drone_retune_seconds: f32) {
        let drone_retune_seconds = drone_retune_seconds.clamp(0.25, 60.0);
        if (self.drone_retune_seconds - drone_retune_seconds).abs() <= f32::EPSILON {
            return;
        }

        self.drone_retune_seconds = drone_retune_seconds;
        self.rack.set_drone_retune_seconds(drone_retune_seconds);
    }

    pub fn set_instrument_params(&mut self, instrument_params: InstrumentParams) {
        let instrument_params = instrument_params.clamped();
        self.instrument_params = instrument_params;
        self.rack.set_params(instrument_params);
        self.texture.set_params(instrument_params.texture());
    }

    fn apply_timeline_controls(&mut self) {
        let Some(timeline) = &self.timeline else {
            return;
        };
        let time_seconds = self.sample_index as f32 / self.sample_rate;

        if let Some(state) = timeline.state_at(time_seconds) {
            self.set_voice_count(state.voice_count);
            let register = state.register();
            self.set_octave_range(register.octave_min, register.octave_max);
            self.set_drone_active(state.active(InstrumentFamily::Drone));
            self.set_harmonic_active(state.active(InstrumentFamily::Harmonic));
            self.set_pulse_active(state.active(InstrumentFamily::Pulse));
            self.set_sample_active(state.active(InstrumentFamily::Sample));
            self.set_noise_active(state.active(InstrumentFamily::Noise));
            self.set_events_active(state.active(InstrumentFamily::Events));
            self.set_event_attack_range(state.event_attack_min, state.event_attack_max);
            self.set_event_decay_range(state.event_decay_min, state.event_decay_max);
            self.set_drone_retune_seconds(state.drone_retune_seconds);
            self.set_root_hz(state.root_hz);
            self.set_instrument_params(state.instrument_params);
            self.apply_timeline_control_overrides(state);
        }
    }

    fn apply_sample_triggers(&mut self) {
        while let Some(trigger) = self.sample_triggers.get(self.next_sample_trigger_index) {
            let trigger_sample_index =
                (trigger.time_seconds.max(0.0) * self.sample_rate).round() as usize;
            if trigger_sample_index > self.sample_index {
                break;
            }

            self.rack.trigger_sample(trigger);
            self.next_sample_trigger_index += 1;
        }
    }

    pub fn seek_seconds(&mut self, time_seconds: f32) {
        let sample_index = (time_seconds.max(0.0) * self.sample_rate).round() as usize;
        self.sample_index = sample_index;
        self.next_sample_trigger_index = self
            .sample_triggers
            .iter()
            .position(|trigger| {
                (trigger.time_seconds * self.sample_rate).round() as usize >= sample_index
            })
            .unwrap_or(self.sample_triggers.len());
    }
}

impl GardenControls {
    pub fn level(self, family: InstrumentFamily) -> f32 {
        match family {
            InstrumentFamily::Drone => self.drone_level,
            InstrumentFamily::Harmonic => self.harmonic_level,
            InstrumentFamily::Pulse => self.pulse_level,
            InstrumentFamily::Sample => self.sample_level,
            InstrumentFamily::Noise => self.noise_level,
            InstrumentFamily::Events => self.event_level,
            InstrumentFamily::Texture => self.texture_level,
        }
    }

    pub fn level_mut(&mut self, family: InstrumentFamily) -> &mut f32 {
        match family {
            InstrumentFamily::Drone => &mut self.drone_level,
            InstrumentFamily::Harmonic => &mut self.harmonic_level,
            InstrumentFamily::Pulse => &mut self.pulse_level,
            InstrumentFamily::Sample => &mut self.sample_level,
            InstrumentFamily::Noise => &mut self.noise_level,
            InstrumentFamily::Events => &mut self.event_level,
            InstrumentFamily::Texture => &mut self.texture_level,
        }
    }

    pub fn clamped(self) -> Self {
        Self {
            density: clamp_macro(self.density),
            brightness: clamp_macro(self.brightness),
            space: clamp_macro(self.space),
            instability: clamp_macro(self.instability),
            drone_level: clamp_macro(self.drone_level),
            harmonic_level: clamp_macro(self.harmonic_level),
            pulse_level: clamp_macro(self.pulse_level),
            sample_level: clamp_macro(self.sample_level),
            noise_level: clamp_macro(self.noise_level),
            event_level: clamp_macro(self.event_level),
            texture_level: clamp_macro(self.texture_level),
        }
    }
}

impl StereoSource for Garden {
    fn next_stereo(&mut self) -> StereoSample {
        self.apply_timeline_controls();
        self.apply_sample_triggers();

        let dry = self.rack.next_stereo();
        let texture = self.texture.process(dry);
        let output = self.delay.process(dry + texture);

        self.sample_index += 1;
        output
    }
}

fn clamp_macro(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn map_range(value: f32, min: f32, max: f32) -> f32 {
    min + (max - min) * value
}
