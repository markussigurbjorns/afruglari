use crate::grid::Event;
use std::fs::File;
use std::io::{self, BufWriter, Write};

#[derive(Clone, Copy, Debug)]
pub struct RenderConfig {
    pub sample_rate: u32,
    pub step_seconds: f32,
    pub tail_seconds: f32,
    pub mode: RenderMode,
    pub stereo_width: f32,
    pub delay_mix: f32,
    pub delay_feedback: f32,
    pub delay_seconds: f32,
    pub drive: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderMode {
    Percussive,
    Drone,
    BrokenRadio,
    Metallic,
    NoiseOrgan,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44_100,
            step_seconds: 0.16,
            tail_seconds: 1.5,
            mode: RenderMode::Percussive,
            stereo_width: 0.75,
            delay_mix: 0.12,
            delay_feedback: 0.28,
            delay_seconds: 0.33,
            drive: 1.15,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct StereoSample {
    left: f32,
    right: f32,
}

pub fn render_events_to_wav(
    events: &[Event],
    path: impl AsRef<std::path::Path>,
    config: RenderConfig,
) -> io::Result<()> {
    let sample_rate = config.sample_rate;
    let max_step = events.iter().map(|event| event.step).max().unwrap_or(0);
    let total_seconds = (max_step as f32 + 1.0) * config.step_seconds + config.tail_seconds;
    let mut samples =
        vec![StereoSample::default(); (total_seconds * sample_rate as f32) as usize + 1];

    for event in events {
        render_event(event, &mut samples, config);
    }

    apply_delay(&mut samples, config);
    soft_limit(&mut samples, config.drive);
    write_wav_stereo_i16(path, sample_rate, &samples)
}

fn render_event(event: &Event, samples: &mut [StereoSample], config: RenderConfig) {
    let start = (event.step as f32 * config.step_seconds * config.sample_rate as f32) as usize;
    let duration = event.duration_steps as f32 * config.step_seconds;
    let register = event.register.unwrap_or(0);
    let amp = 0.08 + event.intensity as f32 * 0.035;
    let timbre = event.timbre as f32;
    let mut mono = vec![0.0_f32; samples.len().saturating_sub(start)];

    match config.mode {
        RenderMode::Percussive => match event.voice % 3 {
            0 => render_fm_pulse(
                &mut mono,
                0,
                config.sample_rate,
                duration,
                register,
                timbre,
                amp,
            ),
            1 => render_metallic_hit(
                &mut mono,
                0,
                config.sample_rate,
                duration,
                register,
                timbre,
                amp,
            ),
            _ => render_noise_cloud(
                &mut mono,
                0,
                config.sample_rate,
                duration,
                register,
                timbre,
                amp,
            ),
        },
        RenderMode::Drone => render_drone(
            &mut mono,
            0,
            config.sample_rate,
            duration,
            register,
            timbre,
            amp,
        ),
        RenderMode::BrokenRadio => render_broken_radio(
            &mut mono,
            0,
            config.sample_rate,
            duration,
            register,
            timbre,
            amp,
            event.voice,
        ),
        RenderMode::Metallic => render_metallic_hit(
            &mut mono,
            0,
            config.sample_rate,
            duration * 1.8,
            register.saturating_add(event.voice as u8),
            timbre + event.voice as f32,
            amp * 0.9,
        ),
        RenderMode::NoiseOrgan => render_noise_organ(
            &mut mono,
            0,
            config.sample_rate,
            duration,
            register,
            timbre,
            amp,
            event.voice,
        ),
    }

    mix_mono_event(samples, start, &mono, event.voice, config.stereo_width);
}

fn render_fm_pulse(
    samples: &mut [f32],
    start: usize,
    sample_rate: u32,
    duration: f32,
    register: u8,
    timbre: f32,
    amp: f32,
) {
    let len = ((duration + 0.35) * sample_rate as f32) as usize;
    let carrier = 45.0 * 2.0_f32.powf(register as f32 * 0.22);
    let modulator = carrier * (1.5 + timbre * 0.17);

    for i in 0..len {
        let out = start + i;
        if out >= samples.len() {
            break;
        }
        let t = i as f32 / sample_rate as f32;
        let env = (-t * 9.0).exp();
        let click = (-t * 80.0).exp() * (t * 12_000.0).sin() * 0.08;
        let fm = (t * modulator * std::f32::consts::TAU).sin() * (6.0 + timbre);
        let body = ((t * carrier * std::f32::consts::TAU) + fm).sin();
        samples[out] += (body * env + click) * amp;
    }
}

fn render_metallic_hit(
    samples: &mut [f32],
    start: usize,
    sample_rate: u32,
    duration: f32,
    register: u8,
    timbre: f32,
    amp: f32,
) {
    let len = ((duration + 0.55) * sample_rate as f32) as usize;
    let base = 160.0 + register as f32 * 70.0 + timbre * 13.0;
    let ratios = [1.0, 1.37, 2.11, 2.92, 4.63];

    for i in 0..len {
        let out = start + i;
        if out >= samples.len() {
            break;
        }
        let t = i as f32 / sample_rate as f32;
        let env = (-t * (4.0 + timbre * 0.4)).exp();
        let mut value = 0.0;
        for (partial, ratio) in ratios.iter().enumerate() {
            let phase = t * base * ratio * std::f32::consts::TAU;
            value += phase.sin() * (1.0 / (partial as f32 + 1.0));
        }
        let folded = (value * (1.0 + timbre * 0.14)).sin();
        samples[out] += folded * env * amp * 0.8;
    }
}

fn render_noise_cloud(
    samples: &mut [f32],
    start: usize,
    sample_rate: u32,
    duration: f32,
    register: u8,
    timbre: f32,
    amp: f32,
) {
    let len = ((duration + 0.8) * sample_rate as f32) as usize;
    let mut state = 0x9e37_79b9_u32
        ^ ((start as u32).wrapping_mul(747_796_405))
        ^ ((register as u32) << 8)
        ^ timbre as u32;
    let resonator = 240.0 + register as f32 * 110.0 + timbre * 29.0;
    let mut last = 0.0_f32;

    for i in 0..len {
        let out = start + i;
        if out >= samples.len() {
            break;
        }
        let t = i as f32 / sample_rate as f32;
        let env = if t < 0.025 {
            t / 0.025
        } else {
            (-t * 1.8).exp()
        };

        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = ((state >> 8) as f32 / 16_777_216.0) * 2.0 - 1.0;
        let tone = (t * resonator * std::f32::consts::TAU).sin();
        last = last * 0.88 + noise * 0.12;
        let gate = if ((t * (18.0 + timbre * 3.0)) as u32) % 3 == 0 {
            1.0
        } else {
            0.35
        };
        samples[out] += (last * 0.75 + tone * 0.25) * env * gate * amp;
    }
}

fn render_drone(
    samples: &mut [f32],
    start: usize,
    sample_rate: u32,
    duration: f32,
    register: u8,
    timbre: f32,
    amp: f32,
) {
    let len = ((duration + 2.4) * sample_rate as f32) as usize;
    let base = 55.0 * 2.0_f32.powf(register as f32 * 0.16);
    let detune = 1.003 + timbre * 0.0009;

    for i in 0..len {
        let out = start + i;
        if out >= samples.len() {
            break;
        }
        let t = i as f32 / sample_rate as f32;
        let attack = (t / 0.18).min(1.0);
        let env = attack * (-t * 0.55).exp();
        let wobble = (t * (0.4 + timbre * 0.05) * std::f32::consts::TAU).sin() * 0.8;
        let low = (t * base * std::f32::consts::TAU + wobble).sin();
        let high = (t * base * 2.01 * detune * std::f32::consts::TAU).sin() * 0.45;
        let scrape = (low + high).tanh();
        samples[out] += scrape * env * amp * 0.7;
    }
}

fn render_broken_radio(
    samples: &mut [f32],
    start: usize,
    sample_rate: u32,
    duration: f32,
    register: u8,
    timbre: f32,
    amp: f32,
    voice: usize,
) {
    let len = ((duration + 0.45) * sample_rate as f32) as usize;
    let mut state = 0x6d2b_79f5_u32 ^ start as u32 ^ ((voice as u32) << 16);
    let carrier = 300.0 + register as f32 * 95.0 + timbre * 41.0;

    for i in 0..len {
        let out = start + i;
        if out >= samples.len() {
            break;
        }
        let t = i as f32 / sample_rate as f32;
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let noise = ((state & 0xffff) as f32 / 32_768.0) - 1.0;
        let gate = if ((t * (11.0 + timbre * 4.0)) as usize + voice) % 2 == 0 {
            1.0
        } else {
            0.05
        };
        let crushed = (noise * 9.0).round() / 9.0;
        let tone = (t * carrier * std::f32::consts::TAU).sin().signum() * 0.35;
        let env = (-t * 5.0).exp();
        samples[out] += (crushed * 0.65 + tone) * gate * env * amp;
    }
}

fn render_noise_organ(
    samples: &mut [f32],
    start: usize,
    sample_rate: u32,
    duration: f32,
    register: u8,
    timbre: f32,
    amp: f32,
    voice: usize,
) {
    let len = ((duration + 1.25) * sample_rate as f32) as usize;
    let mut state = 0x85eb_ca6b_u32 ^ start as u32 ^ ((register as u32) << 12);
    let band = 120.0 + register as f32 * 160.0 + voice as f32 * 53.0;
    let mut low = 0.0_f32;
    let mut high = 0.0_f32;

    for i in 0..len {
        let out = start + i;
        if out >= samples.len() {
            break;
        }
        let t = i as f32 / sample_rate as f32;
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let noise = ((state >> 9) as f32 / 8_388_608.0) * 2.0 - 1.0;
        let cutoff = (band + timbre * 70.0) / sample_rate as f32;
        low += cutoff.clamp(0.001, 0.2) * (noise - low);
        high += 0.025 * (low - high);
        let reed = (t * band * 0.5 * std::f32::consts::TAU).sin() * 0.2;
        let env = (t / 0.08).min(1.0) * (-t * 0.9).exp();
        samples[out] += (high + reed) * env * amp * 1.1;
    }
}

fn mix_mono_event(
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

fn voice_pan(voice: usize, width: f32) -> f32 {
    let base = match voice % 4 {
        0 => 0.18,
        1 => 0.50,
        2 => 0.82,
        _ => 0.35,
    };
    0.5 + (base - 0.5) * width
}

fn apply_delay(samples: &mut [StereoSample], config: RenderConfig) {
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

fn soft_limit(samples: &mut [StereoSample], drive: f32) {
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

fn write_wav_stereo_i16(
    path: impl AsRef<std::path::Path>,
    sample_rate: u32,
    samples: &[StereoSample],
) -> io::Result<()> {
    let channels = 2_u16;
    let bytes_per_sample = 2_u16;
    let data_len = samples.len() as u32 * channels as u32 * bytes_per_sample as u32;
    let mut writer = BufWriter::new(File::create(path)?);

    writer.write_all(b"RIFF")?;
    writer.write_all(&(36 + data_len).to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_all(&16_u32.to_le_bytes())?;
    writer.write_all(&1_u16.to_le_bytes())?;
    writer.write_all(&channels.to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&(sample_rate * channels as u32 * bytes_per_sample as u32).to_le_bytes())?;
    writer.write_all(&(channels * bytes_per_sample).to_le_bytes())?;
    writer.write_all(&16_u16.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_len.to_le_bytes())?;

    for sample in samples {
        let left = (sample.left.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        let right = (sample.right.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer.write_all(&left.to_le_bytes())?;
        writer.write_all(&right.to_le_bytes())?;
    }

    writer.flush()
}
