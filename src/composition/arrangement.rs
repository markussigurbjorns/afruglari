use crate::composition::garden::GardenControls;
use crate::composition::timeline::{
    ControlPoint, ControlTimeline, TimelineState, set_timeline_value,
};
use crate::instruments::InstrumentFamily;

#[derive(Clone, Debug)]
pub struct Arrangement {
    sample_assets: Vec<SampleAssetSpec>,
    instrument_specs: Vec<InstrumentInstanceSpec>,
    sections: Vec<ArrangementSection>,
    timeline: ControlTimeline,
    sample_triggers: Vec<SampleTriggerEvent>,
    duration_seconds: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct ArrangementDefaults {
    pub controls: GardenControls,
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

#[derive(Clone, Debug)]
pub struct ArrangementSection {
    pub name: String,
    pub start_seconds: f32,
    pub duration_seconds: f32,
    pub mode: SectionMode,
    pub entry_state: TimelineState,
    pub state: TimelineState,
    pub instrument_entries: Vec<ArrangementInstrumentEntry>,
    pub sample_triggers: Vec<SampleTriggerEvent>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SectionMode {
    Hold,
    Ramp,
}

#[derive(Clone, Debug)]
pub struct ArrangementInstrumentEntry {
    pub target_id: Option<String>,
    pub family: InstrumentFamily,
    pub level: Option<f32>,
    pub active: Option<bool>,
    pub level_override: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstrumentInstanceSpec {
    pub id: String,
    pub family: InstrumentFamily,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleAssetSpec {
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SampleTriggerEvent {
    pub time_seconds: f32,
    pub sample_name: String,
    pub start_seconds: Option<f32>,
    pub end_seconds: Option<f32>,
    pub fade_in_seconds: Option<f32>,
    pub fade_out_seconds: Option<f32>,
    pub semitones: Option<f32>,
    pub cents: Option<f32>,
    pub gain: Option<f32>,
    pub pan: Option<f32>,
    pub rate: Option<f32>,
}

#[derive(Clone, Debug)]
struct SectionBuilder {
    name: String,
    start_seconds: f32,
    duration_seconds: f32,
    mode: SectionMode,
    entry_state: TimelineState,
    state: TimelineState,
    instrument_entries: Vec<ArrangementInstrumentEntry>,
    sample_triggers: Vec<SampleTriggerEvent>,
}

impl Arrangement {
    pub fn new(
        sample_assets: Vec<SampleAssetSpec>,
        instrument_specs: Vec<InstrumentInstanceSpec>,
        sections: Vec<ArrangementSection>,
    ) -> Self {
        let duration_seconds = sections.last().map_or(0.0, |section| {
            section.start_seconds + section.duration_seconds
        });
        let points = compile_timeline_points(&sections);
        let mut sample_triggers = sections
            .iter()
            .flat_map(|section| section.sample_triggers.iter().cloned())
            .collect::<Vec<_>>();
        sample_triggers.sort_by(|a, b| a.time_seconds.total_cmp(&b.time_seconds));

        Self {
            sample_assets,
            instrument_specs: if instrument_specs.is_empty() {
                default_instrument_specs()
            } else {
                instrument_specs
            },
            sections,
            timeline: ControlTimeline::new(points),
            sample_triggers,
            duration_seconds,
        }
    }

    pub fn sections(&self) -> &[ArrangementSection] {
        &self.sections
    }

    pub fn sections_mut(&mut self) -> &mut [ArrangementSection] {
        &mut self.sections
    }

    pub fn push_section(&mut self, section: ArrangementSection) {
        self.sections.push(section);
    }

    pub fn remove_section(&mut self, index: usize) -> ArrangementSection {
        self.sections.remove(index)
    }

    pub fn sample_assets(&self) -> &[SampleAssetSpec] {
        &self.sample_assets
    }

    pub fn instrument_specs(&self) -> &[InstrumentInstanceSpec] {
        &self.instrument_specs
    }

    pub fn instrument_specs_mut(&mut self) -> &mut Vec<InstrumentInstanceSpec> {
        &mut self.instrument_specs
    }

    pub fn sample_assets_mut(&mut self) -> &mut Vec<SampleAssetSpec> {
        &mut self.sample_assets
    }

    pub fn timeline(&self) -> &ControlTimeline {
        &self.timeline
    }

    pub fn duration_seconds(&self) -> f32 {
        self.duration_seconds
    }

    pub fn sample_triggers(&self) -> &[SampleTriggerEvent] {
        &self.sample_triggers
    }

    pub fn refresh_derived(&mut self) {
        let mut start_seconds: f32 = 0.0;
        let mut previous_state = None;

        for section in &mut self.sections {
            let local_trigger_times = section
                .sample_triggers
                .iter()
                .map(|trigger| (trigger.time_seconds - section.start_seconds).max(0.0))
                .collect::<Vec<_>>();

            section.start_seconds = start_seconds.max(0.0);
            section.duration_seconds = section.duration_seconds.max(0.0);
            section.entry_state = previous_state.unwrap_or(section.entry_state).clamped();
            section.state = section.state.clamped();
            section.instrument_entries = canonical_instrument_entries(
                section.state,
                &self.instrument_specs,
                &section.instrument_entries,
            );

            for (trigger, local_time) in section.sample_triggers.iter_mut().zip(local_trigger_times)
            {
                trigger.time_seconds =
                    section.start_seconds + local_time.clamp(0.0, section.duration_seconds);
            }

            start_seconds = section.start_seconds + section.duration_seconds;
            previous_state = Some(section.state);
        }

        self.duration_seconds = self.sections.last().map_or(0.0, |section| {
            section.start_seconds + section.duration_seconds
        });
        self.timeline = ControlTimeline::new(compile_timeline_points(&self.sections));
        self.sample_triggers = self
            .sections
            .iter()
            .flat_map(|section| section.sample_triggers.iter().cloned())
            .collect::<Vec<_>>();
        self.sample_triggers
            .sort_by(|a, b| a.time_seconds.total_cmp(&b.time_seconds));
    }

    pub fn to_text(&self) -> String {
        let mut output = String::new();

        for asset in &self.sample_assets {
            output.push_str(&format!("sample {} file={}\n", asset.name, asset.path));
        }
        if !self.sample_assets.is_empty() && (!self.instrument_specs.is_empty() || !self.sections.is_empty()) {
            output.push('\n');
        }

        for instrument in &self.instrument_specs {
            output.push_str(&format!(
                "instance {} family={}\n",
                instrument.id,
                format_instrument_family(instrument.family)
            ));
        }
        if !self.instrument_specs.is_empty() && !self.sections.is_empty() {
            output.push('\n');
        }

        for (index, section) in self.sections.iter().enumerate() {
            output.push_str(&format!(
                "section {} duration={} mode={} root={} voices={} octave_min={} octave_max={} event_attack_min={} event_attack_max={} event_decay_min={} event_decay_max={} drone_retune_seconds={} density={} brightness={} space={} instability={} drone_spread={} drone_detune={} harmonic_mix={} harmonic_shimmer={} pulse_rate={} pulse_length={} noise_motion={} sample_auto_rate={} texture_drift={}\n",
                section.name,
                format_f32(section.duration_seconds),
                format_mode(section.mode),
                format_f32(section.state.root_hz),
                section.state.voice_count,
                section.state.octave_min,
                section.state.octave_max,
                format_f32(section.state.event_attack_min),
                format_f32(section.state.event_attack_max),
                format_f32(section.state.event_decay_min),
                format_f32(section.state.event_decay_max),
                format_f32(section.state.drone_retune_seconds),
                format_f32(section.state.controls.density),
                format_f32(section.state.controls.brightness),
                format_f32(section.state.controls.space),
                format_f32(section.state.controls.instability),
                format_f32(section.state.instrument_params.drone().spread),
                format_f32(section.state.instrument_params.drone().detune),
                format_f32(section.state.instrument_params.harmonic().mix),
                format_f32(section.state.instrument_params.harmonic().shimmer),
                format_f32(section.state.instrument_params.pulse().rate),
                format_f32(section.state.instrument_params.pulse().length),
                format_f32(section.state.instrument_params.noise().motion),
                format_f32(section.state.instrument_params.sample().auto_rate),
                format_f32(section.state.instrument_params.texture().drift),
            ));

            write_instrument_line(
                &mut output,
                section
                    .instrument_entries
                    .iter()
                    .find(|entry| entry.family == InstrumentFamily::Drone)
                    .and_then(|entry| entry.target_id.as_deref())
                    .unwrap_or("drone"),
                section.state.controls.level(InstrumentFamily::Drone),
                Some(section.state.active(InstrumentFamily::Drone)),
                section.state.level_override(InstrumentFamily::Drone),
            );
            write_instrument_line(
                &mut output,
                section
                    .instrument_entries
                    .iter()
                    .find(|entry| entry.family == InstrumentFamily::Harmonic)
                    .and_then(|entry| entry.target_id.as_deref())
                    .unwrap_or("harmonic"),
                section.state.controls.level(InstrumentFamily::Harmonic),
                Some(section.state.active(InstrumentFamily::Harmonic)),
                section.state.level_override(InstrumentFamily::Harmonic),
            );
            write_instrument_line(
                &mut output,
                section
                    .instrument_entries
                    .iter()
                    .find(|entry| entry.family == InstrumentFamily::Pulse)
                    .and_then(|entry| entry.target_id.as_deref())
                    .unwrap_or("pulse"),
                section.state.controls.level(InstrumentFamily::Pulse),
                Some(section.state.active(InstrumentFamily::Pulse)),
                section.state.level_override(InstrumentFamily::Pulse),
            );
            write_instrument_line(
                &mut output,
                section
                    .instrument_entries
                    .iter()
                    .find(|entry| entry.family == InstrumentFamily::Sample)
                    .and_then(|entry| entry.target_id.as_deref())
                    .unwrap_or("sample"),
                section.state.controls.level(InstrumentFamily::Sample),
                Some(section.state.active(InstrumentFamily::Sample)),
                section.state.level_override(InstrumentFamily::Sample),
            );
            write_instrument_line(
                &mut output,
                section
                    .instrument_entries
                    .iter()
                    .find(|entry| entry.family == InstrumentFamily::Noise)
                    .and_then(|entry| entry.target_id.as_deref())
                    .unwrap_or("noise"),
                section.state.controls.level(InstrumentFamily::Noise),
                Some(section.state.active(InstrumentFamily::Noise)),
                section.state.level_override(InstrumentFamily::Noise),
            );
            write_instrument_line(
                &mut output,
                section
                    .instrument_entries
                    .iter()
                    .find(|entry| entry.family == InstrumentFamily::Events)
                    .and_then(|entry| entry.target_id.as_deref())
                    .unwrap_or("events"),
                section.state.controls.level(InstrumentFamily::Events),
                Some(section.state.active(InstrumentFamily::Events)),
                section.state.level_override(InstrumentFamily::Events),
            );
            write_instrument_line(
                &mut output,
                section
                    .instrument_entries
                    .iter()
                    .find(|entry| entry.family == InstrumentFamily::Texture)
                    .and_then(|entry| entry.target_id.as_deref())
                    .unwrap_or("texture"),
                section.state.controls.level(InstrumentFamily::Texture),
                None,
                None,
            );

            for trigger in &section.sample_triggers {
                let local_time = (trigger.time_seconds - section.start_seconds).max(0.0);
                output.push_str(&format!(
                    "trigger sample name={} at={}",
                    trigger.sample_name,
                    format_f32(local_time),
                ));
                if let Some(start) = trigger.start_seconds {
                    output.push_str(&format!(" start={}", format_f32(start)));
                }
                if let Some(end) = trigger.end_seconds {
                    output.push_str(&format!(" end={}", format_f32(end)));
                }
                if let Some(fade_in) = trigger.fade_in_seconds {
                    output.push_str(&format!(" fade_in={}", format_f32(fade_in)));
                }
                if let Some(fade_out) = trigger.fade_out_seconds {
                    output.push_str(&format!(" fade_out={}", format_f32(fade_out)));
                }
                if let Some(semitones) = trigger.semitones {
                    output.push_str(&format!(" semitones={}", format_f32(semitones)));
                }
                if let Some(cents) = trigger.cents {
                    output.push_str(&format!(" cents={}", format_f32(cents)));
                }
                if let Some(gain) = trigger.gain {
                    output.push_str(&format!(" gain={}", format_f32(gain)));
                }
                if let Some(pan) = trigger.pan {
                    output.push_str(&format!(" pan={}", format_f32(pan)));
                }
                if let Some(rate) = trigger.rate {
                    output.push_str(&format!(" rate={}", format_f32(rate)));
                }
                output.push('\n');
            }

            if index + 1 < self.sections.len() {
                output.push('\n');
            }
        }

        output
    }
}

impl ArrangementDefaults {
    pub fn state(self) -> TimelineState {
        TimelineState::new(
            self.controls,
            self.root_hz,
            self.voice_count,
            self.octave_min,
            self.octave_max,
            self.event_attack_min,
            self.event_attack_max,
            self.event_decay_min,
            self.event_decay_max,
            self.drone_retune_seconds,
        )
    }
}

impl ArrangementInstrumentEntry {
    fn new(target_id: Option<String>, family: InstrumentFamily) -> Self {
        Self {
            target_id,
            family,
            level: None,
            active: None,
            level_override: None,
        }
    }
}

impl SectionBuilder {
    fn finalize(self) -> ArrangementSection {
        ArrangementSection {
            name: self.name,
            start_seconds: self.start_seconds,
            duration_seconds: self.duration_seconds,
            mode: self.mode,
            entry_state: self.entry_state.clamped(),
            state: self.state.clamped(),
            instrument_entries: self.instrument_entries,
            sample_triggers: self.sample_triggers,
        }
    }
}

pub fn parse_arrangement_text(
    text: &str,
    defaults: ArrangementDefaults,
) -> Result<Arrangement, String> {
    let mut sample_assets = Vec::new();
    let mut instrument_specs = Vec::new();
    let mut sections = Vec::new();
    let mut current_state = defaults.state();
    let mut start_seconds = 0.0;
    let mut current_section: Option<SectionBuilder> = None;

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
        let keyword = tokens
            .next()
            .ok_or_else(|| format!("line {line_number}: missing keyword"))?;

        match keyword {
            "sample" => {
                if current_section.is_some() {
                    return Err(format!(
                        "line {line_number}: sample assets must be declared before sections"
                    ));
                }
                sample_assets.push(parse_sample_asset_line(tokens, line_number)?);
            }
            "instance" => {
                if current_section.is_some() {
                    return Err(format!(
                        "line {line_number}: instrument instances must be declared before sections"
                    ));
                }
                instrument_specs.push(parse_instrument_instance_line(tokens, line_number)?);
            }
            "section" => {
                if let Some(section) = current_section.take() {
                    let section = section.finalize();
                    start_seconds = section.start_seconds + section.duration_seconds;
                    current_state = section.state;
                    sections.push(section);
                }

                current_section = Some(parse_section_header(
                    tokens,
                    line_number,
                    start_seconds,
                    current_state,
                )?);
            }
            "instrument" => {
                let section = current_section
                    .as_mut()
                    .ok_or_else(|| format!("line {line_number}: instrument outside section"))?;
                parse_instrument_line(section, &instrument_specs, tokens, line_number)?;
            }
            "trigger" => {
                let section = current_section
                    .as_mut()
                    .ok_or_else(|| format!("line {line_number}: trigger outside section"))?;
                parse_trigger_line(section, tokens, line_number)?;
            }
            _ => {
                return Err(format!(
                    "line {line_number}: expected `sample`, `instance`, `section`, `instrument`, or `trigger`, got {keyword:?}"
                ));
            }
        }
    }

    if let Some(section) = current_section.take() {
        sections.push(section.finalize());
    }

    Ok(Arrangement::new(sample_assets, instrument_specs, sections))
}

fn parse_section_header(
    mut tokens: std::str::SplitWhitespace<'_>,
    line_number: usize,
    start_seconds: f32,
    current_state: TimelineState,
) -> Result<SectionBuilder, String> {
    let name = tokens
        .next()
        .ok_or_else(|| format!("line {line_number}: missing section name"))?
        .to_string();

    let entry_state = current_state.clamped();
    let mut duration_seconds = None;
    let mut mode = SectionMode::Hold;
    let mut state = current_state;

    for token in tokens {
        let (key, value) = token
            .split_once('=')
            .ok_or_else(|| format!("line {line_number}: expected key=value, got {token:?}"))?;

        if key == "duration" || key == "dur" {
            let value = parse_f32(value, line_number, key)?;
            duration_seconds = Some(value.max(0.0));
        } else if key == "mode" {
            mode = parse_section_mode(value, line_number)?;
        } else {
            let value = parse_f32(value, line_number, key)?;
            set_timeline_value(&mut state, key, value)
                .map_err(|err| format!("line {line_number}: {err}"))?;
        }
    }

    let duration_seconds =
        duration_seconds.ok_or_else(|| format!("line {line_number}: missing duration"))?;

    Ok(SectionBuilder {
        name,
        start_seconds,
        duration_seconds,
        mode,
        entry_state,
        state: state.clamped(),
        instrument_entries: Vec::new(),
        sample_triggers: Vec::new(),
    })
}

fn parse_instrument_line(
    section: &mut SectionBuilder,
    instrument_specs: &[InstrumentInstanceSpec],
    mut tokens: std::str::SplitWhitespace<'_>,
    line_number: usize,
) -> Result<(), String> {
    let target_token = tokens
        .next()
        .ok_or_else(|| format!("line {line_number}: missing instrument target"))?;
    let (target_id, family) = resolve_instrument_target(target_token, instrument_specs, line_number)?;
    let mut entry = ArrangementInstrumentEntry::new(target_id, family);

    for token in tokens {
        let (key, value) = token
            .split_once('=')
            .ok_or_else(|| format!("line {line_number}: expected key=value, got {token:?}"))?;

        match key {
            "level" => {
                let value = parse_f32(value, line_number, key)?.clamp(0.0, 1.0);
                set_instrument_level(&mut section.state, family, value);
                entry.level = Some(value);
            }
            "active" | "enabled" => {
                let value = parse_active(value, line_number)?;
                set_instrument_active(&mut section.state, family, value, line_number)?;
                entry.active = Some(value);
            }
            "override" | "level_override" => {
                let value = parse_f32(value, line_number, key)?.clamp(0.0, 1.0);
                set_instrument_override(&mut section.state, family, value, line_number)?;
                entry.level_override = Some(value);
            }
            _ => {
                return Err(format!(
                    "line {line_number}: unknown instrument control {key:?}"
                ));
            }
        }
    }

    section.instrument_entries.push(entry);
    section.state = section.state.clamped();
    Ok(())
}

fn parse_trigger_line(
    section: &mut SectionBuilder,
    mut tokens: std::str::SplitWhitespace<'_>,
    line_number: usize,
) -> Result<(), String> {
    let family_token = tokens
        .next()
        .ok_or_else(|| format!("line {line_number}: missing trigger family"))?;
    let family = parse_instrument_family(family_token, line_number)?;
    if family != InstrumentFamily::Sample {
        return Err(format!(
            "line {line_number}: only sample triggers are supported right now"
        ));
    }

    let mut at_seconds = None;
    let mut sample_name = String::from("default");
    let mut start_seconds = None;
    let mut end_seconds = None;
    let mut fade_in_seconds = None;
    let mut fade_out_seconds = None;
    let mut semitones = None;
    let mut cents = None;
    let mut gain = None;
    let mut pan = None;
    let mut rate = None;

    for token in tokens {
        let (key, value) = token
            .split_once('=')
            .ok_or_else(|| format!("line {line_number}: expected key=value, got {token:?}"))?;
        match key {
            "name" | "sample" | "asset" => {
                sample_name = value.to_string();
            }
            "start" | "offset" => {
                start_seconds = Some(parse_f32(value, line_number, key)?.max(0.0));
            }
            "end" => {
                end_seconds = Some(parse_f32(value, line_number, key)?.max(0.0));
            }
            "fade_in" => {
                fade_in_seconds = Some(parse_f32(value, line_number, key)?.max(0.0));
            }
            "fade_out" => {
                fade_out_seconds = Some(parse_f32(value, line_number, key)?.max(0.0));
            }
            "semitones" | "transpose" => {
                semitones = Some(parse_f32(value, line_number, key)?);
            }
            "cents" => {
                cents = Some(parse_f32(value, line_number, key)?);
            }
            "at" | "time" => {
                at_seconds = Some(parse_f32(value, line_number, key)?);
            }
            "gain" => {
                gain = Some(parse_f32(value, line_number, key)?.clamp(0.0, 1.0));
            }
            "pan" => {
                pan = Some(parse_f32(value, line_number, key)?.clamp(-1.0, 1.0));
            }
            "rate" => {
                rate = Some(parse_f32(value, line_number, key)?.max(0.05));
            }
            _ => {
                return Err(format!(
                    "line {line_number}: unknown trigger control {key:?}"
                ));
            }
        }
    }

    let at_seconds = at_seconds.ok_or_else(|| format!("line {line_number}: missing trigger at"))?;
    if !(0.0..=section.duration_seconds).contains(&at_seconds) {
        return Err(format!(
            "line {line_number}: trigger at={at_seconds} is outside section duration {}",
            section.duration_seconds
        ));
    }

    if let (Some(start_seconds), Some(end_seconds)) = (start_seconds, end_seconds) {
        if end_seconds <= start_seconds {
            return Err(format!(
                "line {line_number}: trigger end must be greater than start"
            ));
        }
    }

    section.sample_triggers.push(SampleTriggerEvent {
        time_seconds: section.start_seconds + at_seconds,
        sample_name,
        start_seconds,
        end_seconds,
        fade_in_seconds,
        fade_out_seconds,
        semitones,
        cents,
        gain,
        pan,
        rate,
    });

    Ok(())
}

fn parse_sample_asset_line(
    mut tokens: std::str::SplitWhitespace<'_>,
    line_number: usize,
) -> Result<SampleAssetSpec, String> {
    let name = tokens
        .next()
        .ok_or_else(|| format!("line {line_number}: missing sample asset name"))?
        .to_string();
    let mut path = None;

    for token in tokens {
        let (key, value) = token
            .split_once('=')
            .ok_or_else(|| format!("line {line_number}: expected key=value, got {token:?}"))?;
        match key {
            "file" | "path" => path = Some(value.to_string()),
            _ => {
                return Err(format!(
                    "line {line_number}: unknown sample asset control {key:?}"
                ));
            }
        }
    }

    let path = path.ok_or_else(|| format!("line {line_number}: missing sample asset file"))?;
    Ok(SampleAssetSpec { name, path })
}

fn parse_instrument_instance_line(
    mut tokens: std::str::SplitWhitespace<'_>,
    line_number: usize,
) -> Result<InstrumentInstanceSpec, String> {
    let id = tokens
        .next()
        .ok_or_else(|| format!("line {line_number}: missing instrument instance id"))?
        .to_string();
    let mut family = None;

    for token in tokens {
        let (key, value) = token
            .split_once('=')
            .ok_or_else(|| format!("line {line_number}: expected key=value, got {token:?}"))?;
        match key {
            "family" | "type" => family = Some(parse_instrument_family(value, line_number)?),
            _ => {
                return Err(format!(
                    "line {line_number}: unknown instrument instance control {key:?}"
                ));
            }
        }
    }

    let family =
        family.ok_or_else(|| format!("line {line_number}: missing instrument instance family"))?;
    Ok(InstrumentInstanceSpec { id, family })
}

fn resolve_instrument_target(
    value: &str,
    instrument_specs: &[InstrumentInstanceSpec],
    line_number: usize,
) -> Result<(Option<String>, InstrumentFamily), String> {
    if let Some(spec) = instrument_specs.iter().find(|spec| spec.id == value) {
        return Ok((Some(spec.id.clone()), spec.family));
    }

    parse_instrument_family(value, line_number).map(|family| (None, family))
}

fn compile_timeline_points(sections: &[ArrangementSection]) -> Vec<ControlPoint> {
    if sections.is_empty() {
        return Vec::new();
    }

    let mut points = Vec::new();

    for section in sections {
        match section.mode {
            SectionMode::Hold => {
                points.push(ControlPoint::new(section.start_seconds, section.state));
                points.push(ControlPoint::new(
                    section.start_seconds + section.duration_seconds,
                    section.state,
                ));
            }
            SectionMode::Ramp => {
                points.push(ControlPoint::new(
                    section.start_seconds,
                    section.entry_state,
                ));
                points.push(ControlPoint::new(
                    section.start_seconds + section.duration_seconds,
                    section.state,
                ));
            }
        }
    }

    points
}

fn parse_section_mode(value: &str, line_number: usize) -> Result<SectionMode, String> {
    match value {
        "hold" => Ok(SectionMode::Hold),
        "ramp" => Ok(SectionMode::Ramp),
        _ => Err(format!(
            "line {line_number}: invalid mode value {value:?}, expected hold or ramp"
        )),
    }
}

fn parse_instrument_family(value: &str, line_number: usize) -> Result<InstrumentFamily, String> {
    match value {
        "drone" | "pad_a" => Ok(InstrumentFamily::Drone),
        "harmonic" | "pad_b" => Ok(InstrumentFamily::Harmonic),
        "pulse" | "pulse_a" => Ok(InstrumentFamily::Pulse),
        "sample" | "oneshot" | "one_shot" => Ok(InstrumentFamily::Sample),
        "noise" => Ok(InstrumentFamily::Noise),
        "events" | "event" => Ok(InstrumentFamily::Events),
        "texture" => Ok(InstrumentFamily::Texture),
        _ => Err(format!(
            "line {line_number}: unknown instrument family {value:?}"
        )),
    }
}

fn set_instrument_level(state: &mut TimelineState, family: InstrumentFamily, value: f32) {
    *state.controls.level_mut(family) = value;
}

fn set_instrument_active(
    state: &mut TimelineState,
    family: InstrumentFamily,
    value: bool,
    line_number: usize,
) -> Result<(), String> {
    if !family.supports_active() {
        return Err(format!(
            "line {line_number}: texture does not support active state"
        ));
    }

    state.set_active(family, value);
    Ok(())
}

fn set_instrument_override(
    state: &mut TimelineState,
    family: InstrumentFamily,
    value: f32,
    line_number: usize,
) -> Result<(), String> {
    if !family.supports_override() {
        return Err(format!(
            "line {line_number}: texture does not support level overrides"
        ));
    }

    state.set_level_override(family, Some(value));
    Ok(())
}

fn parse_active(value: &str, line_number: usize) -> Result<bool, String> {
    let value = parse_f32(value, line_number, "active")?;
    Ok(value >= 0.5)
}

fn canonical_instrument_entries(
    state: TimelineState,
    instrument_specs: &[InstrumentInstanceSpec],
    current_entries: &[ArrangementInstrumentEntry],
) -> Vec<ArrangementInstrumentEntry> {
    if current_entries.is_empty() {
        return instrument_specs
            .iter()
            .map(|spec| ArrangementInstrumentEntry {
                target_id: Some(spec.id.clone()),
                family: spec.family,
                level: Some(state.controls.level(spec.family)),
                active: spec.family.supports_active().then(|| state.active(spec.family)),
                level_override: spec
                    .family
                    .supports_override()
                    .then(|| state.level_override(spec.family))
                    .flatten(),
            })
            .collect();
    }

    current_entries
        .iter()
        .map(|entry| ArrangementInstrumentEntry {
            target_id: entry.target_id.clone(),
            family: entry.family,
            level: Some(state.controls.level(entry.family)),
            active: entry.family.supports_active().then(|| state.active(entry.family)),
            level_override: entry
                .family
                .supports_override()
                .then(|| state.level_override(entry.family))
                .flatten(),
        })
        .collect()
}

fn parse_f32(value: &str, line_number: usize, label: &str) -> Result<f32, String> {
    value
        .parse::<f32>()
        .map_err(|_| format!("line {line_number}: invalid {label} value {value:?}"))
}

fn write_instrument_line(
    output: &mut String,
    target: &str,
    level: f32,
    active: Option<bool>,
    level_override: Option<f32>,
) {
    output.push_str(&format!(
        "instrument {} level={}",
        target,
        format_f32(level)
    ));
    if let Some(active) = active {
        output.push_str(&format!(" active={}", if active { 1 } else { 0 }));
    }
    if let Some(level_override) = level_override {
        output.push_str(&format!(" override={}", format_f32(level_override)));
    }
    output.push('\n');
}

fn format_mode(mode: SectionMode) -> &'static str {
    match mode {
        SectionMode::Hold => "hold",
        SectionMode::Ramp => "ramp",
    }
}

fn format_instrument_family(family: InstrumentFamily) -> &'static str {
    match family {
        InstrumentFamily::Drone => "drone",
        InstrumentFamily::Harmonic => "harmonic",
        InstrumentFamily::Pulse => "pulse",
        InstrumentFamily::Sample => "sample",
        InstrumentFamily::Noise => "noise",
        InstrumentFamily::Events => "events",
        InstrumentFamily::Texture => "texture",
    }
}

fn default_instrument_specs() -> Vec<InstrumentInstanceSpec> {
    vec![
        InstrumentInstanceSpec {
            id: String::from("drone_main"),
            family: InstrumentFamily::Drone,
        },
        InstrumentInstanceSpec {
            id: String::from("harmonic_main"),
            family: InstrumentFamily::Harmonic,
        },
        InstrumentInstanceSpec {
            id: String::from("pulse_main"),
            family: InstrumentFamily::Pulse,
        },
        InstrumentInstanceSpec {
            id: String::from("sample_main"),
            family: InstrumentFamily::Sample,
        },
        InstrumentInstanceSpec {
            id: String::from("noise_main"),
            family: InstrumentFamily::Noise,
        },
        InstrumentInstanceSpec {
            id: String::from("events_main"),
            family: InstrumentFamily::Events,
        },
        InstrumentInstanceSpec {
            id: String::from("texture_bus"),
            family: InstrumentFamily::Texture,
        },
    ]
}

fn format_f32(value: f32) -> String {
    let mut text = format!("{value:.4}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    if text.is_empty() {
        String::from("0")
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sections_into_cumulative_timeline_points() {
        let arrangement = parse_arrangement_text(
            "
            sample glass file=samples/glass.wav
            instance drone_main family=drone
            instance pulse_low family=pulse
            section intro duration=10 root=110
            instrument drone level=0.8
            instrument harmonic level=0.2

            section bloom duration=20 root=82.41
            instrument harmonic level=0.7
            instrument pulse level=0.3
            ",
            defaults(),
        )
        .unwrap();

        assert_eq!(arrangement.sample_assets().len(), 1);
        assert_eq!(arrangement.instrument_specs().len(), 2);
        assert_eq!(arrangement.instrument_specs()[0].id, "drone_main");
        assert_eq!(arrangement.instrument_specs()[1].id, "pulse_low");
        assert_eq!(arrangement.sample_assets()[0].name, "glass");
        assert_eq!(arrangement.sections().len(), 2);
        assert_eq!(arrangement.sections()[0].name, "intro");
        assert_eq!(arrangement.sections()[0].mode, SectionMode::Hold);
        assert_eq!(arrangement.sections()[0].instrument_entries.len(), 2);
        assert_close(arrangement.sections()[0].start_seconds, 0.0);
        assert_close(arrangement.sections()[1].start_seconds, 10.0);
        assert_close(arrangement.duration_seconds(), 30.0);
        assert_close(
            arrangement
                .timeline()
                .state_at(0.0)
                .unwrap()
                .controls
                .drone_level,
            0.8,
        );
        assert_close(
            arrangement
                .timeline()
                .state_at(10.0)
                .unwrap()
                .controls
                .harmonic_level,
            0.7,
        );
        assert_close(
            arrangement.timeline().state_at(10.0).unwrap().root_hz,
            82.41,
        );
    }

    #[test]
    fn carries_section_state_forward() {
        let arrangement = parse_arrangement_text(
            "
            section intro duration=10 density=0.2
            instrument noise level=0.3

            section hold duration=5
            instrument pulse level=0.4
            ",
            defaults(),
        )
        .unwrap();

        let hold = arrangement.timeline().state_at(10.0).unwrap();

        assert_close(hold.controls.density, 0.2);
        assert_close(hold.controls.noise_level, 0.3);
        assert_close(hold.controls.pulse_level, 0.4);
    }

    #[test]
    fn records_instrument_entry_controls() {
        let arrangement = parse_arrangement_text(
            "
            instance pulse_low family=pulse
            section intro duration=10
            instrument pulse_low level=0.4 active=0 override=0.7
            ",
            defaults(),
        )
        .unwrap();

        let section = &arrangement.sections()[0];
        let entry = &section.instrument_entries[0];

        assert_eq!(entry.family, InstrumentFamily::Pulse);
        assert_eq!(entry.target_id.as_deref(), Some("pulse_low"));
        assert_close(entry.level.unwrap(), 0.4);
        assert_eq!(entry.active, Some(false));
        assert_close(entry.level_override.unwrap(), 0.7);
        assert_close(section.state.controls.pulse_level, 0.4);
        assert!(!section.state.active(InstrumentFamily::Pulse));
        assert_close(section.state.level_override(InstrumentFamily::Pulse).unwrap(), 0.7);
    }

    #[test]
    fn records_instrument_parameter_controls() {
        let arrangement = parse_arrangement_text(
            "
            section intro duration=10 drone_spread=1.3 drone_detune=0.7 harmonic_mix=0.8 harmonic_shimmer=1.2 pulse_rate=1.4 pulse_length=0.6 noise_motion=0.5 sample_auto_rate=1.7 texture_drift=1.1
            ",
            defaults(),
        )
        .unwrap();

        let state = arrangement.sections()[0].state;
        assert_close(state.instrument_params.drone().spread, 1.3);
        assert_close(state.instrument_params.drone().detune, 0.7);
        assert_close(state.instrument_params.harmonic().mix, 0.8);
        assert_close(state.instrument_params.harmonic().shimmer, 1.2);
        assert_close(state.instrument_params.pulse().rate, 1.4);
        assert_close(state.instrument_params.pulse().length, 0.6);
        assert_close(state.instrument_params.noise().motion, 0.5);
        assert_close(state.instrument_params.sample().auto_rate, 1.7);
        assert_close(state.instrument_params.texture().drift, 1.1);
    }

    #[test]
    fn records_sample_trigger_events_with_absolute_times() {
        let arrangement = parse_arrangement_text(
            "
            sample glass file=samples/glass.wav
            sample bell file=samples/bell.wav
            section intro duration=10
            trigger sample name=glass at=2.5 gain=0.4 pan=-0.2 rate=1.3

            section bloom duration=5
            trigger sample name=bell at=1.0 start=0.25 end=0.75 fade_in=0.02 fade_out=0.04 semitones=7 cents=-12
            ",
            defaults(),
        )
        .unwrap();

        assert_eq!(arrangement.sections()[0].sample_triggers.len(), 1);
        assert_eq!(arrangement.sections()[1].sample_triggers.len(), 1);
        assert_close(
            arrangement.sections()[0].sample_triggers[0].time_seconds,
            2.5,
        );
        assert_close(
            arrangement.sections()[1].sample_triggers[0].time_seconds,
            11.0,
        );
        assert_eq!(arrangement.sample_triggers().len(), 2);
        assert_eq!(arrangement.sample_triggers()[0].sample_name, "glass");
        assert_eq!(arrangement.sample_triggers()[1].sample_name, "bell");
        assert_close(arrangement.sample_triggers()[0].gain.unwrap(), 0.4);
        assert_close(arrangement.sample_triggers()[0].pan.unwrap(), -0.2);
        assert_close(arrangement.sample_triggers()[0].rate.unwrap(), 1.3);
        assert_close(
            arrangement.sample_triggers()[1].start_seconds.unwrap(),
            0.25,
        );
        assert_close(arrangement.sample_triggers()[1].end_seconds.unwrap(), 0.75);
        assert_close(
            arrangement.sample_triggers()[1].fade_in_seconds.unwrap(),
            0.02,
        );
        assert_close(
            arrangement.sample_triggers()[1].fade_out_seconds.unwrap(),
            0.04,
        );
        assert_close(arrangement.sample_triggers()[1].semitones.unwrap(), 7.0);
        assert_close(arrangement.sample_triggers()[1].cents.unwrap(), -12.0);
    }

    #[test]
    fn serializes_to_parseable_canonical_text() {
        let arrangement = parse_arrangement_text(
            "
            sample glass file=samples/glass.wav
            instance drone_main family=drone
            instance texture_bus family=texture
            section intro duration=10 mode=hold density=0.2 brightness=0.3 space=0.4 instability=0.5 root=110 voices=3 octave_min=1 octave_max=2 event_attack_min=0.01 event_attack_max=0.15 event_decay_min=2 event_decay_max=6 drone_retune_seconds=8
            instrument drone level=0.8 active=1 override=0.7
            instrument texture level=0.2
            trigger sample name=glass at=2.5 start=0.1 end=0.4 fade_in=0.01 fade_out=0.02 semitones=12 cents=5 gain=0.4 pan=-0.2 rate=1.3
            ",
            defaults(),
        )
        .unwrap();

        let serialized = arrangement.to_text();
        let reparsed = parse_arrangement_text(&serialized, defaults()).unwrap();

        assert_eq!(reparsed.sample_assets()[0].name, "glass");
        assert_eq!(reparsed.instrument_specs(), arrangement.instrument_specs());
        assert_eq!(reparsed.sections().len(), 1);
        assert_close(reparsed.sections()[0].state.controls.drone_level, 0.8);
        assert_close(reparsed.sections()[0].state.controls.texture_level, 0.2);
        assert_close(reparsed.sample_triggers()[0].time_seconds, 2.5);
        assert_close(reparsed.sample_triggers()[0].semitones.unwrap(), 12.0);
        assert_close(reparsed.sample_triggers()[0].cents.unwrap(), 5.0);
    }

    #[test]
    fn supplies_default_instrument_instances_when_missing() {
        let arrangement = parse_arrangement_text(
            "
            section intro duration=10
            instrument drone level=0.8
            ",
            defaults(),
        )
        .unwrap();

        assert_eq!(arrangement.instrument_specs().len(), InstrumentFamily::COUNT);
        assert_eq!(arrangement.instrument_specs()[0].id, "drone_main");
    }

    #[test]
    fn hold_sections_jump_at_section_start() {
        let arrangement = parse_arrangement_text(
            "
            section intro duration=10 mode=hold density=0.2
            section next duration=10 mode=hold density=0.8
            ",
            defaults(),
        )
        .unwrap();

        assert_close(
            arrangement
                .timeline()
                .state_at(5.0)
                .unwrap()
                .controls
                .density,
            0.2,
        );
        assert_close(
            arrangement
                .timeline()
                .state_at(10.0)
                .unwrap()
                .controls
                .density,
            0.8,
        );
        assert_close(
            arrangement
                .timeline()
                .state_at(15.0)
                .unwrap()
                .controls
                .density,
            0.8,
        );
    }

    #[test]
    fn ramp_sections_interpolate_over_section_duration() {
        let arrangement = parse_arrangement_text(
            "
            section intro duration=10 mode=hold density=0.2
            section bloom duration=10 mode=ramp density=0.8
            ",
            defaults(),
        )
        .unwrap();

        assert_eq!(arrangement.sections()[1].mode, SectionMode::Ramp);
        assert_close(
            arrangement
                .timeline()
                .state_at(10.0)
                .unwrap()
                .controls
                .density,
            0.2,
        );
        assert_close(
            arrangement
                .timeline()
                .state_at(15.0)
                .unwrap()
                .controls
                .density,
            0.5,
        );
        assert_close(
            arrangement
                .timeline()
                .state_at(20.0)
                .unwrap()
                .controls
                .density,
            0.8,
        );
    }

    #[test]
    fn rejects_missing_duration() {
        let err = parse_arrangement_text("section intro density=0.2", defaults()).unwrap_err();

        assert!(err.contains("missing duration"));
    }

    #[test]
    fn rejects_unknown_section_mode() {
        let err =
            parse_arrangement_text("section intro duration=10 mode=slide", defaults()).unwrap_err();

        assert!(err.contains("invalid mode"));
    }

    #[test]
    fn rejects_instrument_outside_section() {
        let err = parse_arrangement_text("instrument drone level=0.8", defaults()).unwrap_err();

        assert!(err.contains("instrument outside section"));
    }

    #[test]
    fn rejects_trigger_outside_section() {
        let err = parse_arrangement_text("trigger sample at=1.0", defaults()).unwrap_err();

        assert!(err.contains("trigger outside section"));
    }

    #[test]
    fn rejects_trigger_outside_section_duration() {
        let err = parse_arrangement_text(
            "
            section intro duration=10
            trigger sample at=12
            ",
            defaults(),
        )
        .unwrap_err();

        assert!(err.contains("outside section duration"));
    }

    #[test]
    fn rejects_trigger_end_before_start() {
        let err = parse_arrangement_text(
            "
            sample glass file=samples/glass.wav
            section intro duration=10
            trigger sample name=glass at=1 start=0.8 end=0.2
            ",
            defaults(),
        )
        .unwrap_err();

        assert!(err.contains("end must be greater than start"));
    }

    #[test]
    fn rejects_sample_assets_after_sections() {
        let err = parse_arrangement_text(
            "
            section intro duration=10
            sample glass file=samples/glass.wav
            ",
            defaults(),
        )
        .unwrap_err();

        assert!(err.contains("before sections"));
    }

    #[test]
    fn rejects_texture_active_controls() {
        let err = parse_arrangement_text(
            "
            section intro duration=10
            instrument texture active=1
            ",
            defaults(),
        )
        .unwrap_err();

        assert!(err.contains("texture does not support active"));
    }

    fn defaults() -> ArrangementDefaults {
        ArrangementDefaults {
            controls: GardenControls::default(),
            root_hz: 110.0,
            voice_count: 3,
            octave_min: 1,
            octave_max: 2,
            event_attack_min: 0.015,
            event_attack_max: 0.195,
            event_decay_min: 2.0,
            event_decay_max: 8.0,
            drone_retune_seconds: 9.0,
        }
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.0001,
            "expected {expected}, got {actual}"
        );
    }
}
