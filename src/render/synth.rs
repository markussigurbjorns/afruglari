use super::RenderConfig;
use super::dsp::{
    OnePoleHighpass, OnePoleLowpass, attack_decay_env, decay_env, noise_step, pitch_drop, sine_hz,
    transient_env,
};

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
    let mut body_low = OnePoleLowpass::new(
        sample_rate,
        (220.0 + register as f32 * 34.0 + tone.brightness * 70.0).clamp(90.0, 3_000.0),
    );
    let mut click_high = OnePoleHighpass::new(
        sample_rate,
        (1_800.0 + timbre * 150.0 + tone.brightness * 420.0).clamp(500.0, 10_000.0),
    );

    for i in 0..len {
        let out = start + i;
        if out >= samples.len() {
            break;
        }
        let t = i as f32 / sample_rate as f32;
        let env = decay_env(t, 9.0, tone.sustain);
        let fm = sine_hz(t, modulator) * (6.0 + timbre) * tone.roughness;
        let body = (sine_hz(t, carrier) + fm).sin();
        let shaped_body = body_low.process(body.tanh()) * env;
        let click_src = sine_hz(t, 12_000.0 * tone.brightness) * 0.08 * tone.roughness;
        let click = click_high.process(click_src) * transient_env(t, 80.0);
        samples[out] += (shaped_body + click) * amp;
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
    let mut tone_low = OnePoleLowpass::new(
        sample_rate,
        (3_200.0 + register as f32 * 280.0 + timbre * 90.0).clamp(900.0, 10_500.0),
    );
    let mut strike_high = OnePoleHighpass::new(
        sample_rate,
        (1_400.0 + timbre * 110.0 + tone.brightness * 300.0).clamp(400.0, 8_000.0),
    );
    let mut state = 0x7f4a_7c15_u32 ^ start as u32 ^ ((register as u32) << 10) ^ timbre as u32;

    for i in 0..len {
        let out = start + i;
        if out >= samples.len() {
            break;
        }
        let t = i as f32 / sample_rate as f32;
        let env = decay_env(t, 4.0 + timbre * 0.4, tone.sustain);
        let mut value = 0.0;
        for (partial, ratio) in ratios.iter().enumerate() {
            value += sine_hz(t, base * ratio) * (1.0 / (partial as f32 + 1.0));
        }
        let folded = tone_low.process((value * (1.0 + timbre * 0.14 + tone.roughness * 0.2)).sin());
        let strike = strike_high.process(noise_step(&mut state)) * transient_env(t, 42.0);
        samples[out] += (folded * env + strike * (0.12 + tone.roughness * 0.05)) * amp * 0.8;
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
    let mut band_low = OnePoleLowpass::new(
        sample_rate,
        (1_200.0 + register as f32 * 180.0 + tone.brightness * 360.0).clamp(300.0, 7_000.0),
    );
    let mut air_high = OnePoleHighpass::new(
        sample_rate,
        (2_200.0 + timbre * 120.0 + tone.roughness * 300.0).clamp(700.0, 11_000.0),
    );

    for i in 0..len {
        let out = start + i;
        if out >= samples.len() {
            break;
        }
        let t = i as f32 / sample_rate as f32;
        let env = attack_decay_env(t, 0.025, 1.8, tone.sustain);

        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = ((state >> 8) as f32 / 16_777_216.0) * 2.0 - 1.0;
        let resonant_tone = sine_hz(t, resonator);
        let dust = band_low.process(noise) * 0.82;
        let air = air_high.process(noise) * (0.12 + tone.roughness * 0.08);
        let gate = if ((t * (18.0 + timbre * 3.0 + tone.roughness * 6.0)) as u32) % 3 == 0 {
            1.0
        } else {
            0.35
        };
        samples[out] += (dust + resonant_tone * 0.22 + air) * env * gate * amp;
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
    let mut body_low = OnePoleLowpass::new(
        sample_rate,
        (140.0 + register as f32 * 22.0 + tone.brightness * 40.0).clamp(60.0, 2_200.0),
    );
    let mut snap_high = OnePoleHighpass::new(
        sample_rate,
        (900.0 + timbre * 80.0 + tone.brightness * 280.0).clamp(300.0, 8_000.0),
    );
    let mut metal_tone = OnePoleLowpass::new(
        sample_rate,
        (2_800.0 + register as f32 * 240.0 + timbre * 110.0).clamp(800.0, 9_500.0),
    );

    for i in 0..len {
        let out = start + i;
        if out >= samples.len() {
            break;
        }

        let t = i as f32 / sample_rate as f32;
        let noise = noise_step(&mut state);

        let value = match voice % 3 {
            0 => {
                let base = (38.0 + register as f32 * 6.0) * tone.brightness.max(0.4);
                let freq = base * pitch_drop(t, 1.8 + timbre * 0.08, 28.0);
                let body = sine_hz(t, freq);
                let sub = sine_hz(t, freq * 0.5) * 0.45;
                let thump = body_low.process((body + sub).tanh());
                let click = snap_high.process(noise) * transient_env(t, 95.0);
                let env = decay_env(t, 11.0, tone.sustain);
                thump * env * 1.24 + click * (0.14 + tone.roughness * 0.05)
            }
            1 => {
                let ring = (220.0 + register as f32 * 55.0 + timbre * 12.0) * tone.brightness;
                let wire = sine_hz(t, ring) * 0.24;
                let grain = snap_high.process(noise) * (0.74 + tone.roughness * 0.16);
                let gate = if ((t * (48.0 + timbre * 3.5)) as usize) % 2 == 0 {
                    1.0
                } else {
                    0.48
                };
                let noise_env = decay_env(t, 24.0 + timbre * 0.9, tone.sustain);
                let ring_env = decay_env(t, 14.0, tone.sustain);
                (grain * noise_env + wire * ring_env) * gate
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
                    metal += sine_hz(t, base * ratio) / (index as f32 + 1.0);
                }
                let strike = metal_tone.process((metal * (1.0 + tone.roughness * 0.16)).sin());
                let scrape = snap_high.process(noise) * transient_env(t, 36.0);
                let env = decay_env(t, 9.0 + timbre * 0.5, tone.sustain);
                strike * env * 0.95 + scrape * (0.16 + tone.roughness * 0.08)
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
    let mut kick_low = OnePoleLowpass::new(
        sample_rate,
        (165.0 + register as f32 * 24.0 + tone.brightness * 55.0).clamp(70.0, 1_800.0),
    );
    let mut hat_high = OnePoleHighpass::new(
        sample_rate,
        (3_800.0 + timbre * 140.0 + tone.brightness * 600.0).clamp(1_400.0, 12_000.0),
    );
    let mut stab_low = OnePoleLowpass::new(
        sample_rate,
        (1_600.0 + register as f32 * 180.0 + tone.brightness * 420.0).clamp(400.0, 7_000.0),
    );
    let mut stab_high = OnePoleHighpass::new(
        sample_rate,
        (140.0 + register as f32 * 24.0).clamp(40.0, 1_100.0),
    );

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
                let freq = base * pitch_drop(t, 2.3 + timbre * 0.05, 34.0);
                let body = sine_hz(t, freq);
                let sub = sine_hz(t, freq * 0.5) * 0.55;
                let click = hat_high.process(noise + sine_hz(t, 7_000.0) * 0.25);
                let env = decay_env(t, 13.0, tone.sustain);
                kick_low.process((body + sub).tanh()) * env * 1.34
                    + click * transient_env(t, 125.0) * (0.14 + tone.roughness * 0.04)
            }
            1 => {
                let hat_rate = 7_500.0 + timbre * 320.0 + tone.brightness * 900.0;
                let ring = sine_hz(t, hat_rate).signum() * 0.10;
                let gate = if ((t * (130.0 + timbre * 8.0)) as usize) % 2 == 0 {
                    1.0
                } else {
                    0.35
                };
                let env = decay_env(t, 40.0 + timbre * 2.0, tone.sustain);
                let hat_noise = hat_high.process(noise) * (0.82 + tone.roughness * 0.14);
                (hat_noise + ring) * env * gate
            }
            _ => {
                let stab_freq = (95.0 + register as f32 * 22.0 + timbre * 8.0) * tone.brightness;
                let detune = 1.004 + tone.roughness * 0.0015;
                let osc_a = sine_hz(t, stab_freq);
                let osc_b = sine_hz(t, stab_freq * detune * 1.99) * 0.48;
                let filter = sine_hz(t, stab_freq * 3.0) * 0.10;
                let chord = stab_high.process(stab_low.process((osc_a + osc_b + filter).tanh()));
                let env = attack_decay_env(t, 0.01, 7.5, tone.sustain);
                (chord + noise * 0.035 * tone.roughness) * env * 0.95
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
    let mut hiss_high = OnePoleHighpass::new(
        sample_rate,
        (2_400.0 + timbre * 160.0 + tone.brightness * 500.0).clamp(900.0, 11_000.0),
    );
    let mut body_low = OnePoleLowpass::new(
        sample_rate,
        (1_100.0 + register as f32 * 140.0 + tone.brightness * 260.0).clamp(250.0, 5_500.0),
    );

    for i in 0..len {
        let out = start + i;
        if out >= samples.len() {
            break;
        }
        let t = i as f32 / sample_rate as f32;
        let noise = noise_step(&mut state);
        let gate = if ((t * (11.0 + timbre * 4.0 + tone.roughness * 5.0)) as usize + voice) % 2 == 0
        {
            1.0
        } else {
            0.05
        };
        let crush_steps = (14.0 - tone.roughness * 4.0).clamp(3.0, 24.0);
        let crushed = (noise * crush_steps).round() / crush_steps;
        let hiss = hiss_high.process(crushed) * 0.65;
        let radio_tone = body_low.process(sine_hz(t, carrier).signum() * 0.35);
        let glitch = hiss_high.process(noise * sine_hz(t, carrier * 0.5)) * transient_env(t, 22.0);
        let env = decay_env(t, 5.0, tone.sustain);
        samples[out] += (hiss + radio_tone + glitch * 0.35) * gate * env * amp;
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
