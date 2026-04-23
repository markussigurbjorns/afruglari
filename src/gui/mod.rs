use std::error::Error;
use std::fs;
use std::path::{Component, Path, PathBuf};

use eframe::egui::{
    self, Button, CentralPanel, Checkbox, ComboBox, DragValue, Key, Panel, RichText, ScrollArea,
    Slider, TextEdit,
};

use crate::audio::engine::{AudioEngine, LiveAudioHandle, LiveTransportSnapshot};
use crate::cli::AppConfig;
use crate::composition::arrangement::{
    Arrangement, ArrangementSection, InstrumentInstanceSpec, SampleAssetSpec, SampleTriggerEvent,
    SectionMode,
};
use crate::composition::garden::GardenControls;
use crate::composition::timeline::TimelineState;
use crate::instruments::InstrumentFamily;
use crate::instruments::sampler::{LoadedSample, LoadedSampleAsset};

pub fn run_gui(config: &AppConfig) -> Result<(), Box<dyn Error>> {
    let app = ComposerApp::new(config);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Drone Garden Composer")
            .with_inner_size([1320.0, 860.0])
            .with_min_inner_size([960.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Drone Garden Composer",
        options,
        Box::new(move |_cc| Ok(Box::new(app))),
    )?;
    Ok(())
}

struct ComposerApp {
    arrangement: Option<Arrangement>,
    arrangement_path: Option<PathBuf>,
    source_label: String,
    save_path_input: String,
    blank_section_state: TimelineState,
    project_template: ProjectTemplate,
    sample_assets: Vec<LoadedSampleAsset>,
    new_instrument_id: String,
    new_instrument_family: InstrumentFamily,
    sample_library_files: Vec<PathBuf>,
    selected_library_file: Option<usize>,
    new_asset_name: String,
    selected_section: usize,
    selected_trigger: Option<usize>,
    loop_selected_section: bool,
    dirty: bool,
    status_message: String,
    live_handle: Option<LiveAudioHandle>,
    _audio_engine: Option<AudioEngine>,
}

impl ComposerApp {
    fn new(config: &AppConfig) -> Self {
        let blank_section_state = blank_state(config);
        let arrangement = config
            .timeline
            .as_ref()
            .and_then(|timeline| timeline.arrangement.as_ref())
            .cloned()
            .unwrap_or_else(blank_arrangement);
        let arrangement_path = config
            .timeline
            .as_ref()
            .map(|timeline| timeline.path.clone());
        let source_label = config.timeline.as_ref().map_or_else(
            || String::from("unsaved arrangement"),
            |timeline| format!("source: {}", timeline.path.display()),
        );
        let save_path_input = arrangement_path.as_ref().map_or_else(
            || String::from("timelines/untitled.arrangement"),
            |path| path.display().to_string(),
        );
        let sample_assets = config.garden.sample_assets.clone();
        let sample_library_files = scan_sample_library();
        let selected_library_file = (!sample_library_files.is_empty()).then_some(0);
        let new_asset_name = selected_library_file
            .and_then(|index| sample_library_files.get(index))
            .map_or_else(String::new, |path| default_asset_name(path));
        let new_instrument_id = String::from("drone_alt");

        let (audio_engine, live_handle, status_message) =
            match AudioEngine::start_live(config.garden.clone(), arrangement.clone()) {
                Ok((engine, handle)) => (
                    Some(engine),
                    Some(handle),
                    String::from("Blank project ready"),
                ),
                Err(err) => (None, None, format!("Live audio unavailable: {err}")),
        };

        Self {
            arrangement: Some(arrangement),
            arrangement_path,
            source_label,
            save_path_input,
            blank_section_state,
            project_template: ProjectTemplate::Blank,
            sample_assets,
            new_instrument_id,
            new_instrument_family: InstrumentFamily::Drone,
            sample_library_files,
            selected_library_file,
            new_asset_name,
            selected_section: 0,
            selected_trigger: None,
            loop_selected_section: false,
            dirty: false,
            status_message,
            live_handle,
            _audio_engine: audio_engine,
        }
    }

    fn save_arrangement(&mut self) {
        let Some(path) = self.arrangement_path.clone() else {
            self.status_message = String::from("No path set. Use Save As.");
            return;
        };

        self.save_arrangement_to(path);
    }

    fn save_arrangement_as(&mut self) {
        let path_text = self.save_path_input.trim();
        if path_text.is_empty() {
            self.status_message = String::from("Save path cannot be empty");
            return;
        }

        self.save_arrangement_to(PathBuf::from(path_text));
    }

    fn save_arrangement_to(&mut self, path: PathBuf) {
        let Some(arrangement) = self.arrangement.as_ref() else {
            self.status_message = String::from("Nothing to save");
            return;
        };

        if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            if let Err(err) = fs::create_dir_all(parent) {
                self.status_message = format!("Save failed: {err}");
                return;
            }
        }

        match fs::write(&path, arrangement.to_text()) {
            Ok(()) => {
                self.arrangement_path = Some(path.clone());
                self.save_path_input = path.display().to_string();
                self.sync_source_label();
                self.dirty = false;
                self.status_message = format!("Saved {}", path.display());
            }
            Err(err) => {
                self.status_message = format!("Save failed: {err}");
            }
        }
    }

    fn mark_project_changed(&mut self) {
        if let Some(arrangement) = self.arrangement.as_mut() {
            arrangement.refresh_derived();
            if let Some(handle) = &self.live_handle {
                handle.update_project(arrangement.clone(), self.sample_assets.clone());
            }
        }
        self.dirty = true;
        self.status_message = String::from("Edited in memory");
    }

    fn sync_source_label(&mut self) {
        self.source_label = self.arrangement_path.as_ref().map_or_else(
            || String::from("unsaved arrangement"),
            |path| format!("source: {}", path.display()),
        );
    }

    fn new_project(&mut self) {
        self.arrangement = Some(self.project_template.instantiate(self.blank_section_state));
        self.arrangement_path = None;
        self.save_path_input = String::from("timelines/untitled.arrangement");
        self.sync_source_label();
        self.sample_assets.clear();
        self.new_instrument_id = String::from("drone_alt");
        self.new_instrument_family = InstrumentFamily::Drone;
        self.selected_section = 0;
        self.selected_trigger = None;
        self.loop_selected_section = false;
        self.dirty = false;
        if let Some(handle) = &self.live_handle {
            handle.update_project(
                self.project_template.instantiate(self.blank_section_state),
                Vec::new(),
            );
            handle.set_loop_section(None);
            handle.set_position_seconds(0.0);
        }
        self.status_message = format!("Started new {} project", self.project_template.label());
    }

    fn selected_section_label(&self) -> String {
        self.arrangement
            .as_ref()
            .and_then(|arrangement| arrangement.sections().get(self.selected_section))
            .map_or_else(
                || String::from("No section selected"),
                |section| format!("{}  {:.1}s", section.name, section.duration_seconds),
            )
    }

    fn sync_selection(&mut self) {
        let Some(arrangement) = self.arrangement.as_ref() else {
            self.selected_section = 0;
            self.selected_trigger = None;
            return;
        };
        if arrangement.sections().is_empty() {
            self.selected_section = 0;
            self.selected_trigger = None;
            return;
        }

        self.selected_section = self
            .selected_section
            .min(arrangement.sections().len().saturating_sub(1));
        if let Some(section) = arrangement.sections().get(self.selected_section) {
            self.selected_trigger = self
                .selected_trigger
                .filter(|index| *index < section.sample_triggers.len());
        }
    }

    fn draw_header(&mut self, ui: &mut egui::Ui) {
        Panel::top("header").show_inside(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("Drone Garden Composer");
                if self.dirty {
                    ui.label(RichText::new("edited in memory").strong());
                }
                ComboBox::from_id_salt("project_template")
                    .selected_text(self.project_template.label())
                    .show_ui(ui, |ui| {
                        for template in ProjectTemplate::all() {
                            ui.selectable_value(
                                &mut self.project_template,
                                *template,
                                template.label(),
                            );
                        }
                    });
                if ui.button("New").clicked() {
                    self.new_project();
                }
                if ui.button("Save").clicked() {
                    self.save_arrangement();
                }
                if ui.button("Save As").clicked() {
                    self.save_arrangement_as();
                }
                ui.separator();
                ui.label(&self.source_label);
            });
            ui.horizontal(|ui| {
                ui.label("Path");
                ui.add(
                    TextEdit::singleline(&mut self.save_path_input)
                        .desired_width(320.0)
                        .hint_text("timelines/untitled.arrangement"),
                );
                ui.separator();
                ui.label(format!("Section: {}", self.selected_section_label()));
                ui.separator();
                ui.label(format!("Assets: {}", self.sample_assets.len()));
                ui.separator();
                ui.label(&self.status_message);
            });
            ui.add_space(4.0);
        });
    }

    fn draw_sections_panel(&mut self, ui: &mut egui::Ui) {
        Panel::left("sections")
            .resizable(true)
            .default_size(280.0)
            .show_inside(ui, |ui| {
                ui.heading("Sections");
                ui.separator();

                let mut add_section = false;
                let mut delete_section = false;
                ui.horizontal(|ui| {
                    if ui.button("+ Section").clicked() {
                        add_section = true;
                    }

                    let can_delete = self
                        .arrangement
                        .as_ref()
                        .is_some_and(|arrangement| !arrangement.sections().is_empty());
                    if ui
                        .add_enabled(can_delete, Button::new("Delete"))
                        .clicked()
                    {
                        delete_section = true;
                    }
                });
                ui.separator();

                if add_section {
                    self.add_section();
                }
                if delete_section {
                    self.delete_selected_section();
                }

                let Some(arrangement) = self.arrangement.as_ref() else {
                    ui.label("Project unavailable.");
                    return;
                };
                if arrangement.sections().is_empty() {
                    ui.label("Blank project. Add a section to start composing.");
                    return;
                }

                ScrollArea::vertical().show(ui, |ui| {
                    for (index, section) in arrangement.sections().iter().enumerate() {
                        let selected = index == self.selected_section;
                        let label = format!(
                            "{}\n{:?}  {:.1}s",
                            section.name, section.mode, section.duration_seconds
                        );
                        if ui.selectable_label(selected, label).clicked() {
                            self.selected_section = index;
                            self.selected_trigger = None;
                        }
                    }
                });
            });
    }

    fn draw_detail_panel(&mut self, ui: &mut egui::Ui) {
        CentralPanel::default().show_inside(ui, |ui| {
            if self.arrangement.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.label("Project unavailable.");
                });
                return;
            }

            let mut project_changed = false;
            let mut selected_trigger = self.selected_trigger;
            let live_snapshot = self.live_handle.as_ref().map(|handle| handle.snapshot());
            let loop_selected_section = self.loop_selected_section;
            let mut pending_play_toggle = false;
            let mut pending_restart = None;
            let mut pending_loop = None;

            ScrollArea::vertical().show(ui, |ui| {
                if let Some(arrangement) = self.arrangement.as_mut() {
                    project_changed |= Self::draw_sample_asset_manager(
                        ui,
                        arrangement,
                        &mut self.sample_assets,
                        &mut self.new_instrument_id,
                        &mut self.new_instrument_family,
                        &self.arrangement_path,
                        &self.sample_library_files,
                        &mut self.selected_library_file,
                        &mut self.new_asset_name,
                        &mut self.status_message,
                    );

                    ui.separator();

                    if arrangement.sections().is_empty() {
                        ui.label("Blank project. Use + Section on the left.");
                        return;
                    }

                    self.selected_section = self
                        .selected_section
                        .min(arrangement.sections().len().saturating_sub(1));

                    let instrument_specs = arrangement.instrument_specs().to_vec();
                    let section = &mut arrangement.sections_mut()[self.selected_section];
                    selected_trigger =
                        selected_trigger.filter(|index| *index < section.sample_triggers.len());

                    ui.horizontal(|ui| {
                        ui.heading(&section.name);
                        ui.separator();
                        ui.label(format!(
                            "start {:.1}s   duration {:.1}s",
                            section.start_seconds, section.duration_seconds
                        ));
                    });
                    ui.add_space(8.0);

                    let (transport_play_toggle, transport_restart, transport_loop) =
                        Self::draw_transport(
                            ui,
                            live_snapshot.as_ref(),
                            section,
                            loop_selected_section,
                        );
                    pending_play_toggle = transport_play_toggle;
                    pending_restart = transport_restart;
                    pending_loop = transport_loop;

                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Section mode");
                        project_changed |= ui
                            .selectable_value(&mut section.mode, SectionMode::Hold, "Hold")
                            .changed();
                        project_changed |= ui
                            .selectable_value(&mut section.mode, SectionMode::Ramp, "Ramp")
                            .changed();
                    });

                    ui.separator();
                    ui.columns(2, |columns| {
                        Self::draw_macro_editor(&mut columns[0], section, &mut project_changed);
                        Self::draw_instrument_editor_rows(
                            &mut columns[1],
                            &instrument_specs,
                            section,
                            &mut project_changed,
                        );
                    });

                    ui.separator();
                    let sample_assets = &self.sample_assets;
                    ui.columns(2, |columns| {
                        project_changed |=
                            Self::draw_instrument_parameter_editors(&mut columns[0], section);
                        project_changed |= Self::draw_trigger_editor(
                            &mut columns[1],
                            section,
                            sample_assets,
                            &mut selected_trigger,
                        );
                    });
                }
            });

            self.selected_trigger = selected_trigger;
            if let Some(loop_enabled) = pending_loop {
                self.loop_selected_section = loop_enabled;
            }
            if let Some(handle) = &self.live_handle {
                handle.set_loop_section(self.loop_selected_section.then_some(self.selected_section));
                if pending_play_toggle {
                    handle.toggle_playback();
                }
                if let Some(position_seconds) = pending_restart {
                    handle.set_position_seconds(position_seconds);
                }
            }
            if project_changed {
                self.mark_project_changed();
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_sample_asset_manager(
        ui: &mut egui::Ui,
        arrangement: &mut Arrangement,
        loaded_assets: &mut Vec<LoadedSampleAsset>,
        new_instrument_id: &mut String,
        new_instrument_family: &mut InstrumentFamily,
        arrangement_path: &Option<PathBuf>,
        sample_library_files: &[PathBuf],
        selected_library_file: &mut Option<usize>,
        new_asset_name: &mut String,
        status_message: &mut String,
    ) -> bool {
        let mut changed = false;
        ui.heading("Project Resources");
        ui.columns(3, |columns| {
            columns[0].group(|ui| {
                ui.heading("Instruments");
                ui.label("Named project-side instances. Playback still uses the fixed rack for now.");

                let mut remove_index = None;
                for (index, instrument) in arrangement.instrument_specs().iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&instrument.id).strong());
                            ui.label(instrument.family.label());
                        });
                        if ui.add(Button::new("Remove")).clicked() {
                            remove_index = Some(index);
                        }
                    });
                    ui.separator();
                }

                if let Some(index) = remove_index {
                    let removed = arrangement.instrument_specs_mut().remove(index);
                    *status_message = format!("Removed instrument `{}`", removed.id);
                    changed = true;
                }

                ui.separator();
                ui.add(TextEdit::singleline(new_instrument_id).hint_text("instance id"));
                ComboBox::from_label("Family")
                    .selected_text(new_instrument_family.label())
                    .show_ui(ui, |ui| {
                        for family in InstrumentFamily::all() {
                            ui.selectable_value(new_instrument_family, *family, family.label());
                        }
                    });

                if ui.button("Add instrument").clicked() {
                    let id = sanitize_asset_name(new_instrument_id);
                    if id.is_empty() {
                        *status_message = String::from("Instrument id cannot be empty");
                    } else if arrangement
                        .instrument_specs()
                        .iter()
                        .any(|instrument| instrument.id == id)
                    {
                        *status_message = format!("Instrument `{id}` already exists");
                    } else {
                        arrangement.instrument_specs_mut().push(InstrumentInstanceSpec {
                            id: id.clone(),
                            family: *new_instrument_family,
                        });
                        *new_instrument_id = next_default_instrument_id(*new_instrument_family);
                        *status_message = format!("Added instrument `{id}`");
                        changed = true;
                    }
                }
            });

            columns[1].group(|ui| {
                ui.heading("Project Assets");
                if arrangement.sample_assets().is_empty() {
                    ui.label("No named sample assets yet.");
                } else {
                    let usage_counts = sample_usage_counts(arrangement);
                    let mut remove_index = None;

                    for (index, spec) in arrangement.sample_assets().iter().enumerate() {
                        let duration = loaded_assets
                            .iter()
                            .find(|asset| asset.name() == spec.name)
                            .map_or(0.0, LoadedSampleAsset::duration_seconds);
                        let usage_count = usage_counts
                            .iter()
                            .find_map(|(name, count)| (name == &spec.name).then_some(*count))
                            .unwrap_or(0);

                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&spec.name).strong());
                                ui.label(spec.path.as_str());
                                ui.label(format!(
                                    "used in {} trigger(s), {:.2}s",
                                    usage_count, duration
                                ));
                            });

                            let remove_enabled = usage_count == 0;
                            if ui
                                .add_enabled(remove_enabled, Button::new("Remove"))
                                .clicked()
                            {
                                remove_index = Some(index);
                            }
                        });
                        ui.separator();
                    }

                    if let Some(index) = remove_index {
                        let removed_name = arrangement.sample_assets()[index].name.clone();
                        arrangement.sample_assets_mut().remove(index);
                        if let Some(asset_index) = loaded_assets
                            .iter()
                            .position(|asset| asset.name() == removed_name)
                        {
                            loaded_assets.remove(asset_index);
                        }
                        *status_message = format!("Removed asset `{removed_name}`");
                        changed = true;
                    } else if arrangement.sample_assets().iter().any(|spec| {
                        sample_usage_counts(arrangement)
                            .iter()
                            .any(|(name, count)| name == &spec.name && *count > 0)
                    }) {
                        ui.label("Assets used by triggers must be reassigned before removal.");
                    }
                }
            });

            columns[2].group(|ui| {
                ui.heading("Browser");
                ui.label("WAV files from samples/");

                if sample_library_files.is_empty() {
                    ui.label("No WAV files found in samples/.");
                    return;
                }

                let selected_index = selected_library_file
                    .unwrap_or(0)
                    .min(sample_library_files.len().saturating_sub(1));
                if selected_library_file.is_none() {
                    *selected_library_file = Some(selected_index);
                }

                ComboBox::from_label("Library file")
                    .selected_text(display_library_path(&sample_library_files[selected_index]))
                    .show_ui(ui, |ui| {
                        for (index, path) in sample_library_files.iter().enumerate() {
                            if ui
                                .selectable_value(
                                    selected_library_file,
                                    Some(index),
                                    display_library_path(path),
                                )
                                .changed()
                            {
                                *new_asset_name = default_asset_name(path);
                            }
                        }
                    });

                ui.add(TextEdit::singleline(new_asset_name).hint_text("asset name"));

                if let Some(index) = *selected_library_file {
                    if let Some(path) = sample_library_files.get(index) {
                        ui.label(format!("file: {}", display_library_path(path)));
                        if ui.button("Add asset").clicked() {
                            match Self::add_sample_asset(
                                arrangement,
                                loaded_assets,
                                arrangement_path,
                                path,
                                new_asset_name,
                            ) {
                                Ok(asset_name) => {
                                    *status_message = format!("Added asset `{asset_name}`");
                                    changed = true;
                                }
                                Err(err) => {
                                    *status_message = err;
                                }
                            }
                        }
                    }
                }
            });
        });

        changed
    }

    fn add_sample_asset(
        arrangement: &mut Arrangement,
        loaded_assets: &mut Vec<LoadedSampleAsset>,
        arrangement_path: &Option<PathBuf>,
        file_path: &Path,
        asset_name: &str,
    ) -> Result<String, String> {
        let asset_name = sanitize_asset_name(asset_name);
        if asset_name.is_empty() {
            return Err(String::from("Asset name cannot be empty"));
        }
        if arrangement
            .sample_assets()
            .iter()
            .any(|spec| spec.name == asset_name)
        {
            return Err(format!("Asset `{asset_name}` already exists"));
        }

        let sample =
            LoadedSample::from_wav_path(file_path).map_err(|err| format!("load failed: {err}"))?;
        loaded_assets.push(LoadedSampleAsset::new(asset_name.clone(), sample));

        let arrangement_parent = arrangement_path
            .as_ref()
            .and_then(|path| path.parent())
            .unwrap_or_else(|| Path::new("."));
        let asset_path = make_arrangement_relative_path(arrangement_parent, file_path);
        arrangement.sample_assets_mut().push(SampleAssetSpec {
            name: asset_name.clone(),
            path: asset_path,
        });

        Ok(asset_name)
    }

    fn draw_transport(
        ui: &mut egui::Ui,
        snapshot: Option<&LiveTransportSnapshot>,
        section: &ArrangementSection,
        loop_selected_section: bool,
    ) -> (bool, Option<f32>, Option<bool>) {
        let Some(snapshot) = snapshot else {
            ui.label("Live audio unavailable");
            return (false, None, None);
        };

        let mut play_toggle = false;
        let mut restart = None;
        let mut loop_enabled = loop_selected_section;

        ui.horizontal(|ui| {
            let play_label = if snapshot.playback { "Stop" } else { "Play" };
            if ui.add(Button::new(play_label)).clicked() {
                play_toggle = true;
            }
            if ui.button("Restart").clicked() {
                restart = Some(if loop_selected_section {
                    section.start_seconds
                } else {
                    0.0
                });
            }
            ui.add(Checkbox::new(&mut loop_enabled, "Loop section"));
            ui.separator();
            ui.label(format!("time {:.2}s", snapshot.position_seconds));
            if snapshot.loop_section.is_some() {
                ui.label("looping");
            }
        });

        (play_toggle, restart, Some(loop_enabled))
    }

    fn draw_macro_editor(
        ui: &mut egui::Ui,
        section: &mut ArrangementSection,
        project_changed: &mut bool,
    ) {
        ui.heading("Macros");
        edit_slider(
            ui,
            "Density",
            &mut section.state.controls.density,
            0.0..=1.0,
            project_changed,
        );
        edit_slider(
            ui,
            "Brightness",
            &mut section.state.controls.brightness,
            0.0..=1.0,
            project_changed,
        );
        edit_slider(
            ui,
            "Space",
            &mut section.state.controls.space,
            0.0..=1.0,
            project_changed,
        );
        edit_slider(
            ui,
            "Instability",
            &mut section.state.controls.instability,
            0.0..=1.0,
            project_changed,
        );
        edit_slider(
            ui,
            "Drone",
            &mut section.state.controls.drone_level,
            0.0..=1.0,
            project_changed,
        );
        edit_slider(
            ui,
            "Harmonic",
            &mut section.state.controls.harmonic_level,
            0.0..=1.0,
            project_changed,
        );
        edit_slider(
            ui,
            "Pulse",
            &mut section.state.controls.pulse_level,
            0.0..=1.0,
            project_changed,
        );
        edit_slider(
            ui,
            "Sample",
            &mut section.state.controls.sample_level,
            0.0..=1.0,
            project_changed,
        );
        edit_slider(
            ui,
            "Noise",
            &mut section.state.controls.noise_level,
            0.0..=1.0,
            project_changed,
        );
        edit_slider(
            ui,
            "Events",
            &mut section.state.controls.event_level,
            0.0..=1.0,
            project_changed,
        );
        edit_slider(
            ui,
            "Texture",
            &mut section.state.controls.texture_level,
            0.0..=1.0,
            project_changed,
        );
    }

    fn draw_instrument_editor_rows(
        ui: &mut egui::Ui,
        instrument_specs: &[InstrumentInstanceSpec],
        section: &mut ArrangementSection,
        project_changed: &mut bool,
    ) {
        ui.heading("Instruments");
        ui.label("Section entries target named project instances. Playback still collapses them by family.");
        ui.separator();

        for instrument in instrument_specs {
            Self::draw_instrument_row(ui, instrument, section, project_changed);
            ui.separator();
        }
    }

    fn draw_instrument_row(
        ui: &mut egui::Ui,
        instrument: &InstrumentInstanceSpec,
        section: &mut ArrangementSection,
        project_changed: &mut bool,
    ) {
        let family = instrument.family;
        let label = family.label();
        let supports_active = family.supports_active();
        let supports_override = family.supports_override();

        ui.vertical(|ui| {
            ui.label(RichText::new(&instrument.id).strong());
            ui.label(label);

            if supports_active {
                let active = instrument_active_mut(section, family);
                if ui.checkbox(active, "Active").changed() {
                    *project_changed = true;
                }
            } else {
                ui.label("Always active");
            }

            let level = instrument_level_mut(section, family);
            if ui
                .add(Slider::new(level, 0.0..=1.0).text("Level"))
                .changed()
            {
                *project_changed = true;
            }

            if supports_override {
                let level_value = *instrument_level_mut(section, family);
                let level_override = instrument_override_mut(section, family);
                let mut override_enabled = level_override.is_some();
                if ui.checkbox(&mut override_enabled, "Override").changed() {
                    if override_enabled {
                        *level_override = Some(level_value);
                    } else {
                        *level_override = None;
                    }
                    *project_changed = true;
                }

                if let Some(value) = level_override.as_mut() {
                    if ui
                        .add(Slider::new(value, 0.0..=1.0).text("Override level"))
                        .changed()
                    {
                        *project_changed = true;
                    }
                }
            }
        });

        if *project_changed {
            sync_instrument_entry(section, instrument);
        }
    }

    fn draw_instrument_parameter_editors(
        ui: &mut egui::Ui,
        section: &mut ArrangementSection,
    ) -> bool {
        let mut project_changed = false;

        ui.heading("Instrument Params");
        ui.label("Current engine-backed parameters for this section.");
        ui.separator();

        ui.label(RichText::new("Pitch Field").strong());
        ui.horizontal(|ui| {
            ui.label("Root");
            if ui
                .add(
                    DragValue::new(&mut section.state.root_hz)
                        .speed(0.1)
                        .range(20.0..=880.0),
                )
                .changed()
            {
                project_changed = true;
            }
            ui.label("Hz");
        });
        ui.horizontal(|ui| {
            ui.label("Voices");
            let mut voices = section.state.voice_count as i32;
            if ui.add(DragValue::new(&mut voices).range(1..=12)).changed() {
                section.state.voice_count = voices.clamp(1, 12) as usize;
                project_changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Octave min");
            let mut octave_min = section.state.octave_min;
            if ui
                .add(DragValue::new(&mut octave_min).range(-2..=8))
                .changed()
            {
                section.state.octave_min = octave_min;
                if section.state.octave_max < section.state.octave_min {
                    section.state.octave_max = section.state.octave_min;
                }
                project_changed = true;
            }
            ui.label("Octave max");
            let mut octave_max = section.state.octave_max;
            if ui
                .add(DragValue::new(&mut octave_max).range(-2..=8))
                .changed()
            {
                section.state.octave_max = octave_max.max(section.state.octave_min);
                project_changed = true;
            }
        });

        ui.separator();
        ui.label(RichText::new("Drone / Harmonic").strong());
        ui.horizontal(|ui| {
            ui.label("Retune");
            if ui
                .add(
                    DragValue::new(&mut section.state.drone_retune_seconds)
                        .speed(0.1)
                        .range(0.25..=60.0),
                )
                .changed()
            {
                project_changed = true;
            }
            ui.label("s");
        });
        ui.horizontal(|ui| {
            ui.label("Drone spread");
            if ui
                .add(
                    DragValue::new(&mut section.state.instrument_params.drone_mut().spread)
                        .speed(0.05)
                        .range(0.0..=2.0),
                )
                .changed()
            {
                project_changed = true;
            }
            ui.label("Drone detune");
            if ui
                .add(
                    DragValue::new(&mut section.state.instrument_params.drone_mut().detune)
                        .speed(0.05)
                        .range(0.0..=2.0),
                )
                .changed()
            {
                project_changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Harmonic mix");
            if ui
                .add(
                    DragValue::new(&mut section.state.instrument_params.harmonic_mut().mix)
                        .speed(0.05)
                        .range(0.0..=2.0),
                )
                .changed()
            {
                project_changed = true;
            }
            ui.label("Harmonic shimmer");
            if ui
                .add(
                    DragValue::new(&mut section.state.instrument_params.harmonic_mut().shimmer)
                        .speed(0.05)
                        .range(0.0..=2.0),
                )
                .changed()
            {
                project_changed = true;
            }
        });

        ui.separator();
        ui.label(RichText::new("Pulse").strong());
        ui.horizontal(|ui| {
            ui.label("Rate");
            if ui
                .add(
                    DragValue::new(&mut section.state.instrument_params.pulse_mut().rate)
                        .speed(0.05)
                        .range(0.25..=4.0),
                )
                .changed()
            {
                project_changed = true;
            }
            ui.label("Length");
            if ui
                .add(
                    DragValue::new(&mut section.state.instrument_params.pulse_mut().length)
                        .speed(0.05)
                        .range(0.25..=4.0),
                )
                .changed()
            {
                project_changed = true;
            }
        });

        ui.separator();
        ui.label(RichText::new("Events").strong());
        ui.horizontal(|ui| {
            ui.label("Attack min");
            if ui
                .add(
                    DragValue::new(&mut section.state.event_attack_min)
                        .speed(0.005)
                        .range(0.001..=8.0),
                )
                .changed()
            {
                section.state.event_attack_max = section
                    .state
                    .event_attack_max
                    .max(section.state.event_attack_min);
                project_changed = true;
            }
            ui.label("Attack max");
            if ui
                .add(
                    DragValue::new(&mut section.state.event_attack_max)
                        .speed(0.005)
                        .range(0.001..=8.0),
                )
                .changed()
            {
                section.state.event_attack_max = section
                    .state
                    .event_attack_max
                    .max(section.state.event_attack_min);
                project_changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Decay min");
            if ui
                .add(
                    DragValue::new(&mut section.state.event_decay_min)
                        .speed(0.05)
                        .range(0.05..=30.0),
                )
                .changed()
            {
                section.state.event_decay_max = section
                    .state
                    .event_decay_max
                    .max(section.state.event_decay_min);
                project_changed = true;
            }
            ui.label("Decay max");
            if ui
                .add(
                    DragValue::new(&mut section.state.event_decay_max)
                        .speed(0.05)
                        .range(0.05..=30.0),
                )
                .changed()
            {
                section.state.event_decay_max = section
                    .state
                    .event_decay_max
                    .max(section.state.event_decay_min);
                project_changed = true;
            }
        });

        ui.separator();
        ui.label(RichText::new("Noise / Sample / Texture").strong());
        ui.horizontal(|ui| {
            ui.label("Noise motion");
            if ui
                .add(
                    DragValue::new(&mut section.state.instrument_params.noise_mut().motion)
                        .speed(0.05)
                        .range(0.0..=2.0),
                )
                .changed()
            {
                project_changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Sample auto-rate");
            if ui
                .add(
                    DragValue::new(&mut section.state.instrument_params.sample_mut().auto_rate)
                        .speed(0.05)
                        .range(0.25..=4.0),
                )
                .changed()
            {
                project_changed = true;
            }
            ui.label("Texture drift");
            if ui
                .add(
                    DragValue::new(&mut section.state.instrument_params.texture_mut().drift)
                        .speed(0.05)
                        .range(0.0..=2.0),
                )
                .changed()
            {
                project_changed = true;
            }
        });

        project_changed
    }

    fn draw_trigger_editor(
        ui: &mut egui::Ui,
        section: &mut ArrangementSection,
        sample_assets: &[LoadedSampleAsset],
        selected_trigger: &mut Option<usize>,
    ) -> bool {
        let mut project_changed = false;
        ui.heading("Sample Triggers");
        ui.label(format!("assets loaded: {}", sample_assets.len()));

        ui.horizontal(|ui| {
            if ui.button("+ Trigger").clicked() {
                section
                    .sample_triggers
                    .push(default_trigger(section, sample_assets));
                *selected_trigger = Some(section.sample_triggers.len() - 1);
                project_changed = true;
            }
            let delete_enabled = selected_trigger
                .map(|index| index < section.sample_triggers.len())
                .unwrap_or(false);
            if ui
                .add_enabled(delete_enabled, Button::new("Delete"))
                .clicked()
            {
                if let Some(index) = *selected_trigger {
                    section.sample_triggers.remove(index);
                    *selected_trigger = None;
                    project_changed = true;
                }
            }
        });

        ui.separator();
        for (index, trigger) in section.sample_triggers.iter().enumerate() {
            let selected = *selected_trigger == Some(index);
            let row_text = format!(
                "{} @ {:.2}s  st={:?}  ct={:?}",
                trigger.sample_name,
                (trigger.time_seconds - section.start_seconds).max(0.0),
                trigger.semitones,
                trigger.cents
            );
            if ui.selectable_label(selected, row_text).clicked() {
                *selected_trigger = Some(index);
            }
        }

        let Some(index) = selected_trigger.filter(|index| *index < section.sample_triggers.len())
        else {
            ui.add_space(8.0);
            ui.label("Select a trigger row to edit it.");
            return project_changed;
        };

        let asset_names = sample_name_list(sample_assets);
        let trigger = &mut section.sample_triggers[index];
        let trim_max = current_asset_duration(sample_assets, &trigger.sample_name).max(0.1);
        let local_max = section.duration_seconds.max(0.1);

        ui.separator();
        ui.heading("Trigger Editor");
        ComboBox::from_label("Sample asset")
            .selected_text(trigger.sample_name.clone())
            .show_ui(ui, |ui| {
                for asset_name in &asset_names {
                    if ui
                        .selectable_value(&mut trigger.sample_name, asset_name.clone(), asset_name)
                        .changed()
                    {
                        project_changed = true;
                    }
                }
            });

        let mut local_time = (trigger.time_seconds - section.start_seconds).max(0.0);
        if ui
            .add(Slider::new(&mut local_time, 0.0..=local_max).text("At"))
            .changed()
        {
            trigger.time_seconds = section.start_seconds + local_time;
            project_changed = true;
        }

        let mut start_value = trigger.start_seconds.unwrap_or(0.0);
        if ui
            .add(Slider::new(&mut start_value, 0.0..=trim_max).text("Start"))
            .changed()
        {
            trigger.start_seconds = Some(start_value);
            project_changed = true;
        }

        let mut end_value = trigger
            .end_seconds
            .unwrap_or(trim_max.min(start_value + 0.25));
        if ui
            .add(Slider::new(&mut end_value, start_value..=trim_max).text("End"))
            .changed()
        {
            trigger.end_seconds = Some(end_value.max(start_value + 0.001));
            project_changed = true;
        }

        let fade_limit = (trigger.end_seconds.unwrap_or(trim_max) - start_value).max(0.05);
        let mut fade_in = trigger.fade_in_seconds.unwrap_or(0.0);
        if ui
            .add(Slider::new(&mut fade_in, 0.0..=fade_limit).text("Fade In"))
            .changed()
        {
            trigger.fade_in_seconds = Some(fade_in);
            project_changed = true;
        }

        let mut fade_out = trigger.fade_out_seconds.unwrap_or(0.0);
        if ui
            .add(Slider::new(&mut fade_out, 0.0..=fade_limit).text("Fade Out"))
            .changed()
        {
            trigger.fade_out_seconds = Some(fade_out);
            project_changed = true;
        }

        let mut semitones = trigger.semitones.unwrap_or(0.0);
        if ui
            .add(Slider::new(&mut semitones, -24.0..=24.0).text("Semitones"))
            .changed()
        {
            trigger.semitones = Some(semitones);
            project_changed = true;
        }

        let mut cents = trigger.cents.unwrap_or(0.0);
        if ui
            .add(Slider::new(&mut cents, -100.0..=100.0).text("Cents"))
            .changed()
        {
            trigger.cents = Some(cents);
            project_changed = true;
        }

        let mut gain = trigger.gain.unwrap_or(1.0);
        if ui
            .add(Slider::new(&mut gain, 0.0..=1.0).text("Gain"))
            .changed()
        {
            trigger.gain = Some(gain);
            project_changed = true;
        }

        let mut pan = trigger.pan.unwrap_or(0.0);
        if ui
            .add(Slider::new(&mut pan, -1.0..=1.0).text("Pan"))
            .changed()
        {
            trigger.pan = Some(pan);
            project_changed = true;
        }

        let mut rate = trigger.rate.unwrap_or(1.0);
        if ui
            .add(Slider::new(&mut rate, 0.25..=2.0).text("Rate"))
            .changed()
        {
            trigger.rate = Some(rate);
            project_changed = true;
        }

        project_changed
    }

    fn add_section(&mut self) {
        let blank_section_state = self.blank_section_state;
        let Some(arrangement) = self.arrangement.as_mut() else {
            return;
        };

        let new_section = arrangement
            .sections()
            .last()
            .map(next_section_from)
            .unwrap_or_else(|| default_blank_section(blank_section_state));
        arrangement.push_section(new_section);
        self.selected_section = arrangement.sections().len().saturating_sub(1);
        self.selected_trigger = None;
        self.mark_project_changed();
        self.status_message = format!("Added {}", self.selected_section_label());
    }

    fn delete_selected_section(&mut self) {
        let Some(arrangement) = self.arrangement.as_mut() else {
            return;
        };
        if arrangement.sections().is_empty() {
            return;
        }

        let index = self
            .selected_section
            .min(arrangement.sections().len().saturating_sub(1));
        let removed_name = arrangement.remove_section(index).name;
        self.selected_section = index.saturating_sub(1);
        self.selected_trigger = None;
        self.mark_project_changed();
        self.status_message = format!("Removed section `{removed_name}`");
    }
}

impl eframe::App for ComposerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|input| input.key_pressed(Key::S) && input.modifiers.command) {
            if ctx.input(|input| input.modifiers.shift) {
                self.save_arrangement_as();
            } else {
                self.save_arrangement();
            }
        }
        self.sync_selection();
        self.draw_header(ui);
        self.draw_sections_panel(ui);
        self.draw_detail_panel(ui);
    }
}

