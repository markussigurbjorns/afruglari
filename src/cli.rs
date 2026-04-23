use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use crate::composition::arrangement::{Arrangement, ArrangementDefaults, parse_arrangement_text};
use crate::composition::garden::GardenConfig;
use crate::composition::timeline::{ControlTimeline, parse_timeline_text_with_root};
use crate::instruments::sampler::{LoadedSample, LoadedSampleAsset};

pub struct AppConfig {
    pub gui: bool,
    pub garden: GardenConfig,
    pub duration_seconds: Option<f32>,
    pub output_path: Option<PathBuf>,
    pub timeline: Option<TimelineConfig>,
}

pub struct TimelineConfig {
    pub path: PathBuf,
    pub source: String,
    pub controls: ControlTimeline,
    pub arrangement: Option<Arrangement>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            gui: false,
            garden: GardenConfig::default(),
            duration_seconds: None,
            output_path: None,
            timeline: None,
        }
    }
}

pub fn parse_args() -> Result<AppConfig, Box<dyn Error>> {
    let mut config = AppConfig::default();
    let mut args = env::args().skip(1);
    let mut timeline_path = None;
    let mut arrangement_path = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--gui" => {
                config.gui = true;
            }
            "--seed" => {
                config.garden.seed = parse_next(&mut args, "--seed")?;
            }
            "--root" => {
                config.garden.root_hz = parse_next(&mut args, "--root")?;
            }
            "--voices" => {
                config.garden.voice_count =
                    parse_next::<usize>(&mut args, "--voices")?.clamp(1, 12);
            }
            "--duration" => {
                let duration = parse_next::<f32>(&mut args, "--duration")?;
                config.duration_seconds = Some(duration.max(0.0));
            }
            "--output" => {
                config.output_path =
                    Some(PathBuf::from(parse_next::<String>(&mut args, "--output")?));
            }
            "--sample-file" => {
                let path = PathBuf::from(parse_next::<String>(&mut args, "--sample-file")?);
                config.garden.sample_assets.push(LoadedSampleAsset::new(
                    "default",
                    LoadedSample::from_wav_path(&path)?,
                ));
            }
            "--timeline" => {
                timeline_path = Some(PathBuf::from(parse_next::<String>(
                    &mut args,
                    "--timeline",
                )?));
            }
            "--arrangement" => {
                arrangement_path = Some(PathBuf::from(parse_next::<String>(
                    &mut args,
                    "--arrangement",
                )?));
            }
            "--density" => {
                config.garden.controls.density = parse_normalized(&mut args, "--density")?;
            }
            "--brightness" => {
                config.garden.controls.brightness = parse_normalized(&mut args, "--brightness")?;
            }
            "--space" => {
                config.garden.controls.space = parse_normalized(&mut args, "--space")?;
            }
            "--instability" => {
                config.garden.controls.instability = parse_normalized(&mut args, "--instability")?;
            }
            "--drone" => {
                config.garden.controls.drone_level = parse_normalized(&mut args, "--drone")?;
            }
            "--harmonic" => {
                config.garden.controls.harmonic_level = parse_normalized(&mut args, "--harmonic")?;
            }
            "--pulse" => {
                config.garden.controls.pulse_level = parse_normalized(&mut args, "--pulse")?;
            }
            "--sample" => {
                config.garden.controls.sample_level = parse_normalized(&mut args, "--sample")?;
            }
            "--noise" => {
                config.garden.controls.noise_level = parse_normalized(&mut args, "--noise")?;
            }
            "--events" => {
                config.garden.controls.event_level = parse_normalized(&mut args, "--events")?;
            }
            "--texture" => {
                config.garden.controls.texture_level = parse_normalized(&mut args, "--texture")?;
            }
            unknown => {
                return Err(format!("unknown argument: {unknown}").into());
            }
        }
    }

    if timeline_path.is_some() && arrangement_path.is_some() {
        return Err("use either --timeline or --arrangement, not both".into());
    }

    if let Some(path) = timeline_path {
        let source = fs::read_to_string(&path)?;
        let controls = parse_timeline_text_with_root(
            &source,
            config.garden.controls,
            config.garden.root_hz,
            config.garden.voice_count,
            1,
            2,
            0.015,
            0.195,
            2.0,
            8.0,
            9.0,
        )
        .map_err(|err| format!("{}: {err}", path.display()))?;

        config.timeline = Some(TimelineConfig {
            path,
            source,
            controls,
            arrangement: None,
        });
    }

    if let Some(path) = arrangement_path {
        let source = fs::read_to_string(&path)?;
        let arrangement = parse_arrangement_text(
            &source,
            ArrangementDefaults {
                controls: config.garden.controls,
                root_hz: config.garden.root_hz,
                voice_count: config.garden.voice_count,
                octave_min: 1,
                octave_max: 2,
                event_attack_min: 0.015,
                event_attack_max: 0.195,
                event_decay_min: 2.0,
                event_decay_max: 8.0,
                drone_retune_seconds: 9.0,
            },
        )
        .map_err(|err| format!("{}: {err}", path.display()))?;
        let arrangement_parent = path.parent().unwrap_or(Path::new("."));
        let mut sample_assets = arrangement
            .sample_assets()
            .iter()
            .map(|asset| {
                let asset_path = arrangement_parent.join(&asset.path);
                let sample = LoadedSample::from_wav_path(&asset_path)?;
                Ok(LoadedSampleAsset::new(asset.name.clone(), sample))
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        if !config.garden.sample_assets.is_empty() {
            sample_assets.extend(config.garden.sample_assets.clone());
        }
        config.garden.sample_assets = sample_assets;
        let controls = arrangement.timeline().clone();

        config.timeline = Some(TimelineConfig {
            path,
            source,
            controls,
            arrangement: Some(arrangement),
        });
    }

    Ok(config)
}

fn parse_next<T>(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + 'static,
{
    let value = args
        .next()
        .ok_or_else(|| format!("missing value for {flag}"))?;
    Ok(value.parse()?)
}

fn parse_normalized(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<f32, Box<dyn Error>> {
    Ok(parse_next::<f32>(args, flag)?.clamp(0.0, 1.0))
}

fn print_help() {
    println!("afruglariV2");
    println!();
    println!("Usage:");
    println!("  cargo run -- [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --gui               Open the arrangement inspector GUI");
    println!("  --seed N            Deterministic random seed. Default: 12101854522218779061");
    println!("  --root HZ           Pitch-field root frequency. Default: 110.0");
    println!("  --voices N          Number of drone voices, 1-12. Default: 3");
    println!("  --duration SECONDS  Run duration. Default: run until interrupted");
    println!("  --output PATH       Render offline to a stereo WAV file");
    println!("  --sample-file PATH  Load a WAV file for one-shot sample playback");
    println!("  --timeline PATH     Load explicit macro automation timeline");
    println!("  --arrangement PATH  Load named sections that compile to a timeline");
    println!("  --density N         Activity and retune density, 0-1. Default: 0.35");
    println!("  --brightness N      Filter and tone brightness, 0-1. Default: 0.45");
    println!("  --space N           Delay and stereo space, 0-1. Default: 0.65");
    println!("  --instability N     Detune and drift amount, 0-1. Default: 0.25");
    println!("  --drone N           Drone layer level, 0-1. Default: 1.0");
    println!("  --harmonic N        Harmonic pad layer level, 0-1. Default: 0.0");
    println!("  --pulse N           Pulse instrument level, 0-1. Default: 0.0");
    println!("  --sample N          One-shot sample level, 0-1. Default: 0.0");
    println!("  --noise N           Filtered noise layer level, 0-1. Default: 0.0");
    println!("  --events N          Sparse event layer level, 0-1. Default: 0.0");
    println!("  --texture N         Tape memory texture level, 0-1. Default: 0.0");
}
