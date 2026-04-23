use std::error::Error;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};

use crate::composition::arrangement::{Arrangement, SampleTriggerEvent};
use crate::composition::garden::{Garden, GardenConfig};
use crate::composition::timeline::ControlTimeline;
use crate::dsp::source::StereoSource;

pub struct AudioEngine {
    _stream: cpal::Stream,
}

#[derive(Clone)]
pub struct LiveAudioHandle {
    state: Arc<Mutex<LiveTransportState>>,
}

#[derive(Clone, Copy)]
pub struct LiveTransportSnapshot {
    pub playback: bool,
    pub position_seconds: f32,
    pub loop_section: Option<usize>,
}

#[derive(Clone)]
pub struct LiveTransportState {
    pub arrangement: Arrangement,
    pub garden_config: GardenConfig,
    pub playback: bool,
    pub position_seconds: f32,
    pub loop_section: Option<usize>,
    pub version: u64,
}

impl LiveAudioHandle {
    pub fn snapshot(&self) -> LiveTransportSnapshot {
        let state = self.state.lock().unwrap();
        LiveTransportSnapshot {
            playback: state.playback,
            position_seconds: state.position_seconds,
            loop_section: state.loop_section,
        }
    }

    pub fn update_project(
        &self,
        arrangement: Arrangement,
        sample_assets: Vec<crate::instruments::sampler::LoadedSampleAsset>,
    ) {
        let mut state = self.state.lock().unwrap();
        state.arrangement = arrangement;
        state.garden_config.sample_assets = sample_assets;
        state.version += 1;
    }

    pub fn toggle_playback(&self) {
        let mut state = self.state.lock().unwrap();
        state.playback = !state.playback;
    }

    pub fn set_position_seconds(&self, position_seconds: f32) {
        let mut state = self.state.lock().unwrap();
        state.position_seconds = position_seconds.max(0.0);
        state.version += 1;
    }

    pub fn set_loop_section(&self, loop_section: Option<usize>) {
        let mut state = self.state.lock().unwrap();
        state.loop_section = loop_section;
    }
}

impl AudioEngine {
    pub fn start(
        garden_config: GardenConfig,
        timeline: Option<ControlTimeline>,
        sample_triggers: Vec<SampleTriggerEvent>,
    ) -> Result<Self, Box<dyn Error>> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("no default output device available")?;
        let supported_config = device.default_output_config()?;
        let sample_format = supported_config.sample_format();
        let config: StreamConfig = supported_config.into();

        println!(
            "playing garden on {} at {} Hz, {} channel(s)",
            device.description()?,
            config.sample_rate,
            config.channels
        );
        println!("press Ctrl+C to stop");

        let stream = match sample_format {
            SampleFormat::F32 => build_stream::<f32>(
                &device,
                &config,
                garden_config,
                timeline.clone(),
                sample_triggers.clone(),
            )?,
            SampleFormat::I16 => build_stream::<i16>(
                &device,
                &config,
                garden_config,
                timeline.clone(),
                sample_triggers.clone(),
            )?,
            SampleFormat::U16 => {
                build_stream::<u16>(&device, &config, garden_config, timeline, sample_triggers)?
            }
            sample_format => {
                return Err(format!("unsupported output sample format: {sample_format:?}").into());
            }
        };

        stream.play()?;