fn edit_slider(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    project_changed: &mut bool,
) {
    if ui.add(Slider::new(value, range).text(label)).changed() {
        *project_changed = true;
    }
}

fn instrument_level_mut(section: &mut ArrangementSection, family: InstrumentFamily) -> &mut f32 {
    section.state.controls.level_mut(family)
}

fn instrument_active_mut(section: &mut ArrangementSection, family: InstrumentFamily) -> &mut bool {
    debug_assert!(family.supports_active());
    section.state.active_mut(family)
}

fn instrument_override_mut(
    section: &mut ArrangementSection,
    family: InstrumentFamily,
) -> &mut Option<f32> {
    debug_assert!(family.supports_override());
    section.state.level_override_mut(family)
}

fn sync_instrument_entry(
    section: &mut ArrangementSection,
    instrument: &InstrumentInstanceSpec,
) {
    let index = if let Some(index) = section
        .instrument_entries
        .iter()
        .position(|entry| entry.target_id.as_deref() == Some(instrument.id.as_str()))
    {
        index
    } else {
        section
            .instrument_entries
            .push(crate::composition::arrangement::ArrangementInstrumentEntry {
                target_id: Some(instrument.id.clone()),
                family: instrument.family,
                level: None,
                active: None,
                level_override: None,
            });
        section.instrument_entries.len() - 1
    };

    let entry = &mut section.instrument_entries[index];
    entry.family = instrument.family;
    entry.level = Some(section.state.controls.level(instrument.family));
    entry.active = instrument
        .family
        .supports_active()
        .then(|| section.state.active(instrument.family));
    entry.level_override = instrument
        .family
        .supports_override()
        .then(|| section.state.level_override(instrument.family))
        .flatten();
}

