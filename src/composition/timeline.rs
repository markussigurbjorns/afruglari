#![allow(dead_code)]

use crate::composition::garden::{GardenControls, InstrumentParams};
use crate::composition::tuning::RegisterRange;
use crate::instruments::InstrumentFamily;

#[derive(Clone, Copy, Debug)]
pub struct ControlPoint {
    pub time_seconds: f32,
    pub state: TimelineState,
}

#[derive(Clone, Copy, Debug)]
pub struct TimelineState {
    pub controls: GardenControls,
    pub instrument_params: InstrumentParams,
    pub instrument_states: InstrumentStateMap,
    pub root_hz: f32,
    pub voice_count: usize,
    pub octave_min: i32,
    pub octave_max: i32,
    pub event_attack_min: f32,
    pub event_attack_max: f32,
    pub event_decay_min: f32,
    pub event_decay_max: f32,
    pub drone_retune_seconds: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct InstrumentStateMap {
    active: [bool; InstrumentFamily::COUNT],
    level_overrides: [Option<f32>; InstrumentFamily::COUNT],
}

impl Default for InstrumentStateMap {
    fn default() -> Self {
        Self {
            active: [true; InstrumentFamily::COUNT],
            level_overrides: [None; InstrumentFamily::COUNT],
        }
    }
}

impl ControlPoint {
    pub fn new(time_seconds: f32, state: TimelineState) -> Self {
        Self {
            time_seconds: time_seconds.max(0.0),
            state: state.clamped(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ControlTimeline {
    points: Vec<ControlPoint>,
}

impl ControlTimeline {
    pub fn new(mut points: Vec<ControlPoint>) -> Self {
        points.sort_by(|a, b| a.time_seconds.total_cmp(&b.time_seconds));
        Self { points }
    }

    pub fn constant(state: TimelineState) -> Self {
        Self::new(vec![ControlPoint::new(0.0, state)])
    }

    pub fn state_at(&self, time_seconds: f32) -> Option<TimelineState> {
        let time_seconds = time_seconds.max(0.0);
        let first = self.points.first()?;
        let last = self.points.last()?;

        if time_seconds <= first.time_seconds {
            return Some(first.state);
        }

        if time_seconds >= last.time_seconds {
            return Some(last.state);
        }

        if let Some(point) = self
            .points
            .iter()
            .rev()
            .find(|point| (point.time_seconds - time_seconds).abs() <= f32::EPSILON)
        {
            return Some(point.state);
        }

        self.points
            .windows(2)
            .find_map(|window| interpolate_window(window[0], window[1], time_seconds))
    }

    pub fn controls_at(&self, time_seconds: f32) -> Option<GardenControls> {
        self.state_at(time_seconds).map(|state| state.controls)
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

pub fn parse_timeline_text(
    text: &str,
    base_controls: GardenControls,
) -> Result<ControlTimeline, String> {
    parse_timeline_text_with_root(
        text,
        base_controls,
        110.0,
        3,
        1,
        2,
        0.015,
        0.195,
        2.0,
        8.0,
        9.0,
    )
}

pub fn parse_timeline_text_with_root(
    text: &str,
    base_controls: GardenControls,
    base_root_hz: f32,
    base_voice_count: usize,
    base_octave_min: i32,
    base_octave_max: i32,
    base_event_attack_min: f32,
    base_event_attack_max: f32,
    base_event_decay_min: f32,
    base_event_decay_max: f32,
    base_drone_retune_seconds: f32,
) -> Result<ControlTimeline, String> {
    let mut points = Vec::new();
    let mut current_state = TimelineState::new(
        base_controls,
        base_root_hz,
        base_voice_count,
        base_octave_min,
        base_octave_max,
        base_event_attack_min,
        base_event_attack_max,
        base_event_decay_min,
        base_event_decay_max,
        base_drone_retune_seconds,
    );

    for (line_index, raw_line) in text.lines().enumerate() {
        let line_number = line_index + 1;
        let line = raw_line
            .split_once('#')
            .map_or(raw_line, |(before_comment, _)| before_comment)
            .trim();

        if line.is_empty() {
            continue;
        }

        let mut tokens = line.split_whitespace();
        let time_token = tokens
            .next()
            .ok_or_else(|| format!("line {line_number}: missing time"))?;
        let time_seconds = parse_f32(time_token, line_number, "time")?;

        for token in tokens {
            let (key, value) = token
                .split_once('=')
                .ok_or_else(|| format!("line {line_number}: expected key=value, got {token:?}"))?;
            let value = parse_f32(value, line_number, key)?;

            set_timeline_value(&mut current_state, key, value)
                .map_err(|err| format!("line {line_number}: {err}"))?;
        }

        points.push(ControlPoint::new(time_seconds, current_state));
    }

    Ok(ControlTimeline::new(points))
}

pub(crate) fn set_timeline_value(
    state: &mut TimelineState,
    key: &str,
    value: f32,
) -> Result<(), String> {
    match key {
        "density" => state.controls.density = value.clamp(0.0, 1.0),
        "brightness" => state.controls.brightness = value.clamp(0.0, 1.0),
        "space" => state.controls.space = value.clamp(0.0, 1.0),
        "instability" => state.controls.instability = value.clamp(0.0, 1.0),
        "drone" | "drone_level" => {
            *state.controls.level_mut(InstrumentFamily::Drone) = value.clamp(0.0, 1.0)
        }
        "drone_spread" => state.instrument_params.drone_mut().spread = value,
        "drone_detune" => state.instrument_params.drone_mut().detune = value,
        "drone_active" => state.set_active(InstrumentFamily::Drone, parse_active(value)),
        "drone_level_override" | "drone_override" => {
            state.set_level_override(InstrumentFamily::Drone, Some(value.clamp(0.0, 1.0)))
        }
        "harmonic" | "harmonic_level" => {
            *state.controls.level_mut(InstrumentFamily::Harmonic) = value.clamp(0.0, 1.0)
        }
        "harmonic_mix" => state.instrument_params.harmonic_mut().mix = value,
        "harmonic_shimmer" => state.instrument_params.harmonic_mut().shimmer = value,
        "harmonic_active" => state.set_active(InstrumentFamily::Harmonic, parse_active(value)),
        "harmonic_level_override" | "harmonic_override" => {
            state.set_level_override(InstrumentFamily::Harmonic, Some(value.clamp(0.0, 1.0)))
        }
        "pulse" | "pulse_level" => {
            *state.controls.level_mut(InstrumentFamily::Pulse) = value.clamp(0.0, 1.0)
        }
        "pulse_rate" => state.instrument_params.pulse_mut().rate = value,
        "pulse_length" => state.instrument_params.pulse_mut().length = value,
        "pulse_active" => state.set_active(InstrumentFamily::Pulse, parse_active(value)),
        "pulse_level_override" | "pulse_override" => {
            state.set_level_override(InstrumentFamily::Pulse, Some(value.clamp(0.0, 1.0)))
        }
        "sample" | "sample_level" => {
            *state.controls.level_mut(InstrumentFamily::Sample) = value.clamp(0.0, 1.0)
        }
        "sample_auto_rate" => state.instrument_params.sample_mut().auto_rate = value,
        "sample_active" => state.set_active(InstrumentFamily::Sample, parse_active(value)),
        "sample_level_override" | "sample_override" => {
            state.set_level_override(InstrumentFamily::Sample, Some(value.clamp(0.0, 1.0)))
        }
        "noise" | "noise_level" => {
            *state.controls.level_mut(InstrumentFamily::Noise) = value.clamp(0.0, 1.0)
        }
        "noise_motion" => state.instrument_params.noise_mut().motion = value,
        "noise_active" => state.set_active(InstrumentFamily::Noise, parse_active(value)),
        "noise_level_override" | "noise_override" => {
            state.set_level_override(InstrumentFamily::Noise, Some(value.clamp(0.0, 1.0)))
        }
        "events" | "event_level" => {
            *state.controls.level_mut(InstrumentFamily::Events) = value.clamp(0.0, 1.0)
        }
        "events_active" | "event_active" => {
            state.set_active(InstrumentFamily::Events, parse_active(value))
        }
        "events_level_override" | "event_level_override" | "events_override" | "event_override" => {
            state.set_level_override(InstrumentFamily::Events, Some(value.clamp(0.0, 1.0)))
        }
        "texture" | "texture_level" => {
            *state.controls.level_mut(InstrumentFamily::Texture) = value.clamp(0.0, 1.0)
        }
        "texture_drift" => state.instrument_params.texture_mut().drift = value,
        "root" | "root_hz" => state.root_hz = value.max(1.0),
        "voices" | "voice_count" => state.voice_count = value.round().clamp(1.0, 12.0) as usize,
        "octave_min" | "octaves_min" => state.octave_min = value.round() as i32,
        "octave_max" | "octaves_max" => state.octave_max = value.round() as i32,
        "event_attack_min" => state.event_attack_min = value.max(0.001),
        "event_attack_max" => state.event_attack_max = value.max(0.001),
        "event_decay_min" => state.event_decay_min = value.max(0.05),
        "event_decay_max" => state.event_decay_max = value.max(0.05),
        "drone_retune_seconds" | "retune_seconds" => state.drone_retune_seconds = value.max(0.25),
        _ => return Err(format!("unknown control {key:?}")),
    }

    Ok(())
}

fn parse_f32(value: &str, line_number: usize, label: &str) -> Result<f32, String> {
    value
        .parse::<f32>()
        .map_err(|_| format!("line {line_number}: invalid {label} value {value:?}"))
}

fn interpolate_window(
    start: ControlPoint,
    end: ControlPoint,
    time_seconds: f32,
) -> Option<TimelineState> {
    if time_seconds < start.time_seconds || time_seconds > end.time_seconds {
        return None;
    }

    let span = end.time_seconds - start.time_seconds;
    let amount = if span <= f32::EPSILON {
        1.0
    } else {
        (time_seconds - start.time_seconds) / span
    };

    Some(lerp_state(start.state, end.state, amount).clamped())
}

fn lerp_state(start: TimelineState, end: TimelineState, amount: f32) -> TimelineState {
    TimelineState {
        controls: lerp_controls(start.controls, end.controls, amount),
        instrument_params: lerp_instrument_params(
            start.instrument_params,
            end.instrument_params,
            amount,
        ),
        instrument_states: lerp_instrument_states(
            start.instrument_states,
            end.instrument_states,
            amount,
        ),
        root_hz: lerp(start.root_hz, end.root_hz, amount).max(1.0),
        voice_count: if amount >= 1.0 {
            end.voice_count
        } else {
            start.voice_count
        },
        octave_min: if amount >= 1.0 {
            end.octave_min
        } else {
            start.octave_min
        },
        octave_max: if amount >= 1.0 {
            end.octave_max
        } else {
            start.octave_max
        },
        event_attack_min: if amount >= 1.0 {
            end.event_attack_min
        } else {
            start.event_attack_min
        },
        event_attack_max: if amount >= 1.0 {
            end.event_attack_max
        } else {
            start.event_attack_max
        },
        event_decay_min: if amount >= 1.0 {
            end.event_decay_min
        } else {
            start.event_decay_min
        },
        event_decay_max: if amount >= 1.0 {
            end.event_decay_max
        } else {
            start.event_decay_max
        },
        drone_retune_seconds: if amount >= 1.0 {
            end.drone_retune_seconds
        } else {
            start.drone_retune_seconds
        },
    }
}

fn lerp_instrument_states(
    start: InstrumentStateMap,
    end: InstrumentStateMap,
    amount: f32,
) -> InstrumentStateMap {
    let mut instrument_states = start;
    if amount >= 1.0 {
        instrument_states = end;
    }
    instrument_states
}

fn lerp_controls(start: GardenControls, end: GardenControls, amount: f32) -> GardenControls {
    GardenControls {
        density: lerp(start.density, end.density, amount),
        brightness: lerp(start.brightness, end.brightness, amount),
        space: lerp(start.space, end.space, amount),
        instability: lerp(start.instability, end.instability, amount),
        drone_level: lerp(start.drone_level, end.drone_level, amount),
        harmonic_level: lerp(start.harmonic_level, end.harmonic_level, amount),
        pulse_level: lerp(start.pulse_level, end.pulse_level, amount),
        sample_level: lerp(start.sample_level, end.sample_level, amount),
        noise_level: lerp(start.noise_level, end.noise_level, amount),
        event_level: lerp(start.event_level, end.event_level, amount),
        texture_level: lerp(start.texture_level, end.texture_level, amount),
    }
}

fn lerp_instrument_params(
    start: InstrumentParams,
    end: InstrumentParams,
    amount: f32,
) -> InstrumentParams {
    InstrumentParams {
        values: [
            crate::composition::garden::InstrumentParamValue::Drone(
                crate::composition::garden::DroneParams {
                    spread: lerp(start.drone().spread, end.drone().spread, amount),
                    detune: lerp(start.drone().detune, end.drone().detune, amount),
                },
            ),
            crate::composition::garden::InstrumentParamValue::Harmonic(
                crate::composition::garden::HarmonicParams {
                    mix: lerp(start.harmonic().mix, end.harmonic().mix, amount),
                    shimmer: lerp(start.harmonic().shimmer, end.harmonic().shimmer, amount),
                },
            ),
            crate::composition::garden::InstrumentParamValue::Pulse(
                crate::composition::garden::PulseParams {
                    rate: lerp(start.pulse().rate, end.pulse().rate, amount),
                    length: lerp(start.pulse().length, end.pulse().length, amount),
                },
            ),
            crate::composition::garden::InstrumentParamValue::Sample(
                crate::composition::garden::SampleParams {
                    auto_rate: lerp(start.sample().auto_rate, end.sample().auto_rate, amount),
                },
            ),
            crate::composition::garden::InstrumentParamValue::Noise(
                crate::composition::garden::NoiseParams {
                    motion: lerp(start.noise().motion, end.noise().motion, amount),
                },
            ),
            crate::composition::garden::InstrumentParamValue::Events(start.events()),
            crate::composition::garden::InstrumentParamValue::Texture(
                crate::composition::garden::TextureParams {
                    drift: lerp(start.texture().drift, end.texture().drift, amount),
                },
            ),
        ],
    }
}

fn lerp(start: f32, end: f32, amount: f32) -> f32 {
    start + (end - start) * amount.clamp(0.0, 1.0)
}

fn parse_active(value: f32) -> bool {
    value >= 0.5
}

impl TimelineState {
    pub fn new(
        controls: GardenControls,
        root_hz: f32,
        voice_count: usize,
        octave_min: i32,
        octave_max: i32,
        event_attack_min: f32,
        event_attack_max: f32,
        event_decay_min: f32,
        event_decay_max: f32,
        drone_retune_seconds: f32,
    ) -> Self {
        Self {
            controls: controls.clamped(),
            instrument_params: InstrumentParams::default(),
            instrument_states: InstrumentStateMap::default(),
            root_hz: root_hz.max(1.0),
            voice_count: voice_count.clamp(1, 12),
            octave_min,
            octave_max,
            event_attack_min,
            event_attack_max,
            event_decay_min,
            event_decay_max,
            drone_retune_seconds,
        }
        .clamped()
    }

    pub fn clamped(self) -> Self {
        let octave_min = self.octave_min.clamp(0, 5);
        let octave_max = self.octave_max.clamp(0, 5).max(octave_min);
        let event_attack_min = self.event_attack_min.max(0.001);
        let event_attack_max = self.event_attack_max.max(0.001).max(event_attack_min);
        let event_decay_min = self.event_decay_min.max(0.05);
        let event_decay_max = self.event_decay_max.max(0.05).max(event_decay_min);
        let drone_retune_seconds = self.drone_retune_seconds.clamp(0.25, 60.0);

        Self {
            controls: self.controls.clamped(),
            instrument_params: self.instrument_params.clamped(),
            instrument_states: self.instrument_states.clamped(),
            root_hz: self.root_hz.max(1.0),
            voice_count: self.voice_count.clamp(1, 12),
            octave_min,
            octave_max,
            event_attack_min,
            event_attack_max,
            event_decay_min,
            event_decay_max,
            drone_retune_seconds,
        }
    }

    pub fn register(self) -> RegisterRange {
        RegisterRange::new(self.octave_min, self.octave_max)
    }

    pub fn active(self, family: InstrumentFamily) -> bool {
        self.instrument_states.active(family)
    }

    pub fn active_mut(&mut self, family: InstrumentFamily) -> &mut bool {
        self.instrument_states.active_mut(family)
    }

    pub fn set_active(&mut self, family: InstrumentFamily, active: bool) {
        self.instrument_states.set_active(family, active);
    }

    pub fn level_override(self, family: InstrumentFamily) -> Option<f32> {
        self.instrument_states.level_override(family)
    }

    pub fn level_override_mut(&mut self, family: InstrumentFamily) -> &mut Option<f32> {
        self.instrument_states.level_override_mut(family)
    }

    pub fn set_level_override(&mut self, family: InstrumentFamily, value: Option<f32>) {
        self.instrument_states.set_level_override(family, value);
    }
}

impl InstrumentStateMap {
    pub fn active(self, family: InstrumentFamily) -> bool {
        self.active[family.index()]
    }

    pub fn active_mut(&mut self, family: InstrumentFamily) -> &mut bool {
        &mut self.active[family.index()]
    }

    pub fn set_active(&mut self, family: InstrumentFamily, active: bool) {
        self.active[family.index()] = active;
    }

    pub fn level_override(self, family: InstrumentFamily) -> Option<f32> {
        self.level_overrides[family.index()]
    }

    pub fn level_override_mut(&mut self, family: InstrumentFamily) -> &mut Option<f32> {
        &mut self.level_overrides[family.index()]
    }

    pub fn set_level_override(&mut self, family: InstrumentFamily, value: Option<f32>) {
        self.level_overrides[family.index()] = value;
    }

    pub fn clamped(mut self) -> Self {
        for family in InstrumentFamily::all() {
            let index = family.index();
            self.level_overrides[index] = self.level_overrides[index].map(|value| value.clamp(0.0, 1.0));
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_empty_timeline_as_none() {
        let timeline = ControlTimeline::new(Vec::new());

        assert!(timeline.is_empty());
        assert!(timeline.controls_at(0.0).is_none());
    }

    #[test]
    fn holds_before_first_and_after_last_points() {
        let timeline = ControlTimeline::new(vec![
            ControlPoint::new(
                10.0,
                state(0.2, 0.4, 110.0, 3, 1, 2, 0.02, 0.12, 2.0, 6.0, 12.0),
            ),
            ControlPoint::new(
                20.0,
                state(0.8, 0.9, 220.0, 5, 2, 4, 0.08, 0.30, 4.0, 10.0, 3.0),
            ),
        ]);

        assert_close(timeline.controls_at(0.0).unwrap().density, 0.2);
        assert_close(timeline.state_at(30.0).unwrap().root_hz, 220.0);
        assert_eq!(timeline.state_at(30.0).unwrap().voice_count, 5);
        assert_eq!(timeline.state_at(30.0).unwrap().octave_min, 2);
        assert_eq!(timeline.state_at(30.0).unwrap().octave_max, 4);
        assert_close(timeline.state_at(30.0).unwrap().event_attack_min, 0.08);
        assert_close(timeline.state_at(30.0).unwrap().event_attack_max, 0.30);
        assert_close(timeline.state_at(30.0).unwrap().event_decay_min, 4.0);
        assert_close(timeline.state_at(30.0).unwrap().event_decay_max, 10.0);
        assert_close(timeline.state_at(30.0).unwrap().drone_retune_seconds, 3.0);
    }

    #[test]
    fn sorts_points_and_interpolates_between_neighbors() {
        let timeline = ControlTimeline::new(vec![
            ControlPoint::new(
                20.0,
                state(0.8, 0.9, 220.0, 5, 2, 4, 0.08, 0.30, 4.0, 10.0, 3.0),
            ),
            ControlPoint::new(
                10.0,
                state(0.2, 0.4, 110.0, 3, 1, 2, 0.02, 0.12, 2.0, 6.0, 12.0),
            ),
        ]);

        let midpoint = timeline.state_at(15.0).unwrap();

        assert_close(midpoint.controls.density, 0.5);
        assert_close(midpoint.controls.texture_level, 0.65);
        assert_close(midpoint.root_hz, 165.0);
        assert_eq!(midpoint.voice_count, 3);
        assert_eq!(midpoint.octave_min, 1);
        assert_eq!(midpoint.octave_max, 2);
        assert_close(midpoint.event_attack_min, 0.02);
        assert_close(midpoint.event_attack_max, 0.12);
        assert_close(midpoint.event_decay_min, 2.0);
        assert_close(midpoint.event_decay_max, 6.0);
        assert_close(midpoint.drone_retune_seconds, 12.0);
        assert_eq!(timeline.state_at(20.0).unwrap().voice_count, 5);
    }

    #[test]
    fn clamps_control_values_at_points_and_after_interpolation() {
        let timeline = ControlTimeline::new(vec![
            ControlPoint::new(
                0.0,
                state(-1.0, 0.0, -10.0, 0, -2, 8, -2.0, 0.0, -1.0, 0.0, -2.0),
            ),
            ControlPoint::new(
                10.0,
                state(2.0, 2.0, 440.0, 30, 3, 1, 1.2, 0.4, 12.0, 6.0, 90.0),
            ),
        ]);

        let midpoint = timeline.state_at(5.0).unwrap();

        assert_close(midpoint.controls.density, 0.5);
        assert_close(midpoint.controls.texture_level, 0.5);
        assert_close(midpoint.root_hz, 220.5);
        assert_eq!(midpoint.voice_count, 1);
        assert_eq!(midpoint.octave_min, 0);
        assert_eq!(midpoint.octave_max, 5);
        assert_close(midpoint.event_attack_min, 0.001);
        assert_close(midpoint.event_attack_max, 0.001);
        assert_close(midpoint.event_decay_min, 0.05);
        assert_close(midpoint.event_decay_max, 0.05);
        assert_close(midpoint.drone_retune_seconds, 0.25);
        assert_eq!(timeline.state_at(10.0).unwrap().voice_count, 12);
        assert_eq!(timeline.state_at(10.0).unwrap().octave_min, 3);
        assert_eq!(timeline.state_at(10.0).unwrap().octave_max, 3);
        assert_close(timeline.state_at(10.0).unwrap().event_attack_min, 1.2);
        assert_close(timeline.state_at(10.0).unwrap().event_attack_max, 1.2);
        assert_close(timeline.state_at(10.0).unwrap().event_decay_min, 12.0);
        assert_close(timeline.state_at(10.0).unwrap().event_decay_max, 12.0);
        assert_close(timeline.state_at(10.0).unwrap().drone_retune_seconds, 60.0);
    }

    #[test]
    fn parses_timeline_text_with_comments_and_carried_controls() {
        let timeline = parse_timeline_text_with_root(
            "
            # explicit automation
            0 density=0.2 noise=0.1 root=82.41 voices=5 octave_min=2 octave_max=3 event_attack_min=0.03 event_attack_max=0.18 event_decay_min=3 event_decay_max=7 drone_retune_seconds=6
            10 texture=0.8 events=0.5 # later point
            ",
            GardenControls::default(),
            110.0,
            3,
            1,
            2,
            0.015,
            0.195,
            2.0,
            8.0,
            9.0,
        )
        .unwrap();

        let start = timeline.state_at(0.0).unwrap();
        let end = timeline.state_at(10.0).unwrap();

        assert_close(start.controls.density, 0.2);
        assert_close(start.controls.noise_level, 0.1);
        assert_close(start.controls.texture_level, 0.0);
        assert_close(start.root_hz, 82.41);
        assert_eq!(start.voice_count, 5);
        assert_eq!(start.octave_min, 2);
        assert_eq!(start.octave_max, 3);
        assert_close(start.event_attack_min, 0.03);
        assert_close(start.event_attack_max, 0.18);
        assert_close(start.event_decay_min, 3.0);
        assert_close(start.event_decay_max, 7.0);
        assert_close(start.drone_retune_seconds, 6.0);
        assert_close(end.controls.density, 0.2);
        assert_close(end.controls.noise_level, 0.1);
        assert_close(end.controls.texture_level, 0.8);
        assert_close(end.controls.event_level, 0.5);
        assert_close(end.root_hz, 82.41);
        assert_eq!(end.voice_count, 5);
        assert_eq!(end.octave_min, 2);
        assert_eq!(end.octave_max, 3);
        assert_close(end.event_attack_min, 0.03);
        assert_close(end.event_attack_max, 0.18);
        assert_close(end.event_decay_min, 3.0);
        assert_close(end.event_decay_max, 7.0);
        assert_close(end.drone_retune_seconds, 6.0);
    }

    #[test]
    fn parses_aliases_and_clamps_values() {
        let timeline = parse_timeline_text_with_root(
            "0 drone=2 noise_level=-1 event_level=0.4 texture_level=0.7 root_hz=55 voice_count=8 octave_min=-2 octave_max=9 event_attack_min=-4 event_attack_max=20 event_decay_min=-4 event_decay_max=20 drone_retune_seconds=0.1",
            GardenControls::default(),
            110.0,
            3,
            1,
            2,
            0.015,
            0.195,
            2.0,
            8.0,
            9.0,
        )
        .unwrap();

        let state = timeline.state_at(0.0).unwrap();

        assert_close(state.controls.drone_level, 1.0);
        assert_close(state.controls.noise_level, 0.0);
        assert_close(state.controls.event_level, 0.4);
        assert_close(state.controls.texture_level, 0.7);
        assert_close(state.root_hz, 55.0);
        assert_eq!(state.voice_count, 8);
        assert_eq!(state.octave_min, 0);
        assert_eq!(state.octave_max, 5);
        assert_close(state.event_attack_min, 0.001);
        assert_close(state.event_attack_max, 20.0);
        assert_close(state.event_decay_min, 0.05);
        assert_close(state.event_decay_max, 20.0);
        assert_close(state.drone_retune_seconds, 0.25);
    }

    #[test]
    fn parses_instrument_active_flags_and_carries_them_forward() {
        let timeline = parse_timeline_text_with_root(
            "0 drone_active=1 harmonic_active=0 pulse_active=1 sample_active=0 noise_active=1 events_active=0
             10 harmonic_active=1 sample_active=1",
            GardenControls::default(),
            110.0,
            3,
            1,
            2,
            0.015,
            0.195,
            2.0,
            8.0,
            9.0,
        )
        .unwrap();

        let start = timeline.state_at(0.0).unwrap();
        let end = timeline.state_at(10.0).unwrap();

        assert!(start.active(InstrumentFamily::Drone));
        assert!(!start.active(InstrumentFamily::Harmonic));
        assert!(start.active(InstrumentFamily::Pulse));
        assert!(!start.active(InstrumentFamily::Sample));
        assert!(start.active(InstrumentFamily::Noise));
        assert!(!start.active(InstrumentFamily::Events));

        assert!(end.active(InstrumentFamily::Drone));
        assert!(end.active(InstrumentFamily::Harmonic));
        assert!(end.active(InstrumentFamily::Pulse));
        assert!(end.active(InstrumentFamily::Sample));
        assert!(end.active(InstrumentFamily::Noise));
        assert!(!end.active(InstrumentFamily::Events));
    }

    #[test]
    fn steps_instrument_active_flags_at_control_points() {
        let timeline = ControlTimeline::new(vec![
            ControlPoint::new(
                0.0,
                inactive_state(InstrumentFamily::Harmonic, state(0.2, 0.4, 110.0, 3, 1, 2, 0.02, 0.12, 2.0, 6.0, 12.0)),
            ),
            ControlPoint::new(
                10.0,
                active_state(InstrumentFamily::Harmonic, state(0.8, 0.9, 220.0, 5, 2, 4, 0.08, 0.30, 4.0, 10.0, 3.0)),
            ),
        ]);

        assert!(!timeline.state_at(5.0).unwrap().active(InstrumentFamily::Harmonic));
        assert!(timeline.state_at(10.0).unwrap().active(InstrumentFamily::Harmonic));
    }

    #[test]
    fn parses_instrument_level_overrides_and_carries_them_forward() {
        let timeline = parse_timeline_text_with_root(
            "0 drone_level_override=0.2 harmonic_level_override=0.4 pulse_level_override=0.6 sample_level_override=0.8 noise_level_override=0.1 events_level_override=0.3
             10 harmonic_level_override=0.7 sample_level_override=0.5",
            GardenControls::default(),
            110.0,
            3,
            1,
            2,
            0.015,
            0.195,
            2.0,
            8.0,
            9.0,
        )
        .unwrap();

        let start = timeline.state_at(0.0).unwrap();
        let end = timeline.state_at(10.0).unwrap();

        assert_close(start.level_override(InstrumentFamily::Drone).unwrap(), 0.2);
        assert_close(start.level_override(InstrumentFamily::Harmonic).unwrap(), 0.4);
        assert_close(start.level_override(InstrumentFamily::Pulse).unwrap(), 0.6);
        assert_close(start.level_override(InstrumentFamily::Sample).unwrap(), 0.8);
        assert_close(start.level_override(InstrumentFamily::Noise).unwrap(), 0.1);
        assert_close(start.level_override(InstrumentFamily::Events).unwrap(), 0.3);

        assert_close(end.level_override(InstrumentFamily::Drone).unwrap(), 0.2);
        assert_close(end.level_override(InstrumentFamily::Harmonic).unwrap(), 0.7);
        assert_close(end.level_override(InstrumentFamily::Pulse).unwrap(), 0.6);
        assert_close(end.level_override(InstrumentFamily::Sample).unwrap(), 0.5);
        assert_close(end.level_override(InstrumentFamily::Noise).unwrap(), 0.1);
        assert_close(end.level_override(InstrumentFamily::Events).unwrap(), 0.3);
    }

    #[test]
    fn parses_instrument_parameter_controls() {
        let timeline = parse_timeline_text_with_root(
            "0 drone_spread=1.4 drone_detune=1.2 harmonic_mix=0.8 harmonic_shimmer=1.3 pulse_rate=1.5 pulse_length=0.7 noise_motion=0.6 sample_auto_rate=1.8 texture_drift=1.4
             10 pulse_rate=0.5 texture_drift=0.2",
            GardenControls::default(),
            110.0,
            3,
            1,
            2,
            0.015,
            0.195,
            2.0,
            8.0,
            9.0,
        )
        .unwrap();

        let start = timeline.state_at(0.0).unwrap();
        let end = timeline.state_at(10.0).unwrap();

        assert_close(start.instrument_params.drone().spread, 1.4);
        assert_close(start.instrument_params.drone().detune, 1.2);
        assert_close(start.instrument_params.harmonic().mix, 0.8);
        assert_close(start.instrument_params.harmonic().shimmer, 1.3);
        assert_close(start.instrument_params.pulse().rate, 1.5);
        assert_close(start.instrument_params.pulse().length, 0.7);
        assert_close(start.instrument_params.noise().motion, 0.6);
        assert_close(start.instrument_params.sample().auto_rate, 1.8);
        assert_close(start.instrument_params.texture().drift, 1.4);
        assert_close(end.instrument_params.pulse().rate, 0.5);
        assert_close(end.instrument_params.texture().drift, 0.2);
    }

    #[test]
    fn steps_instrument_level_overrides_at_control_points() {
        let timeline = ControlTimeline::new(vec![
            ControlPoint::new(
                0.0,
                with_override(InstrumentFamily::Pulse, 0.2, state(0.2, 0.4, 110.0, 3, 1, 2, 0.02, 0.12, 2.0, 6.0, 12.0)),
            ),
            ControlPoint::new(
                10.0,
                with_override(InstrumentFamily::Pulse, 0.8, state(0.8, 0.9, 220.0, 5, 2, 4, 0.08, 0.30, 4.0, 10.0, 3.0)),
            ),
        ]);

        assert_close(
            timeline
                .state_at(5.0)
                .unwrap()
                .level_override(InstrumentFamily::Pulse)
                .unwrap(),
            0.2,
        );
        assert_close(
            timeline
                .state_at(10.0)
                .unwrap()
                .level_override(InstrumentFamily::Pulse)
                .unwrap(),
            0.8,
        );
    }

    #[test]
    fn rejects_unknown_controls() {
        let err = parse_timeline_text_with_root(
            "0 fog=0.5",
            GardenControls::default(),
            110.0,
            3,
            1,
            2,
            0.015,
            0.195,
            2.0,
            8.0,
            9.0,
        )
        .unwrap_err();

        assert!(err.contains("line 1"));
        assert!(err.contains("unknown control"));
    }

    #[test]
    fn rejects_malformed_tokens() {
        let err = parse_timeline_text_with_root(
            "0 density",
            GardenControls::default(),
            110.0,
            3,
            1,
            2,
            0.015,
            0.195,
            2.0,
            8.0,
            9.0,
        )
        .unwrap_err();

        assert!(err.contains("line 1"));
        assert!(err.contains("key=value"));
    }

    #[test]
    fn parse_timeline_text_uses_legacy_default_root() {
        let timeline = parse_timeline_text("0 density=0.4", GardenControls::default()).unwrap();

        assert_close(timeline.state_at(0.0).unwrap().root_hz, 110.0);
        assert_eq!(timeline.state_at(0.0).unwrap().voice_count, 3);
        assert_eq!(timeline.state_at(0.0).unwrap().octave_min, 1);
        assert_eq!(timeline.state_at(0.0).unwrap().octave_max, 2);
        assert_close(timeline.state_at(0.0).unwrap().event_attack_min, 0.015);
        assert_close(timeline.state_at(0.0).unwrap().event_attack_max, 0.195);
        assert_close(timeline.state_at(0.0).unwrap().event_decay_min, 2.0);
        assert_close(timeline.state_at(0.0).unwrap().event_decay_max, 8.0);
        assert_close(timeline.state_at(0.0).unwrap().drone_retune_seconds, 9.0);
    }

    fn state(
        density: f32,
        texture_level: f32,
        root_hz: f32,
        voice_count: usize,
        octave_min: i32,
        octave_max: i32,
        event_attack_min: f32,
        event_attack_max: f32,
        event_decay_min: f32,
        event_decay_max: f32,
        drone_retune_seconds: f32,
    ) -> TimelineState {
        TimelineState::new(
            GardenControls {
                density,
                texture_level,
                ..GardenControls::default()
            },
            root_hz,
            voice_count,
            octave_min,
            octave_max,
            event_attack_min,
            event_attack_max,
            event_decay_min,
            event_decay_max,
            drone_retune_seconds,
        )
    }

    fn inactive_state(family: InstrumentFamily, mut state: TimelineState) -> TimelineState {
        state.set_active(family, false);
        state
    }

    fn active_state(family: InstrumentFamily, mut state: TimelineState) -> TimelineState {
        state.set_active(family, true);
        state
    }

    fn with_override(
        family: InstrumentFamily,
        value: f32,
        mut state: TimelineState,
    ) -> TimelineState {
        state.set_level_override(family, Some(value));
        state
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.0001,
            "expected {expected}, got {actual}"
        );
    }
}
