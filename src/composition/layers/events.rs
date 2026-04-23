use std::f32::consts::TAU;

use crate::composition::garden::GardenControls;
use crate::composition::pitch::PitchField;
use crate::composition::tuning::RegisterRange;
use crate::dsp::random::SimpleRng;
use crate::dsp::sample::StereoSample;
use crate::dsp::source::StereoSource;

const EVENT_VOICE_COUNT: usize = 8;

pub struct EventLayer {
    voices: Vec<EventVoice>,
    pitch_field: PitchField,
    rng: SimpleRng,
    controls: GardenControls,
    register: RegisterRange,
    attack_min_seconds: f32,
    attack_max_seconds: f32,
    decay_min_seconds: f32,
    decay_max_seconds: f32,
    sample_rate: f32,
    samples_until_trigger: usize,
}

impl EventLayer {
    pub fn new(
        sample_rate: f32,
        pitch_field: PitchField,
        register: RegisterRange,
        seed: u64,
        controls: GardenControls,
    ) -> Self {
        Self {
            voices: (0..EVENT_VOICE_COUNT)
                .map(|_| EventVoice::new(sample_rate))
                .collect(),
            pitch_field,
            rng: SimpleRng::new(seed),
            controls,
            register: register.clamped(),
            attack_min_seconds: 0.015,
            attack_max_seconds: 0.195,
            decay_min_seconds: 2.0,
            decay_max_seconds: 8.0,
            sample_rate,
            samples_until_trigger: 0,
        }
    }

    pub fn set_controls(&mut self, controls: GardenControls) {
        self.controls = controls;
    }

    pub fn set_pitch_field(&mut self, pitch_field: PitchField) {
        self.pitch_field = pitch_field;
    }

    pub fn set_register(&mut self, register: RegisterRange) {
        self.register = register.clamped();
    }

    pub fn set_decay_range(&mut self, decay_min_seconds: f32, decay_max_seconds: f32) {
        self.decay_min_seconds = decay_min_seconds.max(0.05);
        self.decay_max_seconds = decay_max_seconds.max(0.05).max(self.decay_min_seconds);
    }

    pub fn set_attack_range(&mut self, attack_min_seconds: f32, attack_max_seconds: f32) {
        self.attack_min_seconds = attack_min_seconds.max(0.001);
        self.attack_max_seconds = attack_max_seconds.max(0.001).max(self.attack_min_seconds);
    }

    fn tick_trigger(&mut self) {
        if self.controls.event_level <= 0.0 {
            return;
        }

        if self.samples_until_trigger > 0 {
            self.samples_until_trigger -= 1;
            return;
        }

        self.trigger_voice();
        self.samples_until_trigger = self.next_trigger_interval_samples();
    }

    fn trigger_voice(&mut self) {
        let Some(voice) = self.voices.iter_mut().find(|voice| !voice.is_active()) else {
            return;
        };

        let field_index = self.rng.range_usize(0, self.pitch_field.len());
        let octave = self.rng.range_usize(
            self.register.octave_min as usize,
            self.register.octave_max as usize + 1,
        ) as i32;
        let detune_range_cents = 2.0 + self.controls.instability * 35.0;
        let detune_cents = self.rng.range_f32(-detune_range_cents, detune_range_cents);
        let frequency_hz = cents_to_frequency(
            self.pitch_field.frequency(field_index, octave),
            detune_cents,
        );
        let attack_seconds = self
            .rng
            .range_f32(self.attack_min_seconds, self.attack_max_seconds);
        let decay_seconds = self
            .rng
            .range_f32(self.decay_min_seconds, self.decay_max_seconds);
        let amplitude = self.controls.event_level * self.rng.range_f32(0.025, 0.075);
        let pan_range = 0.35 + self.controls.instability * 0.6;
        let pan = self.rng.range_f32(-pan_range, pan_range);
        let harmonic_mix = self.controls.brightness * self.rng.range_f32(0.05, 0.35);

        voice.trigger(EventVoiceTrigger {
            frequency_hz,
            amplitude,
            pan,
            attack_seconds,
            decay_seconds,
            harmonic_mix,
        });
    }

    fn next_trigger_interval_samples(&mut self) -> usize {
        let fastest_seconds = 0.6 + (1.0 - self.controls.density) * 2.4;
        let slowest_seconds = 2.0 + (1.0 - self.controls.density) * 10.0;
        let seconds = self.rng.range_f32(fastest_seconds, slowest_seconds);
        (seconds * self.sample_rate).round().max(1.0) as usize
    }
}

impl StereoSource for EventLayer {
    fn next_stereo(&mut self) -> StereoSample {
        self.tick_trigger();

        let mut output = StereoSample::default();
        for voice in &mut self.voices {
            output += voice.next_stereo();
        }
        output
    }
}

struct EventVoice {
    active: bool,
    frequency_hz: f32,
    amplitude: f32,
    pan: f32,
    phase: f32,
    harmonic_mix: f32,
    sample_rate: f32,
    age_samples: usize,
    attack_samples: usize,
    decay_samples: usize,
}

impl EventVoice {
    fn new(sample_rate: f32) -> Self {
        Self {
            active: false,
            frequency_hz: 440.0,
            amplitude: 0.0,
            pan: 0.0,
            phase: 0.0,
            harmonic_mix: 0.0,
            sample_rate,
            age_samples: 0,
            attack_samples: 1,
            decay_samples: 1,
        }
    }

    fn trigger(&mut self, trigger: EventVoiceTrigger) {
        self.active = true;
        self.frequency_hz = trigger.frequency_hz;
        self.amplitude = trigger.amplitude;
        self.pan = trigger.pan.clamp(-1.0, 1.0);
        self.phase = 0.0;
        self.harmonic_mix = trigger.harmonic_mix.clamp(0.0, 1.0);
        self.age_samples = 0;
        self.attack_samples = seconds_to_samples(trigger.attack_seconds, self.sample_rate);
        self.decay_samples = seconds_to_samples(trigger.decay_seconds, self.sample_rate);
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn next_stereo(&mut self) -> StereoSample {
        if !self.active {
            return StereoSample::default();
        }

        let envelope = self.envelope();
        let fundamental = (self.phase * TAU).sin();
        let harmonic = (self.phase * TAU * 2.0).sin() * self.harmonic_mix;
        let sample = (fundamental + harmonic) * self.amplitude * envelope;
        let left_gain = (1.0 - self.pan) * 0.5;
        let right_gain = (1.0 + self.pan) * 0.5;

        self.phase = (self.phase + self.frequency_hz / self.sample_rate).fract();
        self.age_samples += 1;
        if self.age_samples >= self.attack_samples + self.decay_samples {
            self.active = false;
        }

        StereoSample::new(sample * left_gain, sample * right_gain)
    }

    fn envelope(&self) -> f32 {
        if self.age_samples < self.attack_samples {
            self.age_samples as f32 / self.attack_samples as f32
        } else {
            let decay_age = self.age_samples - self.attack_samples;
            1.0 - decay_age as f32 / self.decay_samples as f32
        }
        .clamp(0.0, 1.0)
    }
}

struct EventVoiceTrigger {
    frequency_hz: f32,
    amplitude: f32,
    pan: f32,
    attack_seconds: f32,
    decay_seconds: f32,
    harmonic_mix: f32,
}

fn cents_to_frequency(base_frequency_hz: f32, cents: f32) -> f32 {
    base_frequency_hz * 2.0_f32.powf(cents / 1200.0)
}

fn seconds_to_samples(seconds: f32, sample_rate: f32) -> usize {
    (seconds.max(0.001) * sample_rate).round().max(1.0) as usize
}