fn sample_name_list(sample_assets: &[LoadedSampleAsset]) -> Vec<String> {
    if sample_assets.is_empty() {
        vec![String::from("default")]
    } else {
        sample_assets
            .iter()
            .map(|sample| sample.name().to_string())
            .collect()
    }
}

fn current_asset_duration(sample_assets: &[LoadedSampleAsset], sample_name: &str) -> f32 {
    sample_assets
        .iter()
        .find(|asset| asset.name() == sample_name)
        .map_or(1.0, LoadedSampleAsset::duration_seconds)
}

fn default_trigger(
    section: &ArrangementSection,
    sample_assets: &[LoadedSampleAsset],
) -> SampleTriggerEvent {
    let sample_name = sample_assets
        .first()
        .map(|sample| sample.name().to_string())
        .unwrap_or_else(|| String::from("default"));

    SampleTriggerEvent {
        time_seconds: section.start_seconds,
        sample_name,
        start_seconds: Some(0.0),
        end_seconds: Some(0.25),
        fade_in_seconds: Some(0.0),
        fade_out_seconds: Some(0.02),
        semitones: Some(0.0),
        cents: Some(0.0),
        gain: Some(0.5),
        pan: Some(0.0),
        rate: Some(1.0),
    }
}

fn blank_arrangement() -> Arrangement {
    Arrangement::new(Vec::new(), Vec::new(), Vec::new())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProjectTemplate {
    Blank,
    DroneBed,
    PulseSketch,
    NoiseWash,
}

impl ProjectTemplate {
    fn all() -> &'static [Self] {
        &[Self::Blank, Self::DroneBed, Self::PulseSketch, Self::NoiseWash]
    }

    fn label(self) -> &'static str {
        match self {
            Self::Blank => "Blank",
            Self::DroneBed => "Drone Bed",
            Self::PulseSketch => "Pulse Sketch",
            Self::NoiseWash => "Noise Wash",
        }
    }

    fn instantiate(self, base_state: TimelineState) -> Arrangement {
        match self {
            Self::Blank => blank_arrangement(),
            Self::DroneBed => Arrangement::new(Vec::new(), Vec::new(), vec![drone_bed_section(base_state)]),
            Self::PulseSketch => {
                Arrangement::new(Vec::new(), Vec::new(), vec![pulse_sketch_section(base_state)])
            }
            Self::NoiseWash => Arrangement::new(Vec::new(), Vec::new(), vec![noise_wash_section(base_state)]),
        }
    }
}

