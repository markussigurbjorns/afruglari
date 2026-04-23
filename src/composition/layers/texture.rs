use crate::composition::garden::{GardenControls, TextureParams};
use crate::dsp::filter::OnePoleLowpass;
use crate::dsp::sample::StereoSample;

const BUFFER_SECONDS: f32 = 24.0;

pub struct TextureLayer {
    buffer: Vec<StereoSample>,
    write_index: usize,
    taps: [TextureTap; 3],
    left_filter: OnePoleLowpass,
    right_filter: OnePoleLowpass,
    level: f32,
    feedback: f32,
    params: TextureParams,
}

impl TextureLayer {
    pub fn new(sample_rate: f32, controls: GardenControls) -> Self {
        let buffer_len = (BUFFER_SECONDS * sample_rate).round().max(1.0) as usize;
        let level = texture_level(controls);
        let smear = controls.space;
        let drift = controls.instability;
        let cutoff_hz = texture_cutoff_hz(controls);
        let feedback = texture_feedback(controls);

        Self {
            buffer: vec![StereoSample::default(); buffer_len],
            write_index: 0,
            taps: [
                TextureTap::new(0.9 + smear * 2.2, 0.55, -0.0007 * drift, sample_rate),
                TextureTap::new(5.5 + smear * 6.0, 0.35, 0.0011 * drift, sample_rate),
                TextureTap::new(11.0 + smear * 9.0, 0.22, -0.0016 * drift, sample_rate),
            ],
            left_filter: OnePoleLowpass::new(sample_rate, cutoff_hz * 0.9),
            right_filter: OnePoleLowpass::new(sample_rate, cutoff_hz * 1.1),
            level,
            feedback,
            params: TextureParams { drift: 1.0 },
        }
    }

    pub fn set_controls(&mut self, controls: GardenControls) {
        self.level = texture_level(controls);
        self.feedback = texture_feedback(controls);

        let cutoff_hz = texture_cutoff_hz(controls);
        self.left_filter.set_cutoff(cutoff_hz * 0.9);
        self.right_filter.set_cutoff(cutoff_hz * 1.1);
    }

    pub fn set_params(&mut self, params: TextureParams) {
        self.params = params.clamped();
        for tap in &mut self.taps {
            tap.set_drift_scale(self.params.drift);
        }
    }

    pub fn process(&mut self, input: StereoSample) -> StereoSample {
        // Tape memory signal flow:
        // 1. read delayed drifting taps from the circular buffer
        // 2. darken and scale that memory as the returned texture signal
        // 3. write the current dry input plus conservative texture feedback back into the buffer
        if self.level <= 0.0 {
            self.write(input);
            return StereoSample::default();
        }

        let mut output = StereoSample::default();
        for tap in &mut self.taps {
            output += tap.read(&self.buffer, self.write_index);
            tap.advance(self.buffer.len());
        }

        let texture = StereoSample::new(
            self.left_filter.process(output.left),
            self.right_filter.process(output.right),
        )
        .scale(self.level);
        self.write(input + texture.scale(self.feedback));

        texture
    }

    fn write(&mut self, input: StereoSample) {
        self.buffer[self.write_index] = input;
        self.write_index = (self.write_index + 1) % self.buffer.len();
    }
}

struct TextureTap {
    delay_samples: f32,
    gain: f32,
    base_drift_per_sample: f32,
    drift_per_sample: f32,
}

impl TextureTap {
    fn new(delay_seconds: f32, gain: f32, drift_per_sample: f32, sample_rate: f32) -> Self {
        Self {
            delay_samples: delay_seconds * sample_rate,
            gain,
            base_drift_per_sample: drift_per_sample,
            drift_per_sample,
        }
    }

    fn set_drift_scale(&mut self, scale: f32) {
        self.drift_per_sample = self.base_drift_per_sample * scale;
    }

    fn read(&self, buffer: &[StereoSample], write_index: usize) -> StereoSample {
        let len = buffer.len();
        let read_position = (write_index as f32 - self.delay_samples).rem_euclid(len as f32);
        let index_a = read_position.floor() as usize % len;
        let index_b = (index_a + 1) % len;
        let fraction = read_position.fract();
        let a = buffer[index_a];
        let b = buffer[index_b];

        StereoSample::new(
            lerp(a.left, b.left, fraction),
            lerp(a.right, b.right, fraction),
        )
        .scale(self.gain)
    }

    fn advance(&mut self, buffer_len: usize) {
        self.delay_samples = (self.delay_samples + self.drift_per_sample)
            .clamp(1.0, buffer_len.saturating_sub(2) as f32);
    }
}

fn lerp(a: f32, b: f32, amount: f32) -> f32 {
    a + (b - a) * amount
}

fn texture_level(controls: GardenControls) -> f32 {
    controls.texture_level * (0.18 + controls.space * 0.22)
}

fn texture_feedback(controls: GardenControls) -> f32 {
    (0.04 + controls.space * 0.28).min(0.34)
}

fn texture_cutoff_hz(controls: GardenControls) -> f32 {
    350.0 + controls.brightness * 3_500.0
}
