use crate::composition::garden::{DroneParams, GardenControls};
use crate::composition::pitch::PitchField;
use crate::composition::tuning::RegisterRange;
use crate::dsp::mixer::VoiceBank;
use crate::dsp::random::SimpleRng;
use crate::dsp::sample::StereoSample;
use crate::dsp::source::StereoSource;
use crate::dsp::voice::DroneVoice;

const DRONE_GAIN: f32 = 0.07;
const OUTPUT_GAIN: f32 = 0.7;
const MODULATION_INTERVAL_SECONDS: f32 = 9.0;
const MAX_DRONE_VOICES: usize = 12;

pub struct DroneLayer {
    voices: VoiceBank<DroneVoice>,
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
    params: DroneParams,
}

impl DroneLayer {
    pub fn new(
        sample_rate: f32,
        pitch_field: PitchField,
        register: RegisterRange,
        voice_count: usize,
        seed: u64,
        controls: GardenControls,
    ) -> Self {
        let active_voice_count = voice_count.clamp(1, MAX_DRONE_VOICES);
        let voice_slots = initial_voice_slots(MAX_DRONE_VOICES, pitch_field.len());
        let voices = voice_slots
            .iter()
            .enumerate()
            .map(|(index, slot)| {
                let gain = if index < active_voice_count {
                    DRONE_GAIN
                } else {
                    0.0
                };
                DroneVoice::new(
                    pitch_field.frequency(slot.field_index, slot.octave),
                    gain,
                    initial_pan(index, active_voice_count),
                    sample_rate,
                )
            })
            .collect();
        let voices = VoiceBank::new(voices, OUTPUT_GAIN);
        let modulation_interval_samples =
            (MODULATION_INTERVAL_SECONDS * sample_rate).round() as usize;

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
            params: DroneParams {
                spread: 1.0,
                detune: 1.0,
            },
        }
    }

    pub fn voice_count(&self) -> usize {
        self.active_voice_count
    }

    pub fn set_controls(&mut self, controls: GardenControls) {
        self.controls = controls;
    }

    pub fn set_params(&mut self, params: DroneParams) {
        self.params = params.clamped();
    }

    pub fn set_voice_count(&mut self, voice_count: usize) {
        let voice_count = voice_count.clamp(1, MAX_DRONE_VOICES);
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
        let seconds = seconds.clamp(0.25, 60.0);
        if (self.modulation_interval_seconds - seconds).abs() <= f32::EPSILON {
            return;
        }

        self.modulation_interval_seconds = seconds;
        self.modulation_interval_samples = (seconds * self.sample_rate).round().max(1.0) as usize;
        self.samples_until_modulation = self
            .samples_until_modulation
            .min(self.modulation_interval_samples);
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
        let retune_probability = 0.15 + self.controls.density * 0.45;
        let detune_range_cents = (4.0 + self.controls.instability * 32.0) * self.params.detune;
        let gain_min = 0.045 * self.controls.drone_level;
        let gain_max = 0.09 * self.controls.drone_level;

        for (index, voice) in self.voices.voices_mut().iter_mut().enumerate() {
            if index >= self.active_voice_count {
                voice.set_gain(0.0);
                continue;
            }

            if allow_slot_changes && self.rng.range_f32(0.0, 1.0) < retune_probability {
                self.voice_slots[index].field_index =
                    self.rng.range_usize(0, self.pitch_field.len());
            }

            let detune_cents = self.rng.range_f32(-detune_range_cents, detune_range_cents);
            let slot = self.voice_slots[index];
            let base_frequency = self.pitch_field.frequency(slot.field_index, slot.octave);
            let frequency = cents_to_frequency(base_frequency, detune_cents);
            let gain = self.rng.range_f32(gain_min, gain_max);
            let pan = initial_pan(index, self.active_voice_count) * self.params.spread;

            voice.set_frequency_hz(frequency);
            voice.set_gain(gain);
            voice.set_pan(pan);
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

impl StereoSource for DroneLayer {
    fn next_stereo(&mut self) -> StereoSample {
        self.tick_modulation();
        self.voices.next_stereo()
    }
}

fn cents_to_frequency(base_frequency_hz: f32, cents: f32) -> f32 {
    base_frequency_hz * 2.0_f32.powf(cents / 1200.0)
}

fn initial_voice_slots(voice_count: usize, field_len: usize) -> Vec<VoicePitchSlot> {
    (0..voice_count)
        .map(|index| {
            let field_index = match index {
                0 => 0,
                1 => 4,
                _ => (index * 2) % field_len,
            };
            let octave = 1 + (index / field_len) as i32;

            VoicePitchSlot::new(field_index, octave)
        })
        .collect()
}

fn initial_pan(index: usize, voice_count: usize) -> f32 {
    if voice_count <= 1 {
        0.0
    } else {
        let position = index as f32 / (voice_count - 1) as f32;
        position * 1.4 - 0.7
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
