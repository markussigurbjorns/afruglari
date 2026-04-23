use std::f32::consts::TAU;

use crate::composition::garden::{GardenControls, HarmonicParams};
use crate::composition::pitch::PitchField;
use crate::composition::tuning::RegisterRange;
use crate::dsp::mixer::VoiceBank;
use crate::dsp::random::SimpleRng;
use crate::dsp::sample::StereoSample;
use crate::dsp::smooth::SmoothedValue;
use crate::dsp::source::StereoSource;
use crate::instruments::Instrument;

const MAX_PAD_VOICES: usize = 8;
const OUTPUT_GAIN: f32 = 0.75;
const MODULATION_INTERVAL_SECONDS: f32 = 13.0;
const FREQUENCY_SMOOTHING_SECONDS: f32 = 7.0;
const GAIN_SMOOTHING_SECONDS: f32 = 10.0;
const PAN_SMOOTHING_SECONDS: f32 = 11.0;
const HARMONIC_MIX_SMOOTHING_SECONDS: f32 = 8.0;
const SHIMMER_SMOOTHING_SECONDS: f32 = 9.0;

pub struct HarmonicPadInstrument {
    voices: VoiceBank<HarmonicPadVoice>,
    pitch_field: PitchField,
    voice_slots: Vec<VoicePitchSlot>,
    active_voice_count: usize,
    register: RegisterRange,
    rng: SimpleRng,
    sample_rate: f32,
    samples_until_modulation: usize,
    modulation_interval_samples: usize,
    modulation_interval_seconds: f32,
    controls: GardenControls,
    params: HarmonicParams,
    active: bool,
}

impl HarmonicPadInstrument {
    pub fn new(
        sample_rate: f32,
        pitch_field: PitchField,
        register: RegisterRange,
        voice_count: usize,
        seed: u64,
        controls: GardenControls,
    ) -> Self {
        let active_voice_count = voice_count.clamp(1, MAX_PAD_VOICES);
        let voice_slots = initial_voice_slots(MAX_PAD_VOICES, pitch_field.len());
        let voices = voice_slots
            .iter()
            .enumerate()
            .map(|(index, slot)| {
                let params = HarmonicParams {
                    mix: 1.0,
                    shimmer: 1.0,
                };
                let harmonic_mix = harmonic_mix_from_controls(controls, params, index);
                let shimmer_amount = shimmer_amount_from_controls(controls, params, index);
                let gain = if index < active_voice_count {
                    voice_gain_from_controls(controls)
                } else {
                    0.0
                };

                HarmonicPadVoice::new(
                    pitch_field.frequency(slot.field_index, slot.octave),
                    gain,
                    pad_pan(index, active_voice_count),
                    harmonic_mix,
                    shimmer_amount,
                    sample_rate,
                )
            })
            .collect();
        let voices = VoiceBank::new(voices, OUTPUT_GAIN);
        let modulation_interval_samples =
            (MODULATION_INTERVAL_SECONDS * sample_rate).round().max(1.0) as usize;

        Self {
            voices,
            pitch_field,
            voice_slots,
            active_voice_count,
            register: register.clamped(),
            rng: SimpleRng::new(seed),
            sample_rate,
            samples_until_modulation: modulation_interval_samples,
            modulation_interval_samples,
            modulation_interval_seconds: MODULATION_INTERVAL_SECONDS,
            controls,
            params: HarmonicParams {
                mix: 1.0,
                shimmer: 1.0,
            },
            active: true,
        }
    }

    pub fn set_voice_count(&mut self, voice_count: usize) {
        let voice_count = voice_count.clamp(1, MAX_PAD_VOICES);
        if self.active_voice_count == voice_count {
            return;
        }

        self.active_voice_count = voice_count;
        self.redistribute_octaves();
        self.retune_voices(false);
    }

    pub fn set_register(&mut self, register: RegisterRange) {
        let register = register.clamped();
        if self.register == register {
            return;
        }

        self.register = register;
        self.redistribute_octaves();
        self.retune_voices(false);
    }

    pub fn set_pitch_field(&mut self, pitch_field: PitchField) {
        self.pitch_field = pitch_field;
        self.retune_voices(false);
    }

    pub fn set_retune_seconds(&mut self, seconds: f32) {
        let seconds = seconds.clamp(0.5, 90.0);
        if (self.modulation_interval_seconds - seconds).abs() <= f32::EPSILON {
            return;
        }

        self.modulation_interval_seconds = seconds;
        self.modulation_interval_samples = (seconds * self.sample_rate).round().max(1.0) as usize;
        self.samples_until_modulation = self
            .samples_until_modulation
            .min(self.modulation_interval_samples);
    }