fn blank_state(config: &AppConfig) -> TimelineState {
    let mut controls = GardenControls::default();
    controls.drone_level = 0.0;
    controls.harmonic_level = 0.0;
    controls.pulse_level = 0.0;
    controls.sample_level = 0.0;
    controls.noise_level = 0.0;
    controls.event_level = 0.0;
    controls.texture_level = 0.0;

    let mut state = TimelineState::new(
        controls,
        config.garden.root_hz,
        config.garden.voice_count,
        1,
        2,
        0.015,
        0.195,
        2.0,
        8.0,
        9.0,
    );
    state.set_active(InstrumentFamily::Drone, false);
    state.set_active(InstrumentFamily::Harmonic, false);
    state.set_active(InstrumentFamily::Pulse, false);
    state.set_active(InstrumentFamily::Sample, false);
    state.set_active(InstrumentFamily::Noise, false);
    state.set_active(InstrumentFamily::Events, false);
    state
}

fn default_blank_section(state: TimelineState) -> ArrangementSection {
    ArrangementSection {
        name: String::from("section_1"),
        start_seconds: 0.0,
        duration_seconds: 16.0,
        mode: SectionMode::Hold,
        entry_state: state,
        state,
        instrument_entries: Vec::new(),
        sample_triggers: Vec::new(),
    }
}

