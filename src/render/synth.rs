use super::RenderConfig;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ToneControls {
    brightness: f32,
    roughness: f32,
    sustain: f32,
}

impl ToneControls {
    pub(crate) fn from_config(config: &RenderConfig) -> Self {
        Self {
            brightness: config.brightness.clamp(0.1, 4.0),
            roughness: config.roughness.clamp(0.0, 4.0),
            sustain: config.sustain.clamp(0.1, 6.0),
        }
    }
}

pub(crate) fn render_fm_pulse(
    samples: &mut [f32],
    start: usize,
    sample_rate: u32,
    duration: f32,
    register: u8,
    timbre: f32,
    amp: f32,
    tone: ToneControls,
) {
    let len = ((duration + 0.35 * tone.sustain) * sample_rate as f32) as usize;
    let carrier = 45.0 * tone.brightness * 2.0_f32.powf(register as f32 * 0.22);
    let modulator = carrier * (1.5 + timbre * 0.17 + tone.roughness * 0.15);

    for i in 0..len {
        let out = start + i;
        if out >= samples.len() {
            break;
        }
        let t = i as f32 / sample_rate as f32;
        let env = (-t * (9.0 / tone.sustain)).exp();
        let click =
            (-t * 80.0).exp() * (t * 12_000.0 * tone.brightness).sin() * 0.08 * tone.roughness;
        let fm = (t * modulator * std::f32::consts::TAU).sin() * (6.0 + timbre) * tone.roughness;
        let body = ((t * carrier * std::f32::consts::TAU) + fm).sin();
        samples[out] += (body * env + click) * amp;
    }
}

pub(crate) fn render_metallic_hit(
    samples: &mut [f32],
    start: usize,
    sample_rate: u32,
    duration: f32,
    register: u8,
    timbre: f32,
    amp: f32,
    tone: ToneControls,
) {
    let len = ((duration + 0.55 * tone.sustain) * sample_rate as f32) as usize;
    let base = (160.0 + register as f32 * 70.0 + timbre * 13.0) * tone.brightness;
    let ratios = [
        1.0,
        1.37 + tone.roughness * 0.03,
        2.11 + tone.roughness * 0.05,
        2.92,
        4.63 + tone.brightness * 0.07,
    ];

    for i in 0..len {
        let out = start + i;
        if out >= samples.len() {
            break;
        }
        let t = i as f32 / sample_rate as f32;
        let env = (-t * ((4.0 + timbre * 0.4) / tone.sustain)).exp();
        let mut value = 0.0;
        for (partial, ratio) in ratios.iter().enumerate() {
            let phase = t * base * ratio * std::f32::consts::TAU;
            value += phase.sin() * (1.0 / (partial as f32 + 1.0));
        }
        let folded = (value * (1.0 + timbre * 0.14 + tone.roughness * 0.2)).sin();
        samples[out] += folded * env * amp * 0.8;
    }
}

pub(crate) fn render_noise_cloud(
    samples: &mut [f32],
    start: usize,
    sample_rate: u32,
    duration: f32,
    register: u8,
    timbre: f32,
    amp: f32,
    tone: ToneControls,
) {
    let len = ((duration + 0.8 * tone.sustain) * sample_rate as f32) as usize;
    let mut state = 0x9e37_79b9_u32
        ^ ((start as u32).wrapping_mul(747_796_405))
        ^ ((register as u32) << 8)
        ^ timbre as u32;
    let resonator = (240.0 + register as f32 * 110.0 + timbre * 29.0) * tone.brightness;
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
            (-t * (1.8 / tone.sustain)).exp()
        };

        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = ((state >> 8) as f32 / 16_777_216.0) * 2.0 - 1.0;
        let resonant_tone = (t * resonator * std::f32::consts::TAU).sin();
        last = last * (0.94 - tone.roughness * 0.04).clamp(0.5, 0.98)
            + noise * (0.06 + tone.roughness * 0.08).clamp(0.02, 0.5);
        let gate = if ((t * (18.0 + timbre * 3.0 + tone.roughness * 6.0)) as u32) % 3 == 0 {
            1.0
        } else {
            0.35
        };
        samples[out] += (last * 0.75 + resonant_tone * 0.25) * env * gate * amp;
    }
}

