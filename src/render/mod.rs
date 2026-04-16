mod mix;
mod synth;
mod types;
mod wav;

use crate::grid::Event;
use mix::{StereoSample, apply_delay, apply_pump, mix_mono_event, soft_limit, soft_limit_mono};
use std::io;
use synth::{
    ToneControls, render_broken_radio, render_drone, render_fm_pulse, render_glass_harmonics,
    render_granular_dust, render_impact_kit, render_metallic_hit, render_noise_cloud,
    render_noise_organ, render_sub_machine, render_techno_pulse,
};
pub use types::{
    RenderConfig, RenderMode, RenderOverride, RenderSection, RenderVoice, parse_render_mode,
    render_mode_name, render_preset,
};
use wav::write_wav_stereo_i16;

pub fn render_events_to_wav(
    events: &[Event],
    path: impl AsRef<std::path::Path>,
    config: RenderConfig,
) -> io::Result<()> {
    render_events_to_wav_with_automation(events, path, config, &[], &[])
}

pub fn render_events_to_wav_with_sections(
    events: &[Event],
    path: impl AsRef<std::path::Path>,
    config: RenderConfig,
    sections: &[RenderSection],
) -> io::Result<()> {
    render_events_to_wav_with_automation(events, path, config, &[], sections)
}

pub fn render_events_to_wav_with_automation(
    events: &[Event],
    path: impl AsRef<std::path::Path>,
    config: RenderConfig,
    voices: &[RenderVoice],
    sections: &[RenderSection],
) -> io::Result<()> {
    let sample_rate = config.sample_rate;
    let max_step = events.iter().map(|event| event.step).max().unwrap_or(0);
    let total_seconds = (max_step as f32 + 1.0) * config.step_seconds + config.tail_seconds;
    let mut samples =
        vec![StereoSample::default(); (total_seconds * sample_rate as f32) as usize + 1];

    for event in events {
        render_event(
            event,
            &mut samples,
            render_config_for_event(config, voices, sections, event),
        );
    }

    apply_delay(&mut samples, config);
    apply_pump(&mut samples, config);
    soft_limit(&mut samples, config.drive);
    write_wav_stereo_i16(path, sample_rate, &samples)
}

fn render_config_for_event(
    base: RenderConfig,
    voices: &[RenderVoice],
    sections: &[RenderSection],
    event: &Event,
) -> RenderConfig {
    let mut config = base;
    if let Some(voice) = voices.iter().find(|voice| voice.voice == event.voice) {
        voice.overrides.apply_to(&mut config);
    }
    if let Some(section) = sections
        .iter()
        .find(|section| event.step >= section.start_step && event.step < section.end_step)
    {
        section.overrides.apply_to(&mut config);
    }
    config
}

fn render_event(event: &Event, samples: &mut [StereoSample], config: RenderConfig) {
    let start = (event.step as f32 * config.step_seconds * config.sample_rate as f32) as usize;
    let duration = event.duration_steps as f32 * config.step_seconds;
    let register = event.register.unwrap_or(0);
    let amp = 0.08 + event.intensity as f32 * 0.035;
    let timbre = event.timbre as f32;
    let tone = ToneControls::from_config(config);
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
                tone,
            ),
            1 => render_metallic_hit(
                &mut mono,
                0,
                config.sample_rate,
                duration,
                register,
                timbre,
                amp,
                tone,
            ),
            _ => render_noise_cloud(
                &mut mono,
                0,
                config.sample_rate,
                duration,
                register,
                timbre,
                amp,
                tone,
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
            tone,
        ),
        RenderMode::ImpactKit => render_impact_kit(
            &mut mono,
            0,
            config.sample_rate,
            duration,
            register,
            timbre,
            amp,
            event.voice,
            tone,
        ),
        RenderMode::TechnoPulse => render_techno_pulse(
            &mut mono,
            0,
            config.sample_rate,
            duration,
            register,
            timbre,
            amp,
            event.voice,
            tone,
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
            tone,
        ),
        RenderMode::Metallic => render_metallic_hit(
            &mut mono,
            0,
            config.sample_rate,
            duration * 1.8,
            register.saturating_add(event.voice as u8),
            timbre + event.voice as f32,
            amp * 0.9,
            tone,
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
            tone,
        ),
        RenderMode::GranularDust => render_granular_dust(
            &mut mono,
            0,
            config.sample_rate,
            duration,
            register,
            timbre,
            amp,
            event.voice,
            tone,
        ),
        RenderMode::SubMachine => render_sub_machine(
            &mut mono,
            0,
            config.sample_rate,
            duration,
            register,
            timbre,
            amp,
            event.voice,
            tone,
        ),
        RenderMode::GlassHarmonics => render_glass_harmonics(
            &mut mono,
            0,
            config.sample_rate,
            duration,
            register,
            timbre,
            amp,
            event.voice,
            tone,
        ),
    }

    soft_limit_mono(&mut mono, config.drive);
    mix_mono_event(samples, start, &mono, event.voice, config.stereo_width);
}
