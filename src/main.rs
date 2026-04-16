use afruglari::{
    GenerateError, GenerationConfig, PiecePreset, RenderConfig, RenderMode, generate_batch,
    generate_batch_from_config, generate_from_config, generate_one, preset_names, scan_metadata,
};

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();

    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_usage();
        return;
    }

    let result = if args.first().is_some_and(|arg| arg == "--config") {
        run_config(&args)
    } else if args.first().is_some_and(|arg| arg == "--batch-config") {
        run_batch_config(&args)
    } else if args.first().is_some_and(|arg| arg == "--batch") {
        run_batch(&args)
    } else if args.first().is_some_and(|arg| arg == "--scan") {
        run_scan(&args)
    } else {
        run_single(&args)
    };

    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run_config(args: &[String]) -> Result<(), GenerateError> {
    let path = args.get(1).ok_or_else(|| {
        GenerateError::Config("missing config path; usage: --config pieces/file.toml".to_string())
    })?;
    let result = generate_from_config(path)?;
    print_result(&result);
    Ok(())
}

fn run_batch_config(args: &[String]) -> Result<(), GenerateError> {
    let path = args.get(1).ok_or_else(|| {
        GenerateError::Config(
            "missing config path; usage: --batch-config pieces/file.toml count output_dir"
                .to_string(),
        )
    })?;
    let count = args
        .get(2)
        .ok_or_else(|| GenerateError::Config("missing batch-config count".to_string()))?
        .parse::<usize>()
        .map_err(|_| GenerateError::Config("invalid batch-config count".to_string()))?;
    let output_dir = args.get(3).ok_or_else(|| {
        GenerateError::Config("missing batch-config output directory".to_string())
    })?;

    let results = generate_batch_from_config(path, count, output_dir)?;
    for result in &results {
        print_result(result);
    }
    println!("batch renders={}", results.len());
    Ok(())
}

fn run_batch(args: &[String]) -> Result<(), GenerateError> {
    let preset = args
        .get(1)
        .and_then(|name| PiecePreset::parse(name))
        .ok_or_else(|| GenerateError::Config("missing or invalid batch preset".to_string()))?;
    let count = args
        .get(2)
        .ok_or_else(|| GenerateError::Config("missing batch count".to_string()))?
        .parse::<usize>()
        .map_err(|_| GenerateError::Config("invalid batch count".to_string()))?;
    let output_dir = args
        .get(3)
        .cloned()
        .unwrap_or_else(|| format!("target/renders/{}", preset.name()));
    let mode = args
        .get(4)
        .map(|mode| {
            parse_render_mode(mode)
                .ok_or_else(|| GenerateError::Config("invalid render mode".to_string()))
        })
        .transpose()?;

    let results = generate_batch(preset, count, output_dir, mode)?;
    for result in &results {
        print_result(result);
    }
    println!("batch renders={}", results.len());
    Ok(())
}

fn run_scan(args: &[String]) -> Result<(), GenerateError> {
    let dir = args
        .get(1)
        .ok_or_else(|| GenerateError::Config("missing scan directory".to_string()))?;
    let filter = parse_scan_filter(&args[2..])?;
    let entries = scan_metadata(dir, &filter)?;

    println!(
        "{:<44} {:>6} {:>10} {:>7} {:>7} {:<16} output",
        "metadata", "events", "collisions", "voices", "sections", "voice_density"
    );
    for entry in &entries {
        println!(
            "{:<44} {:>6} {:>10} {:>7} {:>7} {:<16} {}",
            entry.metadata_path.display(),
            entry.metadata.events,
            entry.metadata.collisions,
            entry.metadata.voice_render_count,
            entry.metadata.section_render_count,
            format!("{:?}", entry.metadata.voice_density),
            entry.metadata.output.display()
        );
    }
    println!("matches={}", entries.len());
    Ok(())
}

