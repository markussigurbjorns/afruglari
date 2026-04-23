use crate::composition::garden::{GardenControls, NoiseParams};
use crate::composition::layers::noise::NoiseLayer;
use crate::dsp::sample::StereoSample;
use crate::dsp::source::StereoSource;
use crate::instruments::Instrument;

pub struct NoiseInstrument {
    layer: NoiseLayer,
    active: bool,
}

impl NoiseInstrument {
    pub fn new(sample_rate: f32, seed: u64, controls: GardenControls) -> Self {
        Self {
            layer: NoiseLayer::new(sample_rate, seed, controls),
            active: true,
        }
    }

    pub fn set_params(&mut self, params: NoiseParams) {
        self.layer.set_params(params);
    }
}

impl Instrument for NoiseInstrument {
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

impl StereoSource for NoiseInstrument {
    fn next_stereo(&mut self) -> StereoSample {
        if self.active {
            self.layer.next_stereo()
        } else {
            StereoSample::default()
        }
    }
}
