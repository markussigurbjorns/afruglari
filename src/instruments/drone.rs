use crate::composition::garden::{DroneParams, GardenControls};
use crate::composition::layers::drone::DroneLayer;
use crate::composition::pitch::PitchField;
use crate::composition::tuning::RegisterRange;
use crate::dsp::sample::StereoSample;
use crate::dsp::source::StereoSource;
use crate::instruments::Instrument;

pub struct DroneInstrument {
    layer: DroneLayer,
    active: bool,
}

impl DroneInstrument {
    pub fn new(
        sample_rate: f32,
        pitch_field: PitchField,
        register: RegisterRange,
        voice_count: usize,
        seed: u64,
        controls: GardenControls,
    ) -> Self {
        Self {
            layer: DroneLayer::new(
                sample_rate,
                pitch_field,
                register,
                voice_count,
                seed,
                controls,
            ),
            active: true,
        }
    }

    pub fn voice_count(&self) -> usize {
        if self.active {
            self.layer.voice_count()
        } else {
            0
        }
    }

    pub fn set_voice_count(&mut self, voice_count: usize) {
        self.layer.set_voice_count(voice_count);
    }

    pub fn set_register(&mut self, register: RegisterRange) {
        self.layer.set_register(register);
    }

    pub fn set_pitch_field(&mut self, pitch_field: PitchField) {
        self.layer.set_pitch_field(pitch_field);
    }

    pub fn set_retune_seconds(&mut self, seconds: f32) {
        self.layer.set_retune_seconds(seconds);
    }

    pub fn set_params(&mut self, params: DroneParams) {
        self.layer.set_params(params);
    }
}

impl Instrument for DroneInstrument {
    fn set_controls(&mut self, controls: GardenControls) {
        self.layer.set_controls(controls);
    }

    fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

impl StereoSource for DroneInstrument {
    fn next_stereo(&mut self) -> StereoSample {
        if self.active {
            self.layer.next_stereo()
        } else {
            StereoSample::default()
        }
    }
}