fn drone_bed_section(mut state: TimelineState) -> ArrangementSection {
    state.set_active(InstrumentFamily::Drone, true);
    state.set_active(InstrumentFamily::Harmonic, true);
    state.controls.drone_level = 0.82;
    state.controls.harmonic_level = 0.28;
    state.controls.noise_level = 0.08;
    state.controls.texture_level = 0.18;
    state.controls.space = 0.78;
    state.controls.brightness = 0.32;
    state.controls.density = 0.22;

    ArrangementSection {
        name: String::from("opening"),
        start_seconds: 0.0,
        duration_seconds: 32.0,
        mode: SectionMode::Hold,
        entry_state: state,
        state,
        instrument_entries: Vec::new(),
        sample_triggers: Vec::new(),
    }
}

fn pulse_sketch_section(mut state: TimelineState) -> ArrangementSection {
    state.set_active(InstrumentFamily::Drone, true);
    state.set_active(InstrumentFamily::Harmonic, true);
    state.set_active(InstrumentFamily::Pulse, true);
    state.set_active(InstrumentFamily::Events, true);
    state.controls.drone_level = 0.55;
    state.controls.harmonic_level = 0.22;
    state.controls.pulse_level = 0.42;
    state.controls.event_level = 0.24;
    state.controls.space = 0.68;
    state.controls.brightness = 0.46;
    state.controls.density = 0.48;
    state.instrument_params.pulse_mut().rate = 1.35;
    state.instrument_params.pulse_mut().length = 0.75;

    ArrangementSection {
        name: String::from("pulse_sketch"),
        start_seconds: 0.0,
        duration_seconds: 24.0,
        mode: SectionMode::Hold,
        entry_state: state,
        state,
        instrument_entries: Vec::new(),
        sample_triggers: Vec::new(),
    }
}

