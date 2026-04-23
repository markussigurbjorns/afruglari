use std::error::Error;
use std::fs;
use std::path::Path;
use std::time::Instant;

use crate::cli::TimelineConfig;
use crate::composition::garden::{Garden, GardenConfig};
use crate::dsp::source::StereoSource;

pub const DEFAULT_RENDER_SAMPLE_RATE: u32 = 44_100;
const CHANNELS: u16 = 2;

pub fn render_wav(
    path: &Path,
    duration_seconds: f32,
    garden_config: GardenConfig,
    timeline: Option<&TimelineConfig>,
) -> Result<(), Box<dyn Error>> {
    let sample_rate = DEFAULT_RENDER_SAMPLE_RATE;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    let spec = hound::WavSpec {
        channels: CHANNELS,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    let mut garden = Garden::new(
        sample_rate as f32,
        garden_config.clone(),
        timeline.map(|timeline| timeline.controls.clone()),
        timeline
            .and_then(|timeline| timeline.arrangement.as_ref())
            .map_or_else(Vec::new, |arrangement| {
                arrangement.sample_triggers().to_vec()
            }),
    );
    let frames = (duration_seconds.max(0.0) * sample_rate as f32).round() as usize;

    println!(
        "rendering {} second(s) to {} at {} Hz",
        duration_seconds,
        path.display(),
        sample_rate
    );
    println!(
        "seed: {}, root: {} Hz, active voices: {}",
        garden_config.seed,
        garden_config.root_hz,
        garden.voice_count()
    );
    println!(
        "controls: density {:.2}, brightness {:.2}, space {:.2}, instability {:.2}, drone {:.2}, harmonic {:.2}, pulse {:.2}, sample {:.2}, noise {:.2}, events {:.2}, texture {:.2}",
        garden_config.controls.density,
        garden_config.controls.brightness,
        garden_config.controls.space,
        garden_config.controls.instability,
        garden_config.controls.drone_level,
        garden_config.controls.harmonic_level,
        garden_config.controls.pulse_level,
        garden_config.controls.sample_level,
        garden_config.controls.noise_level,
        garden_config.controls.event_level,
        garden_config.controls.texture_level
    );
    if !garden_config.sample_assets.is_empty() {
        for sample in &garden_config.sample_assets {
            println!("sample [{}]: {}", sample.name(), sample.path().display());
        }
    }

    let started_at = Instant::now();
    let mut progress = RenderProgress::new(frames);

    for frame_index in 0..frames {
        let sample = garden.next_stereo();
        writer.write_sample(to_i16(sample.left))?;
        writer.write_sample(to_i16(sample.right))?;
        progress.tick(frame_index + 1);
    }

    writer.finalize()?;
    write_sidecar(path, duration_seconds, garden_config, sample_rate, timeline)?;
    println!(
        "rendered {:.3} second(s), {} frame(s), {} channel(s), {} Hz in {:.2?}",
        duration_seconds,
        frames,
        CHANNELS,
        sample_rate,
        started_at.elapsed()
    );
    Ok(())
}

struct RenderProgress {
    total_frames: usize,
    next_percent: usize,
}

impl RenderProgress {
    fn new(total_frames: usize) -> Self {
        if total_frames == 0 {
            println!("progress: 100%");
        }

        Self {
            total_frames,
            next_percent: 10,
        }
    }

    fn tick(&mut self, completed_frames: usize) {
        if self.total_frames == 0 || self.next_percent > 100 {
            return;
        }

        let percent = completed_frames * 100 / self.total_frames;
        while percent >= self.next_percent && self.next_percent <= 100 {
            println!("progress: {}%", self.next_percent);
            self.next_percent += 10;
        }
    }
}

fn write_sidecar(
    wav_path: &Path,
    duration_seconds: f32,
    garden_config: GardenConfig,
    sample_rate: u32,
    timeline: Option<&TimelineConfig>,
) -> Result<(), Box<dyn Error>> {
    let sidecar_path = wav_path.with_extension("txt");
    let metadata = render_metadata(
        wav_path,
        duration_seconds,
        garden_config,
        sample_rate,
        timeline,
    );

    fs::write(&sidecar_path, metadata)?;
    println!("wrote metadata to {}", sidecar_path.display());
    Ok(())
}

fn render_metadata(
    wav_path: &Path,
    duration_seconds: f32,
    garden_config: GardenConfig,
    sample_rate: u32,
    timeline: Option<&TimelineConfig>,
) -> String {
    let controls = garden_config.controls;
    let output = wav_path.display().to_string();
    let mut reproduce = format!(
        "cargo run -- --seed {} --root {} --voices {} --duration {} --density {} --brightness {} --space {} --instability {} --drone {} --harmonic {} --pulse {} --sample {} --noise {} --events {} --texture {}",
        garden_config.seed,
        garden_config.root_hz,
        garden_config.voice_count,
        duration_seconds,
        controls.density,
        controls.brightness,
        controls.space,
        controls.instability,
        controls.drone_level,
        controls.harmonic_level,
        controls.pulse_level,
        controls.sample_level,
        controls.noise_level,
        controls.event_level,
        controls.texture_level,
    );
    if let Some(timeline) = timeline {
        let timeline_flag = if timeline.arrangement.is_some() {
            "--arrangement"
        } else {
            "--timeline"
        };
        reproduce.push_str(&format!(
            " {timeline_flag} {}",
            shell_quote(&timeline.path.display().to_string())
        ));
    }
    for sample in &garden_config.sample_assets {
        if sample.name() == "default" {
            reproduce.push_str(&format!(
                " --sample-file {}",
                shell_quote(&sample.path().display().to_string())
            ));
        }
    }
    reproduce.push_str(&format!(" --output {}", shell_quote(&output)));

    let timeline_section = timeline.map_or(String::new(), |timeline| {
        let arrangement_summary =
            timeline
                .arrangement
                .as_ref()
                .map_or(String::new(), |arrangement| {
                    let mut summary = format!(
                        "arrangement_duration_seconds: {}\narrangement_sections:\n",
                        arrangement.duration_seconds()
                    );
                    for section in arrangement.sections() {
                        summary.push_str(&format!(
                            "  - {}: start={} duration={} mode={:?}\n",
                            section.name,
                            section.start_seconds,
                            section.duration_seconds,
                            section.mode
                        ));
                        for entry in &section.instrument_entries {
                            summary.push_str(&format!(
                                "    instrument {:?}: level={:?} active={:?} override={:?}\n",
                                entry.family, entry.level, entry.active, entry.level_override
                            ));
                        }
                        for trigger in &section.sample_triggers {
                            summary.push_str(&format!(
                                "    trigger Sample [{}]: time={} start={:?} end={:?} fade_in={:?} fade_out={:?} semitones={:?} cents={:?} gain={:?} pan={:?} rate={:?}\n",
                                trigger.sample_name,
                                trigger.time_seconds,
                                trigger.start_seconds,
                                trigger.end_seconds,
                                trigger.fade_in_seconds,
                                trigger.fade_out_seconds,
                                trigger.semitones,
                                trigger.cents,
                                trigger.gain,
                                trigger.pan,
                                trigger.rate
                            ));
                        }
                    }
                    if !arrangement.sample_assets().is_empty() {
                        summary.push_str("arrangement_sample_assets:\n");
                        for asset in arrangement.sample_assets() {
                            summary.push_str(&format!("  - {}: {}\n", asset.name, asset.path));
                        }
                        summary.push('\n');
                    }
                    summary.push('\n');
                    summary
                });

        format!(
            "\
timeline_path: {path}

{arrangement_summary}\
timeline:
{source}

",
            path = timeline.path.display(),
            arrangement_summary = arrangement_summary,
            source = timeline.source.trim_end()
        )
    });

    format!(
        "\
afruglariV2 render

output: {output}
sample_rate: {sample_rate}
channels: {CHANNELS}
duration_seconds: {duration_seconds}

seed: {seed}
root_hz: {root_hz}
voices: {voices}

controls:
  density: {density}
  brightness: {brightness}
  space: {space}
  instability: {instability}
  drone: {drone}
  harmonic: {harmonic}
  pulse: {pulse}
  sample: {sample}
  noise: {noise}
  events: {events}
  texture: {texture}

{timeline_section}\
reproduce:
{reproduce}
",
        seed = garden_config.seed,
        root_hz = garden_config.root_hz,
        voices = garden_config.voice_count,
        density = controls.density,
        brightness = controls.brightness,
        space = controls.space,
        instability = controls.instability,
        drone = controls.drone_level,
        harmonic = controls.harmonic_level,
        pulse = controls.pulse_level,
        sample = controls.sample_level,
        noise = controls.noise_level,
        events = controls.event_level,
        texture = controls.texture_level,
        timeline_section = timeline_section,
    )
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn to_i16(sample: f32) -> i16 {
    let sample = soft_clip(sample);
    (sample * i16::MAX as f32) as i16
}

fn soft_clip(sample: f32) -> f32 {
    sample.clamp(-1.25, 1.25).tanh()
}
