use super::RenderConfig;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct StereoSample {
    pub(crate) left: f32,
    pub(crate) right: f32,
}

pub(crate) fn mix_mono_event(
    samples: &mut [StereoSample],
    start: usize,
    mono: &[f32],
    voice: usize,
    stereo_width: f32,
) {
    let pan = voice_pan(voice, stereo_width.clamp(0.0, 1.0));
    let left_gain = (1.0 - pan).sqrt();
    let right_gain = pan.sqrt();

    for (index, sample) in mono.iter().copied().enumerate() {
        let out = start + index;
        if out >= samples.len() {
            break;
        }
        samples[out].left += sample * left_gain;
        samples[out].right += sample * right_gain;
    }
}

pub(crate) fn apply_delay(samples: &mut [StereoSample], config: RenderConfig) {
    let delay_samples = (config.delay_seconds * config.sample_rate as f32) as usize;
    if delay_samples == 0 || config.delay_mix <= 0.0 {
        return;
    }

    let mix = config.delay_mix.clamp(0.0, 1.0);
    let feedback = config.delay_feedback.clamp(0.0, 0.95);
    for index in delay_samples..samples.len() {
        let delayed = samples[index - delay_samples];
        samples[index].left += delayed.right * mix;
        samples[index].right += delayed.left * mix;
        samples[index].left += delayed.left * feedback * mix * 0.35;
        samples[index].right += delayed.right * feedback * mix * 0.35;
    }
}

pub(crate) fn soft_limit(samples: &mut [StereoSample], drive: f32) {
    let peak = samples
        .iter()
        .copied()
        .flat_map(|sample| [sample.left.abs(), sample.right.abs()])
        .fold(0.0_f32, f32::max);
    let gain = if peak > 0.95 { 0.95 / peak } else { 1.0 };

    for sample in samples {
        sample.left = (sample.left * gain * drive.max(0.1)).tanh();
        sample.right = (sample.right * gain * drive.max(0.1)).tanh();
    }
}

pub(crate) fn soft_limit_mono(samples: &mut [f32], drive: f32) {
    let gain = drive.max(0.1);
    for sample in samples {
        *sample = (*sample * gain).tanh() / gain;
    }
}

fn voice_pan(voice: usize, width: f32) -> f32 {
    let base = match voice % 4 {
        0 => 0.18,
        1 => 0.50,
        2 => 0.82,
        _ => 0.35,
    };
    0.5 + (base - 0.5) * width
}
