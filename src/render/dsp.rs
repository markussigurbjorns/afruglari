#[derive(Clone, Copy, Debug)]
pub(crate) struct OnePoleLowpass {
    alpha: f32,
    state: f32,
}

impl OnePoleLowpass {
    pub(crate) fn new(sample_rate: u32, cutoff_hz: f32) -> Self {
        let normalized = (cutoff_hz / sample_rate as f32).clamp(0.0005, 0.45);
        let alpha = (std::f32::consts::TAU * normalized).min(0.95);
        Self { alpha, state: 0.0 }
    }

    pub(crate) fn process(&mut self, input: f32) -> f32 {
        self.state += self.alpha * (input - self.state);
        self.state
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct OnePoleHighpass {
    lowpass: OnePoleLowpass,
}

impl OnePoleHighpass {
    pub(crate) fn new(sample_rate: u32, cutoff_hz: f32) -> Self {
        Self {
            lowpass: OnePoleLowpass::new(sample_rate, cutoff_hz),
        }
    }

    pub(crate) fn process(&mut self, input: f32) -> f32 {
        input - self.lowpass.process(input)
    }
}

pub(crate) fn decay_env(t: f32, decay_rate: f32, sustain: f32) -> f32 {
    (-t * (decay_rate / sustain.max(0.05))).exp()
}

pub(crate) fn attack_decay_env(t: f32, attack: f32, decay_rate: f32, sustain: f32) -> f32 {
    let attack = if attack <= 0.0 {
        1.0
    } else {
        (t / attack).min(1.0)
    };
    attack * decay_env(t, decay_rate, sustain)
}

pub(crate) fn transient_env(t: f32, rate: f32) -> f32 {
    (-t * rate.max(0.1)).exp()
}

pub(crate) fn pitch_drop(t: f32, depth: f32, rate: f32) -> f32 {
    1.0 + transient_env(t, rate) * depth.max(0.0)
}

pub(crate) fn sine_hz(t: f32, hz: f32) -> f32 {
    (t * hz * std::f32::consts::TAU).sin()
}

pub(crate) fn noise_step(state: &mut u32) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    ((*state & 0xffff) as f32 / 32_768.0) - 1.0
}