    pub fn set_params(&mut self, params: HarmonicParams) {
        self.params = params.clamped();
    }

    fn tick_modulation(&mut self) {
        if self.samples_until_modulation > 0 {
            self.samples_until_modulation -= 1;
            return;
        }

        self.retune_voices(true);
        self.samples_until_modulation = self.modulation_interval_samples;
    }

    fn retune_voices(&mut self, allow_slot_changes: bool) {
        let retune_probability = 0.10 + self.controls.density * 0.30;
        let detune_range_cents = 1.5 + self.controls.instability * 18.0;
        let pan_range = 0.25 + self.controls.space * 0.45;

        for (index, voice) in self.voices.voices_mut().iter_mut().enumerate() {
            if index >= self.active_voice_count {
                voice.set_gain(0.0);
                continue;
            }

            if allow_slot_changes && self.rng.range_f32(0.0, 1.0) < retune_probability {
                self.voice_slots[index].field_index =
                    self.rng.range_usize(0, self.pitch_field.len());
            }

            let slot = self.voice_slots[index];
            let base_frequency = self.pitch_field.frequency(slot.field_index, slot.octave);
            let detune_cents = self.rng.range_f32(-detune_range_cents, detune_range_cents);
            let frequency = cents_to_frequency(base_frequency, detune_cents);
            let gain = self.rng.range_f32(
                voice_gain_from_controls(self.controls) * 0.8,
                voice_gain_from_controls(self.controls) * 1.2,
            );
            let pan = self.rng.range_f32(-pan_range, pan_range);
            let harmonic_mix = harmonic_mix_from_controls(self.controls, self.params, index)
                * self.rng.range_f32(0.85, 1.15);
            let shimmer_amount = shimmer_amount_from_controls(self.controls, self.params, index)
                * self.rng.range_f32(0.85, 1.20);

            voice.set_frequency_hz(frequency);
            voice.set_gain(gain);
            voice.set_pan(pan);
            voice.set_harmonic_mix(harmonic_mix);
            voice.set_shimmer_amount(shimmer_amount);
        }
    }

    fn redistribute_octaves(&mut self) {
        let range_len = (self.register.octave_max - self.register.octave_min + 1).max(1) as usize;

        for (index, slot) in self.voice_slots.iter_mut().enumerate() {
            let octave_offset = if self.active_voice_count <= 1 {
                0
            } else {
                index.min(self.active_voice_count - 1) % range_len
            };
            slot.octave = self.register.octave_min + octave_offset as i32;
        }
    }
}

impl Instrument for HarmonicPadInstrument {
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

impl StereoSource for HarmonicPadInstrument {
    fn next_stereo(&mut self) -> StereoSample {
        if !self.active {
            return StereoSample::default();
        }

        self.tick_modulation();
        self.voices.next_stereo()
    }
}

struct HarmonicPadVoice {
    frequency_hz: SmoothedValue,
    gain: SmoothedValue,
    pan: SmoothedValue,
    harmonic_mix: SmoothedValue,
    shimmer_amount: SmoothedValue,
    sample_rate: f32,
    phase: f32,
    shimmer_phase: f32,
}

impl HarmonicPadVoice {
    fn new(
        frequency_hz: f32,
        gain: f32,
        pan: f32,
        harmonic_mix: f32,
        shimmer_amount: f32,
        sample_rate: f32,
    ) -> Self {
        let pan = pan.clamp(-1.0, 1.0);
        let mut smoothed_frequency_hz =
            SmoothedValue::new(frequency_hz, FREQUENCY_SMOOTHING_SECONDS, sample_rate);
        let mut smoothed_gain = SmoothedValue::new(gain, GAIN_SMOOTHING_SECONDS, sample_rate);
        let mut smoothed_pan = SmoothedValue::new(pan, PAN_SMOOTHING_SECONDS, sample_rate);
        let mut smoothed_harmonic_mix =
            SmoothedValue::new(harmonic_mix, HARMONIC_MIX_SMOOTHING_SECONDS, sample_rate);
        let mut smoothed_shimmer_amount =
            SmoothedValue::new(shimmer_amount, SHIMMER_SMOOTHING_SECONDS, sample_rate);

        smoothed_frequency_hz.set_target(frequency_hz);
        smoothed_gain.set_target(gain);
        smoothed_pan.set_target(pan);
        smoothed_harmonic_mix.set_target(harmonic_mix);
        smoothed_shimmer_amount.set_target(shimmer_amount);

        Self {
            frequency_hz: smoothed_frequency_hz,
            gain: smoothed_gain,
            pan: smoothed_pan,
            harmonic_mix: smoothed_harmonic_mix,
            shimmer_amount: smoothed_shimmer_amount,
            sample_rate,
            phase: 0.0,
            shimmer_phase: 0.0,
        }
    }

