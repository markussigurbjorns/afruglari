use std::f32::consts::TAU;

use crate::composition::garden::{GardenControls, PulseParams};
use crate::composition::pitch::PitchField;
use crate::composition::tuning::RegisterRange;
use crate::dsp::random::SimpleRng;
use crate::dsp::sample::StereoSample;
use crate::dsp::source::StereoSource;
use crate::instruments::Instrument;

const PULSE_VOICE_COUNT: usize = 6;

pub struct PulseInstrument {
    voices: Vec<PulseVoice>,
    pitch_field: PitchField,
    rng: SimpleRng,
    controls: GardenControls,
    register: RegisterRange,
    sample_rate: f32,
    samples_until_trigger: usize,
    params: PulseParams,
    active: bool,
}

impl PulseInstrument {
    pub fn new(
        sample_rate: f32,
        pitch_field: PitchField,
        register: RegisterRange,
        seed: u64,
        controls: GardenControls,
    ) -> Self {
        Self {
            voices: (0..PULSE_VOICE_COUNT)
                .map(|_| PulseVoice::new(sample_rate))
                .collect(),
            pitch_field,
            rng: SimpleRng::new(seed),
            controls,
            register: register.clamped(),
            sample_rate,
            samples_until_trigger: 0,
            params: PulseParams {
                rate: 1.0,
                length: 1.0,
            },
            active: true,
        }
    }

    pub fn set_pitch_field(&mut self, pitch_field: PitchField) {
        self.pitch_field = pitch_field;
    }

    pub fn set_register(&mut self, register: RegisterRange) {
        self.register = register.clamped();
    }

    pub fn set_voice_count(&mut self, voice_count: usize) {
        let voice_count = voice_count.clamp(1, PULSE_VOICE_COUNT);

        for (index, voice) in self.voices.iter_mut().enumerate() {
            if index >= voice_count {
                voice.deactivate();
            }
        }
    }

    pub fn set_params(&mut self, params: PulseParams) {
        self.params = params.clamped();
    }

    fn tick_trigger(&mut self) {
        if self.controls.pulse_level <= 0.0 {
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

        let octave = self.rng.range_usize(
            self.register.octave_min as usize,
            self.register.octave_max as usize + 1,
        ) as i32;
        let field_index = self.rng.range_usize(0, self.pitch_field.len());
        let base_frequency = self.pitch_field.frequency(field_index, octave);
        let detune_range = 1.0 + self.controls.instability * 16.0;
        let detune_cents = self.rng.range_f32(-detune_range, detune_range);
        let frequency_hz = cents_to_frequency(base_frequency, detune_cents);
        let pulse_seconds = self.rng.range_f32(
            0.08 + (1.0 - self.controls.density) * 0.10,
            0.18 + (1.0 - self.controls.density) * 0.22,
        ) * self.params.length;
        let ring_seconds = self.rng.range_f32(
            0.35 + self.controls.space * 0.15,
            0.9 + self.controls.space * 0.45,
        ) * self.params.length;
        let amplitude = self.controls.pulse_level * self.rng.range_f32(0.025, 0.075);
        let pan_range = 0.20 + self.controls.space * 0.5;
        let pan = self.rng.range_f32(-pan_range, pan_range);
        let overtone_mix =
            (0.15 + self.controls.brightness * 0.55 + self.controls.instability * 0.15)
                .clamp(0.0, 1.0);

        voice.trigger(PulseVoiceTrigger {
            frequency_hz,
            amplitude,
            pan,
            pulse_seconds,
            ring_seconds,
            overtone_mix,
        });
    }

    fn next_trigger_interval_samples(&mut self) -> usize {
        let shortest_seconds = 0.24 + (1.0 - self.controls.density) * 0.45;
        let longest_seconds = 0.65 + (1.0 - self.controls.density) * 1.35;
        let seconds = self.rng.range_f32(shortest_seconds, longest_seconds) / self.params.rate;
        (seconds * self.sample_rate).round().max(1.0) as usize
    }
}

impl Instrument for PulseInstrument {
    fn set_controls(&mut self, controls: GardenControls) {
        self.controls = controls;
    }

    fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

impl StereoSource for PulseInstrument {
    fn next_stereo(&mut self) -> StereoSample {
        if !self.active {
            return StereoSample::default();
        }

        self.tick_trigger();

        let mut output = StereoSample::default();
        for voice in &mut self.voices {
            output += voice.next_stereo();
        }
        output
    }
}

struct PulseVoice {
    active: bool,
    frequency_hz: f32,
    amplitude: f32,
    pan: f32,
    phase: f32,
    overtone_mix: f32,
    sample_rate: f32,
    age_samples: usize,
    pulse_samples: usize,
    ring_samples: usize,
}

impl PulseVoice {
    fn new(sample_rate: f32) -> Self {
        Self {
            active: false,
            frequency_hz: 220.0,
            amplitude: 0.0,
            pan: 0.0,
            phase: 0.0,
            overtone_mix: 0.0,
            sample_rate,
            age_samples: 0,
            pulse_samples: 1,
            ring_samples: 1,
        }
    }

    fn trigger(&mut self, trigger: PulseVoiceTrigger) {
        self.active = true;
        self.frequency_hz = trigger.frequency_hz.max(1.0);
        self.amplitude = trigger.amplitude.clamp(0.0, 1.0);
        self.pan = trigger.pan.clamp(-1.0, 1.0);
        self.phase = 0.0;
        self.overtone_mix = trigger.overtone_mix.clamp(0.0, 1.0);
        self.age_samples = 0;
        self.pulse_samples = seconds_to_samples(trigger.pulse_seconds, self.sample_rate);
        self.ring_samples = seconds_to_samples(trigger.ring_seconds, self.sample_rate);
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn deactivate(&mut self) {
        self.active = false;
    }
}

impl StereoSource for PulseVoice {
    fn next_stereo(&mut self) -> StereoSample {
        if !self.active {
            return StereoSample::default();
        }

        let envelope = pulse_envelope(self.age_samples, self.pulse_samples, self.ring_samples);
        let fundamental = (self.phase * TAU).sin();
        let overtone = (self.phase * TAU * 2.0 + 0.41).sin() * self.overtone_mix;
        let air = (self.phase * TAU * 4.0 + 1.13).sin() * self.overtone_mix * 0.25;
        let sample = (fundamental + overtone * 0.55 + air * 0.18) * self.amplitude * envelope;
        let left_gain = (1.0 - self.pan) * 0.5;
        let right_gain = (1.0 + self.pan) * 0.5;

        self.phase = (self.phase + self.frequency_hz / self.sample_rate).fract();
        self.age_samples += 1;
        if self.age_samples >= self.pulse_samples + self.ring_samples {
            self.active = false;
        }

        StereoSample::new(sample * left_gain, sample * right_gain)
    }
}

struct PulseVoiceTrigger {
    frequency_hz: f32,
    amplitude: f32,
    pan: f32,
    pulse_seconds: f32,
    ring_seconds: f32,
    overtone_mix: f32,
}

fn pulse_envelope(age_samples: usize, pulse_samples: usize, ring_samples: usize) -> f32 {
    if age_samples < pulse_samples {
        let attack_position = age_samples as f32 / pulse_samples.max(1) as f32;
        (attack_position * TAU * 0.5).sin().abs().powf(0.35)
    } else {
        let ring_age = age_samples - pulse_samples;
        let amount = 1.0 - ring_age as f32 / ring_samples.max(1) as f32;
        amount.clamp(0.0, 1.0).powf(1.8)
    }
}

fn seconds_to_samples(seconds: f32, sample_rate: f32) -> usize {
    (seconds.max(0.001) * sample_rate).round().max(1.0) as usize
}

fn cents_to_frequency(base_frequency_hz: f32, cents: f32) -> f32 {
    base_frequency_hz * 2.0_f32.powf(cents / 1200.0)
}
