use std::error::Error;
use std::path::{Path, PathBuf};

use crate::composition::garden::{GardenControls, SampleParams};
use crate::dsp::random::SimpleRng;
use crate::dsp::sample::StereoSample;
use crate::dsp::source::StereoSource;
use crate::instruments::Instrument;

const SAMPLER_VOICE_COUNT: usize = 16;

#[derive(Clone)]
pub struct LoadedSample {
    path: PathBuf,
    frames: Vec<StereoSample>,
    sample_rate: u32,
}

#[derive(Clone)]
pub struct LoadedSampleAsset {
    name: String,
    sample: LoadedSample,
}

impl LoadedSample {
    pub fn from_wav_path(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let path = path.as_ref().to_path_buf();
        let mut reader = hound::WavReader::open(&path)?;
        let spec = reader.spec();
        let channel_count = spec.channels.max(1) as usize;

        let samples = match spec.sample_format {
            hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<Vec<_>, _>>()?,
            hound::SampleFormat::Int => {
                let scale =
                    ((1_i64 << (spec.bits_per_sample.saturating_sub(1) as u32)) - 1).max(1) as f32;
                reader
                    .samples::<i32>()
                    .map(|sample| sample.map(|value| value as f32 / scale))
                    .collect::<Result<Vec<_>, _>>()?
            }
        };

        let mut frames = Vec::with_capacity(samples.len() / channel_count.max(1));
        for frame in samples.chunks(channel_count) {
            let left = frame.first().copied().unwrap_or_default();
            let right = if channel_count == 1 {
                left
            } else {
                frame.get(1).copied().unwrap_or(left)
            };
            frames.push(StereoSample::new(left, right));
        }

        Ok(Self {
            path,
            frames,
            sample_rate: spec.sample_rate.max(1),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    fn frame_at(&self, position: f32) -> StereoSample {
        if self.frames.is_empty() {
            return StereoSample::default();
        }

        let clamped = position.clamp(0.0, self.frames.len().saturating_sub(1) as f32);
        let index_a = clamped.floor() as usize;
        let index_b = (index_a + 1).min(self.frames.len() - 1);
        let fraction = clamped.fract();
        let a = self.frames[index_a];
        let b = self.frames[index_b];

        StereoSample::new(
            lerp(a.left, b.left, fraction),
            lerp(a.right, b.right, fraction),
        )
    }
}

impl LoadedSampleAsset {
    pub fn new(name: impl Into<String>, sample: LoadedSample) -> Self {
        Self {
            name: name.into(),
            sample,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn path(&self) -> &Path {
        self.sample.path()
    }

    pub fn duration_seconds(&self) -> f32 {
        self.sample.len() as f32 / self.sample.sample_rate() as f32
    }
}

pub struct SamplerInstrument {
    samples: Vec<LoadedSampleAsset>,
    voices: Vec<SamplerVoice>,
    rng: SimpleRng,
    controls: GardenControls,
    sample_rate: f32,
    samples_until_trigger: usize,
    params: SampleParams,
    explicit_triggering: bool,
    active: bool,
}

impl SamplerInstrument {
    pub fn new(
        sample_rate: f32,
        samples: Vec<LoadedSampleAsset>,
        seed: u64,
        controls: GardenControls,
    ) -> Self {
        Self {
            samples,
            voices: (0..SAMPLER_VOICE_COUNT)
                .map(|_| SamplerVoice::default())
                .collect(),
            rng: SimpleRng::new(seed),
            controls,
            sample_rate,
            samples_until_trigger: 0,
            params: SampleParams { auto_rate: 1.0 },
            explicit_triggering: false,
            active: true,
        }
    }

    fn tick_trigger(&mut self) {
        if self.explicit_triggering || self.controls.sample_level <= 0.0 || self.samples.is_empty()
        {
            return;
        }

        if self.samples[0].sample.is_empty() {
            return;
        }

        if self.samples_until_trigger > 0 {
            self.samples_until_trigger -= 1;
            return;
        }

        self.trigger_voice();
        self.samples_until_trigger = self.next_trigger_interval_samples();
    }

    pub fn set_explicit_triggering(&mut self, explicit_triggering: bool) {
        self.explicit_triggering = explicit_triggering;
    }

    pub fn set_params(&mut self, params: SampleParams) {
        self.params = params.clamped();
    }

    pub fn trigger_once(
        &mut self,
        sample_name: &str,
        start_seconds: Option<f32>,
        end_seconds: Option<f32>,
        fade_in_seconds: Option<f32>,
        fade_out_seconds: Option<f32>,
        semitones: Option<f32>,
        cents: Option<f32>,
        gain: Option<f32>,
        pan: Option<f32>,
        rate: Option<f32>,
    ) {
        let Some(sample_index) = self
            .samples
            .iter()
            .position(|sample| sample.name() == sample_name)
        else {
            return;
        };

        let sample = &self.samples[sample_index].sample;
        if sample.is_empty() {
            return;
        }

        let base_rate = sample.sample_rate() as f32 / self.sample_rate.max(1.0);
        let gain = self.controls.sample_level * gain.unwrap_or(1.0).clamp(0.0, 1.0);
        if gain <= 0.0 {
            return;
        }

        let start_position = seconds_to_position(sample, start_seconds.unwrap_or(0.0));
        let end_position = end_seconds
            .map(|seconds| seconds_to_position(sample, seconds))
            .unwrap_or_else(|| sample.len().saturating_sub(1) as f32);
        if end_position <= start_position {
            return;
        }
        let fade_in_samples = seconds_to_samples(sample, fade_in_seconds.unwrap_or(0.0));
        let fade_out_samples = seconds_to_samples(sample, fade_out_seconds.unwrap_or(0.0));

        let pan = pan.unwrap_or(0.0).clamp(-1.0, 1.0);
        let transpose_ratio = transpose_ratio(semitones.unwrap_or(0.0), cents.unwrap_or(0.0));
        let rate = (base_rate * transpose_ratio * rate.unwrap_or(1.0).max(0.05)).max(0.05);
        self.trigger_voice_with(
            sample_index,
            start_position,
            end_position,
            fade_in_samples,
            fade_out_samples,
            gain,
            pan,
            rate,
        );
    }

    fn trigger_voice(&mut self) {
        if self.samples.is_empty() {
            return;
        }

        let sample_index = 0;
        let sample = &self.samples[sample_index].sample;
        let base_rate = sample.sample_rate() as f32 / self.sample_rate.max(1.0);
        let instability = self.controls.instability;
        let rate_variation = self.rng.range_f32(
            1.0 - (0.05 + instability * 0.30),
            1.0 + (0.08 + instability * 0.35),
        );
        let gain = self.controls.sample_level * self.rng.range_f32(0.18, 0.65);
        let pan_range = 0.2 + self.controls.space * 0.65;
        let pan = self.rng.range_f32(-pan_range, pan_range);
        let rate = (base_rate * rate_variation).max(0.05);

        let end_position = sample.len().saturating_sub(1) as f32;
        self.trigger_voice_with(sample_index, 0.0, end_position, 0.0, 0.0, gain, pan, rate);
    }

    fn trigger_voice_with(
        &mut self,
        sample_index: usize,
        start_position: f32,
        end_position: f32,
        fade_in_samples: f32,
        fade_out_samples: f32,
        gain: f32,
        pan: f32,
        rate: f32,
    ) {
        let Some(voice) = self.voices.iter_mut().find(|voice| !voice.active) else {
            return;
        };

        voice.active = true;
        voice.sample_index = sample_index;
        voice.position = start_position;
        voice.start_position = start_position;
        voice.end_position = end_position;
        voice.fade_in_samples = fade_in_samples;
        voice.fade_out_samples = fade_out_samples;
        voice.rate = rate;
        voice.gain = gain;
        voice.pan = pan;
    }

    fn next_trigger_interval_samples(&mut self) -> usize {
        let shortest_seconds = 0.7 + (1.0 - self.controls.density) * 0.9;
        let longest_seconds = 2.0 + (1.0 - self.controls.density) * 5.0;
        let seconds = self.rng.range_f32(shortest_seconds, longest_seconds) / self.params.auto_rate;
        (seconds * self.sample_rate).round().max(1.0) as usize
    }
}

impl Instrument for SamplerInstrument {
    fn set_controls(&mut self, controls: GardenControls) {
        self.controls = controls;
    }

    fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

impl StereoSource for SamplerInstrument {
    fn next_stereo(&mut self) -> StereoSample {
        if !self.active {
            return StereoSample::default();
        }

        self.tick_trigger();

        let mut output = StereoSample::default();
        for voice in &mut self.voices {
            if !voice.active {
                continue;
            }

            let sample = &self.samples[voice.sample_index].sample;
            let envelope = voice_envelope(voice);
            let frame = sample.frame_at(voice.position).scale(voice.gain * envelope);
            let left_gain = (1.0 - voice.pan) * 0.5;
            let right_gain = (1.0 + voice.pan) * 0.5;
            output += StereoSample::new(frame.left * left_gain, frame.right * right_gain);

            voice.position += voice.rate;
            if voice.position >= voice.end_position {
                voice.active = false;
            }
        }

        output
    }
}

#[derive(Default)]
struct SamplerVoice {
    active: bool,
    sample_index: usize,
    position: f32,
    start_position: f32,
    end_position: f32,
    fade_in_samples: f32,
    fade_out_samples: f32,
    rate: f32,
    gain: f32,
    pan: f32,
}

fn lerp(a: f32, b: f32, amount: f32) -> f32 {
    a + (b - a) * amount
}

fn seconds_to_position(sample: &LoadedSample, seconds: f32) -> f32 {
    let position = seconds.max(0.0) * sample.sample_rate() as f32;
    position.clamp(0.0, sample.len().saturating_sub(1) as f32)
}

fn seconds_to_samples(sample: &LoadedSample, seconds: f32) -> f32 {
    (seconds.max(0.0) * sample.sample_rate() as f32).max(0.0)
}

fn transpose_ratio(semitones: f32, cents: f32) -> f32 {
    2.0_f32.powf((semitones + cents / 100.0) / 12.0)
}

fn voice_envelope(voice: &SamplerVoice) -> f32 {
    let fade_in = if voice.fade_in_samples > 0.0 {
        ((voice.position - voice.start_position) / voice.fade_in_samples).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let fade_out = if voice.fade_out_samples > 0.0 {
        ((voice.end_position - voice.position) / voice.fade_out_samples).clamp(0.0, 1.0)
    } else {
        1.0
    };

    fade_in.min(fade_out)
}
