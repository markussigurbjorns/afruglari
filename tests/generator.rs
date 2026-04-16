use afruglari::{
    AntiRepeatWindow, Constraint, DifferentAdjacent, Domain, Event, ExactCount, Implication,
    Literal, MaxCount, MaxRun, MinCount, MinDensityWindow, Param, PhaseResponse, PieceConfig,
    PiecePreset, RenderConfig, RenderMode, SlowChange, Value, VarId, events_from_grid,
    example_piece, generate_batch_from_config, generate_one, piece_from_preset,
    render_events_to_wav, scan_metadata, solve, solve_with_seed,
};

#[test]
fn max_run_limits_consecutive_true_values() {
    let mut engine = afruglari::Engine::new(vec![Domain::bool(); 4]);
    engine.add_constraint(MaxRun::new((0..4).map(VarId).collect(), 2));

    engine.assign(VarId(0), Value::Bool(true)).unwrap();
    engine.assign(VarId(1), Value::Bool(true)).unwrap();
    engine.propagate_all().unwrap();

    assert_eq!(engine.value(VarId(2)), Some(Value::Bool(false)));
}

#[test]
fn exact_count_forces_remaining_values() {
    let mut engine = afruglari::Engine::new(vec![Domain::bool(); 4]);
    engine.add_constraint(ExactCount::new(
        (0..4).map(VarId).collect(),
        Value::Bool(true),
        2,
    ));

    engine.assign(VarId(0), Value::Bool(true)).unwrap();
    engine.assign(VarId(1), Value::Bool(true)).unwrap();
    engine.propagate_all().unwrap();

    assert_eq!(engine.value(VarId(2)), Some(Value::Bool(false)));
    assert_eq!(engine.value(VarId(3)), Some(Value::Bool(false)));
}

#[test]
fn min_count_forces_all_remaining_possible_values() {
    let mut engine = afruglari::Engine::new(vec![Domain::bool(); 4]);
    engine.add_constraint(MinCount::new(
        (0..4).map(VarId).collect(),
        Value::Bool(true),
        2,
    ));

    engine.assign(VarId(0), Value::Bool(false)).unwrap();
    engine.assign(VarId(1), Value::Bool(false)).unwrap();
    engine.propagate_all().unwrap();

    assert_eq!(engine.value(VarId(2)), Some(Value::Bool(true)));
    assert_eq!(engine.value(VarId(3)), Some(Value::Bool(true)));
}

#[test]
fn max_count_removes_value_after_limit_is_reached() {
    let mut engine = afruglari::Engine::new(vec![Domain::bool(); 4]);
    engine.add_constraint(MaxCount::new(
        (0..4).map(VarId).collect(),
        Value::Bool(true),
        1,
    ));

    engine.assign(VarId(0), Value::Bool(true)).unwrap();
    engine.propagate_all().unwrap();

    assert_eq!(engine.value(VarId(1)), Some(Value::Bool(false)));
    assert_eq!(engine.value(VarId(2)), Some(Value::Bool(false)));
    assert_eq!(engine.value(VarId(3)), Some(Value::Bool(false)));
}

#[test]
fn min_density_forces_last_possible_active_event() {
    let mut engine = afruglari::Engine::new(vec![Domain::bool(); 3]);
    engine.add_constraint(MinDensityWindow::new((0..3).map(VarId).collect(), 3, 1));

    engine.assign(VarId(0), Value::Bool(false)).unwrap();
    engine.assign(VarId(1), Value::Bool(false)).unwrap();
    engine.propagate_all().unwrap();

    assert_eq!(engine.value(VarId(2)), Some(Value::Bool(true)));
}

