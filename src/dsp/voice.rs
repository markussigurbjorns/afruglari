use std::f32::consts::TAU;

use crate::dsp::sample::StereoSample;
use crate::dsp::smooth::SmoothedValue;
use crate::dsp::source::StereoSource;

const FREQUENCY_SMOOTHING_SECONDS: f32 = 5.0;
const GAIN_SMOOTHING_SECONDS: f32 = 8.0;
const PAN_SMOOTHING_SECONDS: f32 = 10.0;

pub struct DroneVoice {
    frequency_hz: SmoothedValue,
    gain: SmoothedValue,
    pan: SmoothedValue,
    sample_rate: f32,
    phase: f32,
}

impl DroneVoice {
    pub fn new(frequency_hz: f32, gain: f32, pan: f32, sample_rate: f32) -> Self {
        let pan = pan.clamp(-1.0, 1.0);
        let mut smoothed_frequency_hz =
            SmoothedValue::new(frequency_hz, FREQUENCY_SMOOTHING_SECONDS, sample_rate);
        let mut smoothed_gain = SmoothedValue::new(gain, GAIN_SMOOTHING_SECONDS, sample_rate);
        let mut smoothed_pan = SmoothedValue::new(pan, PAN_SMOOTHING_SECONDS, sample_rate);

        smoothed_frequency_hz.set_target(frequency_hz);
        smoothed_gain.set_target(gain);
        smoothed_pan.set_target(pan);

        Self {
            frequency_hz: smoothed_frequency_hz,
            gain: smoothed_gain,
            pan: smoothed_pan,
            sample_rate,
            phase: 0.0,
        }
    }

    pub fn set_frequency_hz(&mut self, frequency_hz: f32) {
        self.frequency_hz.set_target(frequency_hz.max(1.0));
    }

    pub fn set_gain(&mut self, gain: f32) {
        self.gain.set_target(gain.clamp(0.0, 1.0));
    }

    pub fn set_pan(&mut self, pan: f32) {
        self.pan.set_target(pan.clamp(-1.0, 1.0));
    }

    fn next_sample(&mut self) -> f32 {
        let frequency_hz = self.frequency_hz.next();
        let gain = self.gain.next();
        let sample = (self.phase * TAU).sin() * gain;
        self.phase = (self.phase + frequency_hz / self.sample_rate).fract();
        sample
    }
}

impl StereoSource for DroneVoice {
    fn next_stereo(&mut self) -> StereoSample {
        let sample = self.next_sample();
        let pan = self.pan.next();
        let left_gain = (1.0 - pan) * 0.5;
        let right_gain = (1.0 + pan) * 0.5;

        StereoSample::new(sample * left_gain, sample * right_gain)
    }
}