fn noise_wash_section(mut state: TimelineState) -> ArrangementSection {
    state.set_active(InstrumentFamily::Drone, true);
    state.set_active(InstrumentFamily::Harmonic, true);
    state.set_active(InstrumentFamily::Noise, true);
    state.controls.drone_level = 0.36;
    state.controls.harmonic_level = 0.14;
    state.controls.noise_level = 0.44;
    state.controls.texture_level = 0.34;
    state.controls.space = 0.9;
    state.controls.brightness = 0.24;
    state.controls.instability = 0.4;
    state.instrument_params.noise_mut().motion = 1.4;

    ArrangementSection {
        name: String::from("noise_wash"),
        start_seconds: 0.0,
        duration_seconds: 28.0,
        mode: SectionMode::Hold,
        entry_state: state,
        state,
        instrument_entries: Vec::new(),
        sample_triggers: Vec::new(),
    }
}

fn next_section_from(previous: &ArrangementSection) -> ArrangementSection {
    let index = previous
        .name
        .strip_prefix("section_")
        .and_then(|value| value.parse::<usize>().ok())
        .map_or(2, |value| value + 1);

    ArrangementSection {
        name: format!("section_{index}"),
        start_seconds: previous.start_seconds + previous.duration_seconds,
        duration_seconds: previous.duration_seconds,
        mode: previous.mode,
        entry_state: previous.state,
        state: previous.state,
        instrument_entries: Vec::new(),
        sample_triggers: Vec::new(),
    }
}