#[test]
fn implication_propagates_forward_and_backward() {
    let mut engine = afruglari::Engine::new(vec![Domain::bool(); 2]);
    engine.add_constraint(Implication::new(
        Literal {
            var: VarId(0),
            value: Value::Bool(true),
        },
        Literal {
            var: VarId(1),
            value: Value::Bool(false),
        },
    ));

    engine.assign(VarId(0), Value::Bool(true)).unwrap();
    engine.propagate_all().unwrap();
    assert_eq!(engine.value(VarId(1)), Some(Value::Bool(false)));

    let mut engine = afruglari::Engine::new(vec![Domain::bool(); 2]);
    engine.add_constraint(Implication::new(
        Literal {
            var: VarId(0),
            value: Value::Bool(true),
        },
        Literal {
            var: VarId(1),
            value: Value::Bool(false),
        },
    ));
    engine.assign(VarId(1), Value::Bool(true)).unwrap();
    engine.propagate_all().unwrap();
    assert_eq!(engine.value(VarId(0)), Some(Value::Bool(false)));
}

#[test]
fn slow_change_keeps_values_constant_inside_windows() {
    let mut engine = afruglari::Engine::new(vec![Domain::small_range(0, 6); 8]);
    engine.add_constraint(SlowChange::new((0..8).map(VarId).collect(), 4));

    engine.assign(VarId(2), Value::Int(5)).unwrap();
    engine.propagate_all().unwrap();

    for index in 0..4 {
        assert_eq!(engine.value(VarId(index)), Some(Value::Int(5)));
    }
    assert_eq!(engine.domain(VarId(4)).size(), 7);
}

#[test]
fn different_adjacent_removes_neighbor_value() {
    let mut engine = afruglari::Engine::new(vec![Domain::small_range(0, 2); 3]);
    engine.add_constraint(DifferentAdjacent::new((0..3).map(VarId).collect()));

    engine.assign(VarId(1), Value::Int(2)).unwrap();
    engine.propagate_all().unwrap();

    assert!(!engine.domain(VarId(0)).contains(Value::Int(2)));
    assert!(!engine.domain(VarId(2)).contains(Value::Int(2)));
}

#[test]
fn anti_repeat_window_limits_repetition_inside_windows() {
    let mut engine = afruglari::Engine::new(vec![Domain::small_range(0, 2); 4]);
    engine.add_constraint(AntiRepeatWindow::new((0..4).map(VarId).collect(), 4, 2));

    engine.assign(VarId(0), Value::Int(1)).unwrap();
    engine.assign(VarId(1), Value::Int(1)).unwrap();
    engine.propagate_all().unwrap();

    assert!(!engine.domain(VarId(2)).contains(Value::Int(1)));
    assert!(!engine.domain(VarId(3)).contains(Value::Int(1)));
}

#[test]
fn phase_response_forces_last_possible_pairs() {
    let mut engine = afruglari::Engine::new(vec![Domain::bool(); 4]);
    engine.add_constraint(PhaseResponse::new(
        vec![(VarId(0), VarId(2)), (VarId(1), VarId(3))],
        1,
    ));

    engine.assign(VarId(0), Value::Bool(false)).unwrap();
    engine.propagate_all().unwrap();

    assert_eq!(engine.value(VarId(1)), Some(Value::Bool(true)));
    assert_eq!(engine.value(VarId(3)), Some(Value::Bool(true)));
}

#[test]
fn example_piece_solves_and_satisfies_structural_checks() {
    let (grid, mut engine) = example_piece();
    assert!(solve(&mut engine));

    let events = events_from_grid(&engine, &grid);
    assert_eq!(events.len(), 28);

    for voice in 0..3 {
        let active = grid.voice_param(voice, Param::Active);
        assert!(MaxRun::new(active, 3).is_satisfied_complete(&engine));
    }

    assert!(MinDensityWindow::new(grid.all_active(), 9, 1).is_satisfied_complete(&engine));

    let collisions = (0..32)
        .filter(|step| {
            engine.value(grid.var(1, *step, Param::Active)) == Some(Value::Bool(true))
                && engine.value(grid.var(2, *step, Param::Active)) == Some(Value::Bool(true))
        })
        .count();
    assert!(collisions >= 2);

    assert!(SlowChange::new(grid.voice_param(2, Param::Timbre), 4).is_satisfied_complete(&engine));

    let first_half = (0..16)
        .filter(|step| engine.value(grid.var(0, *step, Param::Active)) == Some(Value::Bool(true)))
        .count();
    let second_half = (16..32)
        .filter(|step| engine.value(grid.var(0, *step, Param::Active)) == Some(Value::Bool(true)))
        .count();
    assert!(first_half > second_half);
}