pub(crate) fn render_drone(
    samples: &mut [f32],
    start: usize,
    sample_rate: u32,
    duration: f32,
    register: u8,
    timbre: f32,
    amp: f32,
    tone: ToneControls,
) {
    let len = ((duration + 2.4 * tone.sustain) * sample_rate as f32) as usize;
    let base = 55.0 * tone.brightness * 2.0_f32.powf(register as f32 * 0.16);
    let detune = 1.003 + timbre * 0.0009 + tone.roughness * 0.0008;

    for i in 0..len {
        let out = start + i;
        if out >= samples.len() {
            break;
        }
        let t = i as f32 / sample_rate as f32;
        let attack = (t / 0.18).min(1.0);
        let env = attack * (-t * (0.55 / tone.sustain)).exp();
        let wobble = (t * (0.4 + timbre * 0.05 + tone.roughness * 0.08) * std::f32::consts::TAU)
            .sin()
            * 0.8
            * tone.roughness.max(0.25);
        let low = (t * base * std::f32::consts::TAU + wobble).sin();
        let high = (t * base * 2.01 * detune * std::f32::consts::TAU).sin() * 0.45;
        let scrape = (low + high).tanh();
        samples[out] += scrape * env * amp * 0.7;
    }
}

pub(crate) fn render_impact_kit(
    samples: &mut [f32],
    start: usize,
    sample_rate: u32,
    duration: f32,
    register: u8,
    timbre: f32,
    amp: f32,
    voice: usize,
    tone: ToneControls,
) {
    let len = ((duration + 0.45 * tone.sustain) * sample_rate as f32) as usize;
    let mut state = 0x51f2_ac91_u32
        ^ start as u32
        ^ ((voice as u32) << 9)
        ^ ((register as u32) << 4)
        ^ timbre as u32;

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

        let value = match voice % 3 {
            0 => {
                let base = (38.0 + register as f32 * 6.0) * tone.brightness.max(0.4);
                let drop = 1.0 + (-t * 28.0).exp() * (1.8 + timbre * 0.08);
                let phase = t * base * drop * std::f32::consts::TAU;
                let body = phase.sin();
                let sub = (phase * 0.5).sin() * 0.45;
                let click = (-t * 85.0).exp() * noise * (0.18 + tone.roughness * 0.04);
                let env = (-t * (11.0 / tone.sustain)).exp();
                (body + sub) * env * 1.2 + click
            }
            1 => {
                let ring = (220.0 + register as f32 * 55.0 + timbre * 12.0) * tone.brightness;
                let wire = (t * ring * std::f32::consts::TAU).sin() * 0.28;
                let grain = noise * (0.78 + tone.roughness * 0.18);
                let gate = if ((t * (52.0 + timbre * 3.5)) as usize) % 2 == 0 {
                    1.0
                } else {
                    0.55
                };
                let env = (-t * ((20.0 + timbre * 0.9) / tone.sustain)).exp();
                (grain + wire) * env * gate
            }
            _ => {
                let base = (170.0 + register as f32 * 68.0 + timbre * 18.0) * tone.brightness;
                let partials = [
                    1.0,
                    1.41 + tone.roughness * 0.03,
                    2.27 + timbre * 0.01,
                    3.18 + tone.roughness * 0.05,
                ];
                let mut metal = 0.0_f32;
                for (index, ratio) in partials.iter().enumerate() {
                    metal += (t * base * ratio * std::f32::consts::TAU).sin()
                        / (index as f32 + 1.0);
                }
                let scrape = noise * (-t * 34.0).exp() * (0.20 + tone.roughness * 0.06);
                let env = (-t * ((9.0 + timbre * 0.5) / tone.sustain)).exp();
                (metal * (1.0 + tone.roughness * 0.16)).sin() * env * 0.95 + scrape
            }
        };

        samples[out] += value * amp;
    }
}

