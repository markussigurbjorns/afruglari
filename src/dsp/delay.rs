use crate::dsp::filter::OnePoleLowpass;
use crate::dsp::sample::StereoSample;

pub struct StereoDelay {
    left_buffer: Vec<f32>,
    right_buffer: Vec<f32>,
    left_index: usize,
    right_index: usize,
    feedback: f32,
    wet: f32,
    dry: f32,
    left_filter: OnePoleLowpass,
    right_filter: OnePoleLowpass,
}

impl StereoDelay {
    pub fn new(
        sample_rate: f32,
        left_delay_seconds: f32,
        right_delay_seconds: f32,
        feedback: f32,
        wet: f32,
        feedback_cutoff_hz: f32,
    ) -> Self {
        let left_len = delay_len(sample_rate, left_delay_seconds);
        let right_len = delay_len(sample_rate, right_delay_seconds);
        let wet = wet.clamp(0.0, 1.0);

        Self {
            left_buffer: vec![0.0; left_len],
            right_buffer: vec![0.0; right_len],
            left_index: 0,
            right_index: 0,
            feedback: feedback.clamp(0.0, 0.95),
            wet,
            dry: 1.0 - wet,
            left_filter: OnePoleLowpass::new(sample_rate, feedback_cutoff_hz),
            right_filter: OnePoleLowpass::new(sample_rate, feedback_cutoff_hz),
        }
    }

    pub fn process(&mut self, input: StereoSample) -> StereoSample {
        let delayed_left = self.left_buffer[self.left_index];
        let delayed_right = self.right_buffer[self.right_index];
        let feedback_left = self.left_filter.process(delayed_right) * self.feedback;
        let feedback_right = self.right_filter.process(delayed_left) * self.feedback;

        self.left_buffer[self.left_index] = input.left + feedback_left;
        self.right_buffer[self.right_index] = input.right + feedback_right;

        self.left_index = (self.left_index + 1) % self.left_buffer.len();
        self.right_index = (self.right_index + 1) % self.right_buffer.len();

        StereoSample::new(
            input.left * self.dry + delayed_left * self.wet,
            input.right * self.dry + delayed_right * self.wet,
        )
    }

    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback.clamp(0.0, 0.95);
    }

    pub fn set_wet(&mut self, wet: f32) {
        self.wet = wet.clamp(0.0, 1.0);
        self.dry = 1.0 - self.wet;
    }

    pub fn set_feedback_cutoff(&mut self, cutoff_hz: f32) {
        self.left_filter.set_cutoff(cutoff_hz);
        self.right_filter.set_cutoff(cutoff_hz);
    }
}

fn delay_len(sample_rate: f32, delay_seconds: f32) -> usize {
    (sample_rate * delay_seconds.max(0.001)).round().max(1.0) as usize
}