fn run_single(args: &[String]) -> Result<(), GenerateError> {
    let preset = args
        .first()
        .and_then(|name| PiecePreset::parse(name))
        .unwrap_or(PiecePreset::Example);
    let seed = args
        .get(1)
        .map(|seed| {
            seed.parse::<u64>()
                .map_err(|_| GenerateError::Config("invalid seed".to_string()))
        })
        .transpose()?
        .unwrap_or(0);
    let output = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| format!("target/{}-{}.wav", preset.name(), seed));
    let mode = args
        .get(3)
        .map(|mode| {
            parse_render_mode(mode)
                .ok_or_else(|| GenerateError::Config("invalid render mode".to_string()))
        })
        .transpose()?
        .unwrap_or_else(|| default_mode_for_preset(preset));
    let config = RenderConfig {
        mode,
        ..RenderConfig::default()
    };
    let result = generate_one(&GenerationConfig {
        preset,
        piece: None,
        sections: Vec::new(),
        section_renders: Vec::new(),
        voice_renders: Vec::new(),
        constraints: Vec::new(),
        seed,
        output: output.into(),
        render: config,
    })?;

    print_result(&result);
    Ok(())
}

fn parse_render_mode(name: &str) -> Option<RenderMode> {
    afruglari::parse_render_mode(name)
}

fn default_mode_for_preset(preset: PiecePreset) -> RenderMode {
    afruglari::workflow::default_mode_for_preset(preset)
}

fn print_result(result: &afruglari::GenerateResult) {
    let metadata = &result.metadata;
    println!(
        "piece={} seed={} mode={}",
        metadata.piece,
        metadata.seed,
        afruglari::render_mode_name(metadata.render_mode)
    );
    println!("wrote {}", metadata.output.display());
    println!("metadata {}", metadata.json_path().display());
    println!(
        "events={} collisions={} voice_density={:?}",
        metadata.events, metadata.collisions, metadata.voice_density
    );
}

fn print_usage() {
    println!("usage: cargo run -- [preset] [seed] [output.wav] [render-mode]");
    println!("       cargo run -- --config pieces/file.toml");
    println!("       cargo run -- --batch-config pieces/file.toml count output_dir");
    println!("       cargo run -- --batch preset count output_dir [render-mode]");
    println!("       cargo run -- --scan dir [filters]");
    println!();
    println!("presets: {}", preset_names().join(", "));
    println!(
        "render modes: percussive, impact-kit, drone, broken-radio, metallic, noise-organ, granular-dust, sub-machine, glass-harmonics"
    );
    println!();
    println!("examples:");
    println!("  cargo run -- sparse-cracks 42 target/sparse-42.wav");
    println!("  cargo run -- slow-noise-blocks 7 target/slow.wav drone");
    println!("  cargo run -- dense-collision-field 99 target/dense.wav noise-organ");
    println!("  cargo run -- --config pieces/sparse.toml");
    println!(
        "  cargo run -- --batch-config pieces/sectioned-form.toml 8 target/renders/sectioned-form"
    );
    println!("  cargo run -- --batch sparse-cracks 8 target/renders/sparse broken-radio");
    println!("  cargo run -- --scan target/renders/sparse --min-collisions 5");
}

fn parse_scan_filter(args: &[String]) -> Result<afruglari::MetadataFilter, GenerateError> {
    let mut filter = afruglari::MetadataFilter::default();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let value = args
            .get(index + 1)
            .ok_or_else(|| GenerateError::Config(format!("{flag} requires a value")))?;
        match flag {
            "--min-collisions" => filter.min_collisions = Some(parse_usize(value, flag)?),
            "--max-collisions" => filter.max_collisions = Some(parse_usize(value, flag)?),
            "--min-events" => filter.min_events = Some(parse_usize(value, flag)?),
            "--max-events" => filter.max_events = Some(parse_usize(value, flag)?),
            "--voice-dominates" => filter.voice_dominates = Some(parse_usize(value, flag)?),
            _ => {
                return Err(GenerateError::Config(format!(
                    "unknown scan filter '{flag}'"
                )));
            }
        }
        index += 2;
    }
    Ok(filter)
}

fn parse_usize(value: &str, flag: &str) -> Result<usize, GenerateError> {
    value
        .parse()
        .map_err(|_| GenerateError::Config(format!("{flag} expects an unsigned integer")))
}