pub(crate) fn render_techno_pulse(
    samples: &mut [f32],
    start: usize,
    sample_rate: u32,
    duration: f32,
    register: u8,
    timbre: f32,
    amp: f32,
    voice: usize,
    tone: ToneControls,
) {
    let len = ((duration + 0.60 * tone.sustain) * sample_rate as f32) as usize;
    let mut state = 0x4f1b_bc7d_u32
        ^ start as u32
        ^ ((voice as u32) << 11)
        ^ ((register as u32) << 5)
        ^ timbre as u32;

    for i in 0..len {
        let out = start + i;
        if out >= samples.len() {
            break;
        }

        let t = i as f32 / sample_rate as f32;
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = ((state >> 8) as f32 / 16_777_216.0) * 2.0 - 1.0;

        let value = match voice % 3 {
            0 => {
                let base = (42.0 + register as f32 * 5.0) * tone.brightness.max(0.4);
                let pitch_drop = 1.0 + (-t * 34.0).exp() * (2.3 + timbre * 0.05);
                let phase = t * base * pitch_drop * std::f32::consts::TAU;
                let body = phase.sin();
                let sub = (phase * 0.5).sin() * 0.55;
                let click = (-t * 120.0).exp() * (noise + (t * 7_000.0).sin() * 0.25);
                let env = (-t * (13.0 / tone.sustain)).exp();
                (body + sub) * env * 1.35 + click * (0.16 + tone.roughness * 0.03)
            }
            1 => {
                let hat_rate = 7_500.0 + timbre * 320.0 + tone.brightness * 900.0;
                let ring = (t * hat_rate * std::f32::consts::TAU).sin().signum() * 0.12;
                let gate = if ((t * (130.0 + timbre * 8.0)) as usize) % 2 == 0 {
                    1.0
                } else {
                    0.35
                };
                let env = (-t * ((40.0 + timbre * 2.0) / tone.sustain)).exp();
                (noise * (0.82 + tone.roughness * 0.14) + ring) * env * gate
            }
            _ => {
                let stab_freq = (95.0 + register as f32 * 22.0 + timbre * 8.0) * tone.brightness;
                let detune = 1.004 + tone.roughness * 0.0015;
                let osc_a = (t * stab_freq * std::f32::consts::TAU).sin();
                let osc_b = (t * stab_freq * detune * 1.99 * std::f32::consts::TAU).sin() * 0.48;
                let chord = (osc_a + osc_b).tanh();
                let filter = (t * stab_freq * 3.0 * std::f32::consts::TAU).sin() * 0.10;
                let attack = (t / 0.01).min(1.0);
                let env = attack * (-t * (7.5 / tone.sustain)).exp();
                (chord + filter + noise * 0.04 * tone.roughness) * env * 0.95
            }
        };

        samples[out] += value * amp;
    }
}

pub(crate) fn render_broken_radio(
    samples: &mut [f32],
    start: usize,
    sample_rate: u32,
    duration: f32,
    register: u8,
    timbre: f32,
    amp: f32,
    voice: usize,
    tone: ToneControls,
) {
    let len = ((duration + 0.45 * tone.sustain) * sample_rate as f32) as usize;
    let mut state = 0x6d2b_79f5_u32 ^ start as u32 ^ ((voice as u32) << 16);
    let carrier = (300.0 + register as f32 * 95.0 + timbre * 41.0) * tone.brightness;

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
        let gate = if ((t * (11.0 + timbre * 4.0 + tone.roughness * 5.0)) as usize + voice) % 2 == 0
        {
            1.0
        } else {
            0.05
        };
        let crush_steps = (14.0 - tone.roughness * 4.0).clamp(3.0, 24.0);
        let crushed = (noise * crush_steps).round() / crush_steps;
        let radio_tone = (t * carrier * std::f32::consts::TAU).sin().signum() * 0.35;
        let env = (-t * (5.0 / tone.sustain)).exp();
        samples[out] += (crushed * 0.65 + radio_tone) * gate * env * amp;
    }
}

pub(crate) fn render_noise_organ(
    samples: &mut [f32],
    start: usize,
    sample_rate: u32,
    duration: f32,
    register: u8,
    timbre: f32,
    amp: f32,
    voice: usize,
    tone: ToneControls,
) {
    let len = ((duration + 1.25 * tone.sustain) * sample_rate as f32) as usize;
    let mut state = 0x85eb_ca6b_u32 ^ start as u32 ^ ((register as u32) << 12);
    let band = (120.0 + register as f32 * 160.0 + voice as f32 * 53.0) * tone.brightness;
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
        let cutoff = (band + timbre * 70.0 * tone.brightness) / sample_rate as f32;
        low += cutoff.clamp(0.001, 0.2) * (noise - low);
        high += (0.015 + tone.roughness * 0.015).clamp(0.005, 0.08) * (low - high);
        let reed =
            (t * band * 0.5 * std::f32::consts::TAU).sin() * 0.2 * (1.0 + tone.roughness * 0.2);
        let env = (t / 0.08).min(1.0) * (-t * (0.9 / tone.sustain)).exp();
        samples[out] += (high * (0.8 + tone.roughness * 0.25) + reed) * env * amp * 1.1;
    }
}

