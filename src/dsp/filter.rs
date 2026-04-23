use std::f32::consts::TAU;

pub struct OnePoleLowpass {
    sample_rate: f32,
    coefficient: f32,
    state: f32,
}

impl OnePoleLowpass {
    pub fn new(sample_rate: f32, cutoff_hz: f32) -> Self {
        let mut filter = Self {
            sample_rate,
            coefficient: 1.0,
            state: 0.0,
        };
        filter.set_cutoff(cutoff_hz);
        filter
    }

    pub fn process(&mut self, input: f32) -> f32 {
        self.state += (input - self.state) * self.coefficient;
        self.state
    }

    pub fn set_cutoff(&mut self, cutoff_hz: f32) {
        let cutoff_hz = cutoff_hz.clamp(1.0, self.sample_rate * 0.45);
        self.coefficient = 1.0 - (-TAU * cutoff_hz / self.sample_rate).exp();
    }
}