    fn set_frequency_hz(&mut self, frequency_hz: f32) {
        self.frequency_hz.set_target(frequency_hz.max(1.0));
    }

    fn set_gain(&mut self, gain: f32) {
        self.gain.set_target(gain.clamp(0.0, 1.0));
    }

    fn set_pan(&mut self, pan: f32) {
        self.pan.set_target(pan.clamp(-1.0, 1.0));
    }

    fn set_harmonic_mix(&mut self, harmonic_mix: f32) {
        self.harmonic_mix.set_target(harmonic_mix.clamp(0.0, 1.0));
    }

    fn set_shimmer_amount(&mut self, shimmer_amount: f32) {
        self.shimmer_amount
            .set_target(shimmer_amount.clamp(0.0, 1.0));
    }
}

impl StereoSource for HarmonicPadVoice {
    fn next_stereo(&mut self) -> StereoSample {
        let frequency_hz = self.frequency_hz.next();
        let gain = self.gain.next();
        let pan = self.pan.next();
        let harmonic_mix = self.harmonic_mix.next();
        let shimmer_amount = self.shimmer_amount.next();

        let fundamental = (self.phase * TAU).sin();
        let second = (self.phase * TAU * 2.0 + 0.31).sin();
        let third = (self.phase * TAU * 3.0 + 1.07).sin();
        let fifth = (self.phase * TAU * 5.0 + 2.21).sin();
        let shimmer = (self.shimmer_phase * TAU).sin();

        let sample = (fundamental * (1.0 - harmonic_mix * 0.35)
            + second * harmonic_mix * 0.26
            + third * harmonic_mix * 0.18
            + fifth * harmonic_mix * harmonic_mix * 0.08
            + shimmer * shimmer_amount * 0.22)
            * gain;

        self.phase = (self.phase + frequency_hz / self.sample_rate).fract();
        self.shimmer_phase = (self.shimmer_phase
            + frequency_hz * (1.0015 + shimmer_amount * 0.0035) / self.sample_rate)
            .fract();

        let left_gain = (1.0 - pan) * 0.5;
        let right_gain = (1.0 + pan) * 0.5;

        StereoSample::new(sample * left_gain, sample * right_gain)
    }
}

fn voice_gain_from_controls(controls: GardenControls) -> f32 {
    controls.harmonic_level * (0.035 + controls.space * 0.025)
}

fn harmonic_mix_from_controls(
    controls: GardenControls,
    params: HarmonicParams,
    voice_index: usize,
) -> f32 {
    ((0.25 + controls.brightness * 0.55 + voice_index as f32 * 0.02) * params.mix).clamp(0.0, 1.0)
}

fn shimmer_amount_from_controls(
    controls: GardenControls,
    params: HarmonicParams,
    voice_index: usize,
) -> f32 {
    ((0.08 + controls.instability * 0.35 + controls.space * 0.15 + voice_index as f32 * 0.01)
        * params.shimmer)
        .clamp(0.0, 1.0)
}

fn cents_to_frequency(base_frequency_hz: f32, cents: f32) -> f32 {
    base_frequency_hz * 2.0_f32.powf(cents / 1200.0)
}

fn initial_voice_slots(voice_count: usize, field_len: usize) -> Vec<VoicePitchSlot> {
    (0..voice_count)
        .map(|index| {
            let field_index = match index {
                0 => 0,
                1 => 2,
                _ => (index * 3) % field_len,
            };
            let octave = 1 + (index / field_len) as i32;

            VoicePitchSlot::new(field_index, octave)
        })
        .collect()
}

fn pad_pan(index: usize, voice_count: usize) -> f32 {
    if voice_count <= 1 {
        0.0
    } else {
        let position = index as f32 / (voice_count - 1) as f32;
        position * 1.0 - 0.5
    }
}

#[derive(Clone, Copy)]
struct VoicePitchSlot {
    field_index: usize,
    octave: i32,
}

impl VoicePitchSlot {
    fn new(field_index: usize, octave: i32) -> Self {
        Self {
            field_index,
            octave,
        }
    }
}
