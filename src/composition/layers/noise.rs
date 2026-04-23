use crate::composition::garden::{GardenControls, NoiseParams};
use crate::dsp::filter::OnePoleLowpass;
use crate::dsp::random::SimpleRng;
use crate::dsp::sample::StereoSample;
use crate::dsp::smooth::SmoothedValue;
use crate::dsp::source::StereoSource;

const MODULATION_INTERVAL_SECONDS: f32 = 7.0;
const LEVEL_SMOOTHING_SECONDS: f32 = 6.0;
const PAN_SMOOTHING_SECONDS: f32 = 9.0;

pub struct NoiseLayer {
    rng: SimpleRng,
    left_filter: OnePoleLowpass,
    right_filter: OnePoleLowpass,
    level: SmoothedValue,
    pan: SmoothedValue,
    controls: GardenControls,
    samples_until_modulation: usize,
    modulation_interval_samples: usize,
    sample_rate: f32,
    params: NoiseParams,
}

impl NoiseLayer {
    pub fn new(sample_rate: f32, seed: u64, controls: GardenControls) -> Self {
        let cutoff_hz = cutoff_from_brightness(controls.brightness);
        let modulation_interval_samples =
            (MODULATION_INTERVAL_SECONDS * sample_rate).round() as usize;

        Self {
            rng: SimpleRng::new(seed),
            left_filter: OnePoleLowpass::new(sample_rate, cutoff_hz * 0.9),
            right_filter: OnePoleLowpass::new(sample_rate, cutoff_hz * 1.1),
            level: SmoothedValue::new(target_level(controls), LEVEL_SMOOTHING_SECONDS, sample_rate),
            pan: SmoothedValue::new(0.0, PAN_SMOOTHING_SECONDS, sample_rate),
            controls,
            samples_until_modulation: modulation_interval_samples,
            modulation_interval_samples,
            sample_rate,
            params: NoiseParams { motion: 1.0 },
        }
    }

    pub fn set_controls(&mut self, controls: GardenControls) {
        self.controls = controls;
        let cutoff_hz = cutoff_from_brightness(controls.brightness);

        self.left_filter.set_cutoff(cutoff_hz * 0.9);
        self.right_filter.set_cutoff(cutoff_hz * 1.1);
        self.level.set_target(target_level(controls));
    }

    pub fn set_params(&mut self, params: NoiseParams) {
        self.params = params.clamped();
        self.modulation_interval_samples =
            ((MODULATION_INTERVAL_SECONDS / self.params.motion.max(0.1)) * self.sample_rate)
                .round()
                .max(1.0) as usize;
        self.samples_until_modulation = self
            .samples_until_modulation
            .min(self.modulation_interval_samples);
    }

    fn tick_modulation(&mut self) {
        if self.samples_until_modulation > 0 {
            self.samples_until_modulation -= 1;
            return;
        }

        let motion = self.params.motion;
        let cutoff_hz = cutoff_from_brightness(self.controls.brightness)
            * self.rng.range_f32(1.0 - 0.25 * motion, 1.0 + 0.25 * motion);
        let level = target_level(self.controls)
            * self
                .rng
                .range_f32((1.0 - 0.35 * motion).max(0.2), 1.0 + 0.15 * motion);
        let pan_range = (0.25 + self.controls.instability * 0.6) * motion;

        self.left_filter.set_cutoff(cutoff_hz * 0.9);
        self.right_filter.set_cutoff(cutoff_hz * 1.1);
        self.level.set_target(level);
        self.pan
            .set_target(self.rng.range_f32(-pan_range, pan_range));
        self.samples_until_modulation = self.modulation_interval_samples;
    }
}

impl StereoSource for NoiseLayer {
    fn next_stereo(&mut self) -> StereoSample {
        self.tick_modulation();

        let left_noise = self.rng.range_f32(-1.0, 1.0);
        let right_noise = self.rng.range_f32(-1.0, 1.0);
        let left = self.left_filter.process(left_noise);
        let right = self.right_filter.process(right_noise);
        let level = self.level.next();
        let pan = self.pan.next();
        let left_gain = (1.0 - pan) * 0.5;
        let right_gain = (1.0 + pan) * 0.5;

        StereoSample::new(left * left_gain, right * right_gain).scale(level)
    }
}

fn target_level(controls: GardenControls) -> f32 {
    controls.noise_level * (0.025 + controls.density * 0.035)
}

fn cutoff_from_brightness(brightness: f32) -> f32 {
    180.0 + brightness * 3_200.0
}