fn scan_sample_library() -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_wav_files(&PathBuf::from("samples"), &mut files);
    files.sort();
    files
}

fn collect_wav_files(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_wav_files(&path, files);
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
        {
            files.push(path);
        }
    }
}

fn display_library_path(path: &Path) -> String {
    let cwd = std::env::current_dir().ok();
    cwd.as_ref()
        .and_then(|cwd| path.strip_prefix(cwd).ok())
        .unwrap_or(path)
        .display()
        .to_string()
}

fn default_asset_name(path: &Path) -> String {
    sanitize_asset_name(
        &path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("sample")
            .to_ascii_lowercase(),
    )
}

fn sanitize_asset_name(name: &str) -> String {
    let mut normalized = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
        } else if (ch == '-' || ch == '_' || ch == ' ') && !normalized.ends_with('_') {
            normalized.push('_');
        }
    }
    normalized.trim_matches('_').to_string()
}

fn next_default_instrument_id(family: InstrumentFamily) -> String {
    match family {
        InstrumentFamily::Drone => String::from("drone_alt"),
        InstrumentFamily::Harmonic => String::from("harmonic_alt"),
        InstrumentFamily::Pulse => String::from("pulse_alt"),
        InstrumentFamily::Sample => String::from("sample_alt"),
        InstrumentFamily::Noise => String::from("noise_alt"),
        InstrumentFamily::Events => String::from("events_alt"),
        InstrumentFamily::Texture => String::from("texture_alt"),
    }
}

