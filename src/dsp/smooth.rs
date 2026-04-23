pub struct SmoothedValue {
    current: f32,
    target: f32,
    coefficient: f32,
}

impl SmoothedValue {
    pub fn new(value: f32, smoothing_seconds: f32, sample_rate: f32) -> Self {
        let coefficient = if smoothing_seconds <= 0.0 {
            1.0
        } else {
            1.0 - (-1.0 / (smoothing_seconds * sample_rate)).exp()
        };

        Self {
            current: value,
            target: value,
            coefficient,
        }
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    pub fn next(&mut self) -> f32 {
        self.current += (self.target - self.current) * self.coefficient;
        self.current
    }
}
