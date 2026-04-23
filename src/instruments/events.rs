use crate::composition::garden::GardenControls;
use crate::composition::layers::events::EventLayer;
use crate::composition::pitch::PitchField;
use crate::composition::tuning::RegisterRange;
use crate::dsp::sample::StereoSample;
use crate::dsp::source::StereoSource;
use crate::instruments::Instrument;

pub struct EventInstrument {
    layer: EventLayer,
    active: bool,
}

impl EventInstrument {
    pub fn new(
        sample_rate: f32,
        pitch_field: PitchField,
        register: RegisterRange,
        seed: u64,
        controls: GardenControls,
    ) -> Self {
        Self {
            layer: EventLayer::new(sample_rate, pitch_field, register, seed, controls),
            active: true,
        }
    }

    pub fn set_pitch_field(&mut self, pitch_field: PitchField) {
        self.layer.set_pitch_field(pitch_field);
    }

    pub fn set_register(&mut self, register: RegisterRange) {
        self.layer.set_register(register);
    }

    pub fn set_decay_range(&mut self, decay_min_seconds: f32, decay_max_seconds: f32) {
        self.layer
            .set_decay_range(decay_min_seconds, decay_max_seconds);
    }

    pub fn set_attack_range(&mut self, attack_min_seconds: f32, attack_max_seconds: f32) {
        self.layer
            .set_attack_range(attack_min_seconds, attack_max_seconds);
    }
}

impl Instrument for EventInstrument {
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

impl StereoSource for EventInstrument {
    fn next_stereo(&mut self) -> StereoSample {
        if self.active {
            self.layer.next_stereo()
        } else {
            StereoSample::default()
        }
    }
}