#[test]
fn renderer_writes_a_valid_wav_file() {
    let path = std::env::temp_dir().join("afruglari-renderer-test.wav");
    let events = vec![Event {
        voice: 2,
        step: 0,
        duration_steps: 1,
        register: Some(3),
        timbre: 5,
        intensity: 5,
    }];

    render_events_to_wav(
        &events,
        &path,
        RenderConfig {
            sample_rate: 8_000,
            step_seconds: 0.05,
            tail_seconds: 0.05,
            mode: RenderMode::NoiseOrgan,
            ..RenderConfig::default()
        },
    )
    .unwrap();

    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 2);
    assert!(bytes.len() > 44);

    let _ = std::fs::remove_file(path);
}

#[test]
fn all_render_modes_write_valid_wav_files() {
    let events = vec![Event {
        voice: 1,
        step: 0,
        duration_steps: 1,
        register: Some(2),
        timbre: 4,
        intensity: 5,
    }];

    for mode in [
        RenderMode::Percussive,
        RenderMode::ImpactKit,
        RenderMode::TechnoPulse,
        RenderMode::Drone,
        RenderMode::BrokenRadio,
        RenderMode::Metallic,
        RenderMode::NoiseOrgan,
        RenderMode::GranularDust,
        RenderMode::SubMachine,
        RenderMode::GlassHarmonics,
    ] {
        let path = std::env::temp_dir().join(format!(
            "afruglari-render-mode-{}.wav",
            afruglari::render_mode_name(mode)
        ));
        render_events_to_wav(
            &events,
            &path,
            RenderConfig {
                sample_rate: 8_000,
                step_seconds: 0.04,
                tail_seconds: 0.04,
                mode,
                ..RenderConfig::default()
            },
        )
        .unwrap();

        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert!(bytes.len() > 44, "{mode:?} produced no audio payload");
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn all_presets_solve_with_seeded_search() {
    for preset in [
        PiecePreset::Example,
        PiecePreset::SparseCracks,
        PiecePreset::DenseCollisionField,
        PiecePreset::SlowNoiseBlocks,
        PiecePreset::MetallicSwarm,
    ] {
        let (grid, mut engine) = piece_from_preset(preset);
        assert!(solve_with_seed(&mut engine, 17), "{preset:?} did not solve");
        assert!(!events_from_grid(&engine, &grid).is_empty());
    }
}

#[test]
fn config_parser_accepts_piece_and_render_sections() {
    let config = afruglari::GenerationConfig::parse(
        r#"
        [piece]
        preset = "slow-noise-blocks"
        seed = 7
        output = "target/custom.wav"
        render_mode = "sub-machine"

        [render]
        sample_rate = 22050
        step_seconds = 0.2
        tail_seconds = 2.0
        stereo_width = 0.6
        delay_mix = 0.2
        delay_feedback = 0.4
        delay_seconds = 0.5
        pump_amount = 0.35
        pump_release = 0.22
        drive = 1.3
        brightness = 1.4
        roughness = 0.7
        sustain = 1.8

        [[section]]
        name = "rupture"
        start = 8
        end = 16

        [[section_render]]
        section = "rupture"
        preset = "static-ash"
        stereo_width = 0.9
        drive = 1.7
        brightness = 1.8
        roughness = 2.2
        sustain = 0.6

        [[voice_render]]
        voice = 1
        preset = "glass-insects"
        stereo_width = 0.3
        drive = 1.1
        brightness = 0.9
        roughness = 1.5
        sustain = 2.1
        "#,
    )
    .unwrap();

    assert_eq!(config.preset, PiecePreset::SlowNoiseBlocks);
    assert_eq!(config.seed, 7);
    assert_eq!(config.output, std::path::PathBuf::from("target/custom.wav"));
    assert_eq!(config.render.mode, RenderMode::SubMachine);
    assert_eq!(config.render.sample_rate, 22_050);
    assert_eq!(config.render.step_seconds, 0.2);
    assert_eq!(config.render.tail_seconds, 2.0);
    assert_eq!(config.render.stereo_width, 0.6);
    assert_eq!(config.render.delay_mix, 0.2);
    assert_eq!(config.render.delay_feedback, 0.4);
    assert_eq!(config.render.delay_seconds, 0.5);
    assert_eq!(config.render.pump_amount, 0.35);
    assert_eq!(config.render.pump_release, 0.22);
    assert_eq!(config.render.drive, 1.3);
    assert_eq!(config.render.brightness, 1.4);
    assert_eq!(config.render.roughness, 0.7);
    assert_eq!(config.render.sustain, 1.8);
    assert_eq!(config.section_renders.len(), 1);
    assert_eq!(config.section_renders[0].section, "rupture");
    assert_eq!(
        config.section_renders[0].preset.as_deref(),
        Some("static-ash")
    );
    assert_eq!(config.section_renders[0].mode, None);
    assert_eq!(config.section_renders[0].stereo_width, Some(0.9));
    assert_eq!(config.section_renders[0].drive, Some(1.7));
    assert_eq!(config.section_renders[0].brightness, Some(1.8));
    assert_eq!(config.section_renders[0].roughness, Some(2.2));
    assert_eq!(config.section_renders[0].sustain, Some(0.6));
    assert_eq!(config.voice_renders.len(), 1);
    assert_eq!(config.voice_renders[0].voice, 1);
    assert_eq!(
        config.voice_renders[0].preset.as_deref(),
        Some("glass-insects")
    );
    assert_eq!(config.voice_renders[0].mode, None);
    assert_eq!(config.voice_renders[0].stereo_width, Some(0.3));
    assert_eq!(config.voice_renders[0].drive, Some(1.1));
    assert_eq!(config.voice_renders[0].brightness, Some(0.9));
    assert_eq!(config.voice_renders[0].roughness, Some(1.5));
    assert_eq!(config.voice_renders[0].sustain, Some(2.1));
}

#[test]
fn generate_one_writes_wav_and_metadata() {
    let base = std::env::temp_dir().join("afruglari-generate-one-test");
    let wav = base.with_extension("wav");
    let json = base.with_extension("json");
    let config = afruglari::GenerationConfig {
        preset: PiecePreset::Example,
        piece: None,
        sections: Vec::new(),
        section_renders: Vec::new(),
        voice_renders: Vec::new(),
        constraints: Vec::new(),
        seed: 3,
        output: wav.clone(),
        render: RenderConfig {
            sample_rate: 8_000,
            step_seconds: 0.04,
            tail_seconds: 0.04,
            mode: RenderMode::Percussive,
            ..RenderConfig::default()
        },
    };

    let result = generate_one(&config).unwrap();

    assert!(wav.exists());
    assert!(json.exists());
    assert_eq!(result.metadata.json_path(), json);
    assert_eq!(result.metadata.preset, PiecePreset::Example);
    assert_eq!(result.metadata.seed, 3);
    assert_eq!(result.metadata.events, 28);

    let metadata = std::fs::read_to_string(&json).unwrap();
    assert!(metadata.contains(r#""preset": "example""#));
    assert!(metadata.contains(r#""events": 28"#));

    let _ = std::fs::remove_file(wav);
    let _ = std::fs::remove_file(json);
}

#[test]
fn custom_config_constraints_generate_a_piece() {
    let base = std::env::temp_dir().join("afruglari-custom-constraint-test");
    let wav = base.with_extension("wav");
    let json = base.with_extension("json");
    let source = format!(
        r#"
        [piece]
        voices = 3
        steps = 16
        registers = 4
        timbres = 6
        intensities = 5
        seed = 5
        output = "{}"
        render_mode = "noise-organ"

        [render]
        sample_rate = 8000
        step_seconds = 0.04
        tail_seconds = 0.04

        [[constraint]]
        type = "max-run"
        voice = 0
        param = "active"
        len = 2

        [[constraint]]
        type = "exact-count"
        param = "active"
        value = true
        density = 0.875

        [[constraint]]
        type = "min-density-window"
        param = "active"
        window = 6
        min = 1

        [[constraint]]
        type = "at-least-collisions"
        voice_a = 1
        voice_b = 2
        count = 2

        [[constraint]]
        type = "slow-change"
        voice = 2
        param = "timbre"
        window = 4
        "#,
        wav.display()
    );
    let config = afruglari::GenerationConfig::parse(&source).unwrap();

    assert_eq!(
        config.piece,
        Some(PieceConfig {
            voices: 3,
            steps: 16,
            registers: 4,
            timbres: 6,
            intensities: 5,
            sections: Vec::new(),
        })
    );
    assert_eq!(config.constraints.len(), 5);

    let result = generate_one(&config).unwrap();

    assert!(wav.exists());
    assert!(json.exists());
    assert_eq!(result.metadata.piece, "custom");
    assert_eq!(result.metadata.events, 42);
    assert!(result.metadata.collisions >= 2);

    let metadata = std::fs::read_to_string(&json).unwrap();
    assert!(metadata.contains(r#""preset": "custom""#));
    assert!(metadata.contains(r#""events": 42"#));

    let _ = std::fs::remove_file(wav);
    let _ = std::fs::remove_file(json);
}

#[test]
fn metadata_records_render_automation() {
    let base = std::env::temp_dir().join("afruglari-render-metadata-test");
    let wav = base.with_extension("wav");
    let json = base.with_extension("json");
    let source = format!(
        r#"
        [piece]
        voices = 2
        steps = 10
        seed = 5
        output = "{}"
        render_mode = "sub-machine"

        [render]
        sample_rate = 8000
        step_seconds = 0.02
        tail_seconds = 0.02

        [[section]]
        name = "break"
        start = 5
        end = 10

        [[voice_render]]
        voice = 0
        mode = "noise-organ"
        brightness = 0.7

        [[section_render]]
        section = "break"
        mode = "granular-dust"
        roughness = 2.2

        [[constraint]]
        type = "exact-count"
        param = "active"
        value = true
        count = 6
        "#,
        wav.display()
    );
    let config = afruglari::GenerationConfig::parse(&source).unwrap();
    let result = generate_one(&config).unwrap();

    assert_eq!(result.metadata.voice_render_count, 1);
    assert_eq!(result.metadata.section_render_count, 1);
    assert!(result.metadata.voice_renders[0].contains("voice 0"));
    assert!(result.metadata.section_renders[0].contains("section break"));

    let metadata = std::fs::read_to_string(&json).unwrap();
    assert!(metadata.contains(r#""voice_render_count": 1"#));
    assert!(metadata.contains(r#""section_render_count": 1"#));
    assert!(metadata.contains("noise-organ"));
    assert!(metadata.contains("granular-dust"));

    let parsed = afruglari::GenerationMetadata::parse_json(&metadata).unwrap();
    assert_eq!(parsed.voice_render_count, 1);
    assert_eq!(parsed.section_render_count, 1);

    let _ = std::fs::remove_file(wav);
    let _ = std::fs::remove_file(json);
}

#[test]
fn batch_from_config_renders_multiple_seeded_versions() {
    let base = std::env::temp_dir().join("afruglari-batch-config-test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let config_path = base.join("tiny-piece.toml");
    std::fs::write(
        &config_path,
        r#"
        [piece]
        voices = 2
        steps = 8
        seed = 20
        render_mode = "metallic"

        [render]
        sample_rate = 8000
        step_seconds = 0.02
        tail_seconds = 0.02

        [[constraint]]
        type = "exact-count"
        param = "active"
        value = true
        count = 5
        "#,
    )
    .unwrap();
    let output_dir = base.join("renders");

    let results = generate_batch_from_config(&config_path, 2, &output_dir).unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].metadata.seed, 20);
    assert_eq!(results[1].metadata.seed, 21);
    assert!(output_dir.join("tiny-piece-seed-020.wav").exists());
    assert!(output_dir.join("tiny-piece-seed-021.wav").exists());
    assert!(output_dir.join("tiny-piece-seed-020.json").exists());
    assert!(output_dir.join("tiny-piece-seed-021.json").exists());

    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn config_supports_new_compositional_constraints() {
    let base = std::env::temp_dir().join("afruglari-new-constraints-test");
    let wav = base.with_extension("wav");
    let json = base.with_extension("json");
    let source = format!(
        r#"
        [piece]
        voices = 3
        steps = 16
        seed = 8
        output = "{}"
        render_mode = "broken-radio"

        [render]
        sample_rate = 8000
        step_seconds = 0.03
        tail_seconds = 0.03

        [[constraint]]
        type = "min-count"
        voice = 2
        param = "active"
        value = true
        count = 6

        [[constraint]]
        type = "max-count"
        voice = 0
        param = "active"
        value = true
        count = 12

        [[constraint]]
        type = "exact-count"
        param = "active"
        value = true
        density = 0.75

        [[constraint]]
        type = "different-adjacent"
        voice = 1
        param = "timbre"

        [[constraint]]
        type = "anti-repeat-window"
        voice = 1
        param = "timbre"
        window = 4
        max_repeats = 3

        [[constraint]]
        type = "phase-response"
        voice_a = 0
        voice_b = 2
        offset = 2
        min = 2
        "#,
        wav.display()
    );
    let config = afruglari::GenerationConfig::parse(&source).unwrap();

    assert_eq!(config.constraints.len(), 6);

    let result = generate_one(&config).unwrap();

    assert!(wav.exists());
    assert!(json.exists());
    assert_eq!(result.metadata.events, 36);
    assert!(result.metadata.voice_density[2] >= 6);
    assert!(result.metadata.voice_density[0] <= 12);

    let _ = std::fs::remove_file(wav);
    let _ = std::fs::remove_file(json);
}

#[test]
fn section_constraints_scope_counts_to_named_ranges() {
    let base = std::env::temp_dir().join("afruglari-section-test");
    let wav = base.with_extension("wav");
    let json = base.with_extension("json");
    let source = format!(
        r#"
        [piece]
        voices = 2
        steps = 12
        seed = 4
        output = "{}"
        render_mode = "percussive"

        [render]
        sample_rate = 8000
        step_seconds = 0.02
        tail_seconds = 0.02

        [[section]]
        name = "opening"
        start = 0
        end = 6

        [[section]]
        name = "ending"
        start = 6
        end = 12

        [[constraint]]
        type = "exact-count"
        section = "opening"
        param = "active"
        value = true
        count = 2

        [[constraint]]
        type = "exact-count"
        section = "ending"
        param = "active"
        value = true
        count = 8
        "#,
        wav.display()
    );
    let config = afruglari::GenerationConfig::parse(&source).unwrap();

    assert_eq!(config.sections.len(), 2);
    let result = generate_one(&config).unwrap();

    assert_eq!(result.metadata.events, 10);
    assert!(wav.exists());
    assert!(json.exists());

    let _ = std::fs::remove_file(wav);
    let _ = std::fs::remove_file(json);
}

#[test]
fn section_render_settings_generate_with_section_automation() {
    let base = std::env::temp_dir().join("afruglari-section-render-test");
    let wav = base.with_extension("wav");
    let json = base.with_extension("json");
    let source = format!(
        r#"
        [piece]
        voices = 2
        steps = 12
        seed = 11
        output = "{}"
        render_mode = "drone"

        [render]
        sample_rate = 8000
        step_seconds = 0.03
        tail_seconds = 0.04
        brightness = 0.8
        roughness = 0.6
        sustain = 1.8

        [[section]]
        name = "opening"
        start = 0
        end = 6

        [[section]]
        name = "rupture"
        start = 6
        end = 12

        [[section_render]]
        section = "rupture"
        preset = "radio-wound"
        stereo_width = 0.9
        roughness = 2.4

        [[voice_render]]
        voice = 0
        preset = "glass-insects"
        roughness = 1.1

        [[voice_render]]
        voice = 1
        preset = "metal-splinters"
        roughness = 1.8

        [[constraint]]
        type = "exact-count"
        param = "active"
        value = true
        count = 8
        "#,
        wav.display()
    );
    let config = afruglari::GenerationConfig::parse(&source).unwrap();

    assert_eq!(config.section_renders.len(), 1);
    assert_eq!(config.voice_renders.len(), 2);
    let result = generate_one(&config).unwrap();

    assert_eq!(result.metadata.events, 8);
    assert!(wav.exists());
    assert!(json.exists());

    let _ = std::fs::remove_file(wav);
    let _ = std::fs::remove_file(json);
}

#[test]
fn exact_count_density_scales_with_scope() {
    let config = afruglari::GenerationConfig::parse(
        r#"
        [piece]
        voices = 3
        steps = 16

        [[constraint]]
        type = "exact-count"
        param = "active"
        value = true
        density = 0.875
        "#,
    )
    .unwrap();

    assert_eq!(
        config.constraints[0].fields.get("density").unwrap(),
        "0.875"
    );

    let base = std::env::temp_dir().join("afruglari-density-test");
    let config = afruglari::GenerationConfig {
        output: base.with_extension("wav"),
        render: RenderConfig {
            sample_rate: 8_000,
            step_seconds: 0.02,
            tail_seconds: 0.02,
            mode: RenderMode::Percussive,
            ..RenderConfig::default()
        },
        ..config
    };
    let result = generate_one(&config).unwrap();

    assert_eq!(result.metadata.events, 42);

    let _ = std::fs::remove_file(config.output);
    let _ = std::fs::remove_file(base.with_extension("json"));
}

#[test]
fn scan_metadata_filters_generated_json_files() {
    let dir = std::env::temp_dir().join("afruglari-scan-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let first = dir.join("first.json");
    let second = dir.join("second.json");
    std::fs::write(
        &first,
        r#"{
  "preset": "custom",
  "seed": 1,
  "render_mode": "noise-organ",
  "output": "first.wav",
  "events": 20,
  "collisions": 3,
  "voice_density": [9, 8, 3]
}
"#,
    )
    .unwrap();
    std::fs::write(
        &second,
        r#"{
  "preset": "custom",
  "seed": 2,
  "render_mode": "noise-organ",
  "output": "second.wav",
  "events": 24,
  "collisions": 7,
  "voice_density": [5, 6, 13]
}
"#,
    )
    .unwrap();

    let entries = scan_metadata(
        &dir,
        &afruglari::MetadataFilter {
            min_collisions: Some(5),
            voice_dominates: Some(2),
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].metadata.seed, 2);
    assert_eq!(entries[0].metadata.events, 24);
    assert_eq!(entries[0].metadata.voice_density, vec![5, 6, 13]);

    let _ = std::fs::remove_dir_all(dir);
}