pub(crate) fn render_granular_dust(
    samples: &mut [f32],
    start: usize,
    sample_rate: u32,
    duration: f32,
    register: u8,
    timbre: f32,
    amp: f32,
    voice: usize,
    tone: ToneControls,
) {
    let len = ((duration + 0.6 * tone.sustain) * sample_rate as f32) as usize;
    let mut state = 0xa511_e9b3_u32
        ^ ((start as u32).wrapping_mul(2_654_435_761))
        ^ ((voice as u32) << 18)
        ^ ((register as u32) << 9)
        ^ timbre as u32;
    let grain_rate = (45.0 + timbre * 18.0 + tone.roughness * 22.0).max(1.0);
    let grain_samples = (sample_rate as f32 / grain_rate) as usize;
    let resonator = (900.0 + register as f32 * 360.0 + timbre * 83.0) * tone.brightness;
    let mut band = 0.0_f32;

    for i in 0..len {
        let out = start + i;
        if out >= samples.len() {
            break;
        }
        let t = i as f32 / sample_rate as f32;
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = ((state >> 8) as f32 / 16_777_216.0) * 2.0 - 1.0;
        let grain_pos = if grain_samples == 0 {
            0.0
        } else {
            (i % grain_samples) as f32 / grain_samples as f32
        };
        let grain_env = (std::f32::consts::PI * grain_pos).sin().max(0.0).powf(2.0);
        let trigger = if grain_samples == 0 || i % grain_samples < grain_samples / 3 {
            1.0
        } else {
            0.18 + tone.roughness * 0.08
        };
        let cutoff = (0.015 + register as f32 * 0.004 + tone.brightness * 0.01).clamp(0.004, 0.12);
        band += cutoff * (noise - band);
        let tone_spark = (t * resonator * std::f32::consts::TAU).sin() * 0.35;
        let env = (-t * (2.2 / tone.sustain)).exp();
        samples[out] += (band * 0.85 + tone_spark) * grain_env * trigger * env * amp * 1.25;
    }
}

pub(crate) fn render_sub_machine(
    samples: &mut [f32],
    start: usize,
    sample_rate: u32,
    duration: f32,
    register: u8,
    timbre: f32,
    amp: f32,
    voice: usize,
    tone: ToneControls,
) {
    let len = ((duration + 1.0 * tone.sustain) * sample_rate as f32) as usize;
    let base = (28.0 + register as f32 * 5.5 + voice as f32 * 1.7) * tone.brightness.max(0.3);
    let pulse_rate = 4.0 + timbre * 0.65 + tone.roughness * 1.2;
    let mut state = 0x27d4_eb2d_u32 ^ start as u32 ^ ((voice as u32) << 12);
    let mut low = 0.0_f32;

    for i in 0..len {
        let out = start + i;
        if out >= samples.len() {
            break;
        }
        let t = i as f32 / sample_rate as f32;
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let grit = ((state & 0xffff) as f32 / 32_768.0 - 1.0) * tone.roughness * 0.08;
        let pulse_phase = (t * pulse_rate + voice as f32 * 0.17).fract();
        let gate = if pulse_phase < 0.44 { 1.0 } else { 0.18 };
        let thump = (-pulse_phase * (10.0 + tone.roughness * 6.0)).exp();
        let wobble = (t * (0.25 + timbre * 0.04) * std::f32::consts::TAU).sin() * 0.9;
        let phase = t * (base + wobble) * std::f32::consts::TAU;
        let sub = (phase.sin() + (phase * 0.5).sin() * 0.45).tanh();
        low += 0.035 * ((sub + grit) - low);
        let env = (t / 0.03).min(1.0) * (-t * (0.85 / tone.sustain)).exp();
        samples[out] += (low * gate + thump * 0.35) * env * amp * 1.45;
    }
}

pub(crate) fn render_glass_harmonics(
    samples: &mut [f32],
    start: usize,
    sample_rate: u32,
    duration: f32,
    register: u8,
    timbre: f32,
    amp: f32,
    voice: usize,
    tone: ToneControls,
) {
    let len = ((duration + 1.4 * tone.sustain) * sample_rate as f32) as usize;
    let fundamental = (180.0 + register as f32 * 95.0 + voice as f32 * 21.0) * tone.brightness;
    let ratios = [
        1.0,
        2.01 + timbre * 0.006,
        2.97 + tone.roughness * 0.018,
        4.18 + voice as f32 * 0.013,
        6.07 + timbre * 0.011,
    ];

    for i in 0..len {
        let out = start + i;
        if out >= samples.len() {
            break;
        }
        let t = i as f32 / sample_rate as f32;
        let strike = (-t * (18.0 / tone.sustain)).exp();
        let shimmer = (-t * (1.7 / tone.sustain)).exp();
        let mut value = 0.0_f32;
        for (partial, ratio) in ratios.iter().enumerate() {
            let detune = 1.0 + (partial as f32 * 0.0007 * (voice as f32 + 1.0));
            let phase = t * fundamental * ratio * detune * std::f32::consts::TAU;
            let decay = (-t * ((1.2 + partial as f32 * 0.55) / tone.sustain)).exp();
            value += phase.sin() * decay / (partial as f32 + 1.0);
        }
        let ping = (t * fundamental * 8.0 * std::f32::consts::TAU).sin() * strike * 0.18;
        let rough_edge = (value * (1.0 + tone.roughness * 0.18)).sin();
        samples[out] += (rough_edge * shimmer + ping) * amp * 0.95;
    }
}