fn sample_usage_counts(arrangement: &Arrangement) -> Vec<(String, usize)> {
    let mut counts = Vec::<(String, usize)>::new();
    for trigger in arrangement.sample_triggers() {
        if let Some((_, count)) = counts
            .iter_mut()
            .find(|(name, _)| name == &trigger.sample_name)
        {
            *count += 1;
        } else {
            counts.push((trigger.sample_name.clone(), 1));
        }
    }
    counts
}

fn make_arrangement_relative_path(base_dir: &Path, asset_path: &Path) -> String {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let absolute_base = if base_dir.is_absolute() {
        base_dir.to_path_buf()
    } else {
        cwd.join(base_dir)
    };
    let absolute_asset = if asset_path.is_absolute() {
        asset_path.to_path_buf()
    } else {
        cwd.join(asset_path)
    };

    relative_path(&absolute_base, &absolute_asset)
        .unwrap_or_else(|| PathBuf::from(display_library_path(asset_path)))
        .display()
        .to_string()
}

fn relative_path(from_dir: &Path, to_path: &Path) -> Option<PathBuf> {
    let from_components = normalized_components(from_dir);
    let to_components = normalized_components(to_path);

    if from_dir.is_absolute() != to_path.is_absolute() {
        return None;
    }

    let mut common = 0usize;
    while common < from_components.len()
        && common < to_components.len()
        && from_components[common] == to_components[common]
    {
        common += 1;
    }

    let mut relative = PathBuf::new();
    for _ in common..from_components.len() {
        relative.push("..");
    }
    for component in &to_components[common..] {
        relative.push(component.as_os_str());
    }

    if relative.as_os_str().is_empty() {
        relative.push(".");
    }

    Some(relative)
}

fn normalized_components(path: &Path) -> Vec<Component<'_>> {
    path.components()
        .filter(|component| !matches!(component, Component::CurDir))
        .collect()
}
