#[derive(Clone, Debug)]
pub struct PitchField {
    root_hz: f32,
    ratios: Vec<f32>,
}

impl PitchField {
    pub fn new(root_hz: f32, ratios: Vec<f32>) -> Self {
        Self {
            root_hz: root_hz.max(1.0),
            ratios,
        }
    }

    pub fn default_just(root_hz: f32) -> Self {
        Self::new(
            root_hz,
            vec![
                1.0,
                6.0 / 5.0,
                5.0 / 4.0,
                4.0 / 3.0,
                3.0 / 2.0,
                8.0 / 5.0,
                2.0,
            ],
        )
    }

    pub fn frequency(&self, index: usize, octave: i32) -> f32 {
        let ratio = self.ratios[index % self.ratios.len()];
        self.root_hz * ratio * 2.0_f32.powi(octave)
    }

    pub fn root_hz(&self) -> f32 {
        self.root_hz
    }

    pub fn len(&self) -> usize {
        self.ratios.len()
    }
}