        Ok(Self { _stream: stream })
    }

    pub fn start_live(
        garden_config: GardenConfig,
        arrangement: Arrangement,
    ) -> Result<(Self, LiveAudioHandle), Box<dyn Error>> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("no default output device available")?;
        let supported_config = device.default_output_config()?;
        let sample_format = supported_config.sample_format();
        let config: StreamConfig = supported_config.into();

        println!(
            "starting live composer on {} at {} Hz, {} channel(s)",
            device.description()?,
            config.sample_rate,
            config.channels
        );

        let state = Arc::new(Mutex::new(LiveTransportState {
            arrangement,
            garden_config,
            playback: false,
            position_seconds: 0.0,
            loop_section: None,
            version: 0,
        }));
        let handle = LiveAudioHandle {
            state: state.clone(),
        };

        let stream = match sample_format {
            SampleFormat::F32 => build_live_stream::<f32>(&device, &config, state.clone())?,
            SampleFormat::I16 => build_live_stream::<i16>(&device, &config, state.clone())?,
            SampleFormat::U16 => build_live_stream::<u16>(&device, &config, state)?,
            sample_format => {
                return Err(format!("unsupported output sample format: {sample_format:?}").into());
            }
        };

        stream.play()?;

        Ok((Self { _stream: stream }, handle))
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    garden_config: GardenConfig,
    timeline: Option<ControlTimeline>,
    sample_triggers: Vec<SampleTriggerEvent>,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>,
{
    let channels = config.channels as usize;
    let sample_rate = config.sample_rate as f32;
    let mut source = Garden::new(
        sample_rate,
        garden_config.clone(),
        timeline,
        sample_triggers,
    );

    println!(
        "seed: {}, root: {} Hz, active voices: {}",
        garden_config.seed,
        garden_config.root_hz,
        source.voice_count()
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

    device.build_output_stream(
        config,
        move |output: &mut [T], _| {
            write_output(output, channels, &mut source);
        },
        move |err| {
            eprintln!("audio stream error: {err}");
        },
        None,
    )
}

fn build_live_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    state: Arc<Mutex<LiveTransportState>>,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>,
{
    let channels = config.channels as usize;
    let sample_rate = config.sample_rate as f32;
    let mut source = LiveGardenSource::new(sample_rate, state.clone());
    let garden_config = state.lock().unwrap().garden_config.clone();
    if !garden_config.sample_assets.is_empty() {
        for sample in &garden_config.sample_assets {
            println!("sample [{}]: {}", sample.name(), sample.path().display());
        }
    }

    device.build_output_stream(
        config,
        move |output: &mut [T], _| {
            source.write_output(output, channels);
        },
        move |err| {
            eprintln!("audio stream error: {err}");
        },
        None,
    )
}

fn write_output<T, S>(output: &mut [T], channels: usize, source: &mut S)
where
    T: cpal::Sample + cpal::FromSample<f32>,
    S: StereoSource,
{
    for frame in output.chunks_mut(channels) {
        let sample = source.next_stereo();

        for (channel_index, channel_sample) in frame.iter_mut().enumerate() {
            let value = match channel_index {
                0 => sample.left,
                1 => sample.right,
                _ => (sample.left + sample.right) * 0.5,
            };

            *channel_sample = T::from_sample(value);
        }
    }
}

struct LiveGardenSource {
    sample_rate: f32,
    state: Arc<Mutex<LiveTransportState>>,
    version: Option<u64>,
    garden: Option<Garden>,
}

impl LiveGardenSource {
    fn new(sample_rate: f32, state: Arc<Mutex<LiveTransportState>>) -> Self {
        Self {
            sample_rate,
            state,
            version: None,
            garden: None,
        }
    }

    fn write_output<T>(&mut self, output: &mut [T], channels: usize)
    where
        T: cpal::Sample + cpal::FromSample<f32>,
    {
        let (arrangement_update, playback, arrangement_duration, loop_bounds) = {
            let state = self.state.lock().unwrap();
            let arrangement_update = (self.version != Some(state.version)).then(|| {
                (
                    state.arrangement.clone(),
                    state.garden_config.clone(),
                    state.version,
                    state.position_seconds,
                )
            });
            let arrangement_duration = state.arrangement.duration_seconds();
            let loop_bounds = state
                .loop_section
                .and_then(|index| state.arrangement.sections().get(index))
                .map(|section| {
                    (
                        section.start_seconds,
                        section.start_seconds + section.duration_seconds,
                    )
                });
            (
                arrangement_update,
                state.playback,
                arrangement_duration,
                loop_bounds,
            )
        };
        if let Some((arrangement, garden_config, new_version, position_seconds)) =
            arrangement_update
        {
            self.sync_arrangement(arrangement, garden_config, new_version, position_seconds);
        }

        if !playback {
            for sample in output.iter_mut() {
                *sample = T::from_sample(0.0);
            }
            return;
        }

        for frame in output.chunks_mut(channels) {
            let current_position = {
                let state = self.state.lock().unwrap();
                state.position_seconds
            };

            if let Some((loop_start, loop_end)) = loop_bounds {
                if current_position >= loop_end {
                    self.seek(loop_start);
                }
            } else if current_position >= arrangement_duration {
                let mut state = self.state.lock().unwrap();
                state.playback = false;
                for channel_sample in frame.iter_mut() {
                    *channel_sample = T::from_sample(0.0);
                }
                continue;
            }

            let sample = if let Some(garden) = &mut self.garden {
                garden.next_stereo()
            } else {
                Default::default()
            };

            for (channel_index, channel_sample) in frame.iter_mut().enumerate() {
                let value = match channel_index {
                    0 => sample.left,
                    1 => sample.right,
                    _ => (sample.left + sample.right) * 0.5,
                };
                *channel_sample = T::from_sample(value);
            }

            let mut state = self.state.lock().unwrap();
            state.position_seconds += 1.0 / self.sample_rate;
        }
    }

    fn sync_arrangement(
        &mut self,
        arrangement: Arrangement,
        garden_config: GardenConfig,
        version: u64,
        position_seconds: f32,
    ) {
        let mut garden = Garden::new(
            self.sample_rate,
            garden_config,
            Some(arrangement.timeline().clone()),
            arrangement.sample_triggers().to_vec(),
        );
        garden.seek_seconds(position_seconds);
        self.garden = Some(garden);
        self.version = Some(version);
    }

    fn seek(&mut self, position_seconds: f32) {
        if let Some(garden) = &mut self.garden {
            garden.seek_seconds(position_seconds);
        }
        let mut state = self.state.lock().unwrap();
        state.position_seconds = position_seconds.max(0.0);
    }
}
