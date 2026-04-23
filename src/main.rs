use std::error::Error;
use std::thread;
use std::time::Duration;

use audio::engine::AudioEngine;
use cli::parse_args;
use gui::run_gui;
use render::wav::render_wav;

mod audio;
mod cli;
mod composition;
mod dsp;
mod gui;
mod instruments;
mod render;

fn main() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?;

    if let Some(timeline) = &config.timeline {
        if let Some(arrangement) = &timeline.arrangement {
            println!(
                "loaded arrangement from {} ({} section(s), {:.2} second(s))",
                timeline.path.display(),
                arrangement.sections().len(),
                arrangement.duration_seconds()
            );
        } else {
            println!(
                "loaded timeline from {} ({}, {} line(s))",
                timeline.path.display(),
                if timeline.controls.is_empty() {
                    "no control points"
                } else {
                    "ready"
                },
                timeline.source.lines().count()
            );
        }
    }

    if config.gui {
        run_gui(&config)?;
        return Ok(());
    }

    if let Some(output_path) = &config.output_path {
        let duration_seconds = config.duration_seconds.unwrap_or_else(|| {
            config
                .timeline
                .as_ref()
                .and_then(|timeline| timeline.arrangement.as_ref())
                .map_or(60.0, |arrangement| arrangement.duration_seconds())
        });
        render_wav(
            output_path,
            duration_seconds,
            config.garden,
            config.timeline.as_ref(),
        )?;
        return Ok(());
    }

    let timeline_controls = config
        .timeline
        .as_ref()
        .map(|timeline| timeline.controls.clone());
    let sample_triggers = config
        .timeline
        .as_ref()
        .and_then(|timeline| timeline.arrangement.as_ref())
        .map_or_else(Vec::new, |arrangement| {
            arrangement.sample_triggers().to_vec()
        });

    let _engine = AudioEngine::start(config.garden, timeline_controls, sample_triggers)?;

    if let Some(duration_seconds) = config.duration_seconds {
        thread::sleep(Duration::from_secs_f32(duration_seconds));
    } else {
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }

    Ok(())
}
