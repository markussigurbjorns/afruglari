use crate::composition::arrangement::SampleTriggerEvent;
use crate::composition::garden::{
    EventShapeConfig, GardenConfig, GardenControls, InstrumentParams,
};
use crate::composition::timeline::TimelineState;
use crate::composition::tuning::{RegisterRange, TuningConfig};
use crate::dsp::sample::StereoSample;
use crate::dsp::source::StereoSource;
use crate::instruments::drone::DroneInstrument;
use crate::instruments::events::EventInstrument;
use crate::instruments::harmonic_pad::HarmonicPadInstrument;
use crate::instruments::noise::NoiseInstrument;
use crate::instruments::pulse::PulseInstrument;
use crate::instruments::sampler::SamplerInstrument;
use crate::instruments::{Instrument, InstrumentFamily};

pub struct InstrumentRack {
    drones: DroneInstrument,
    harmonic_pad: HarmonicPadInstrument,
    pulse: PulseInstrument,
    sampler: Option<SamplerInstrument>,
    noise: NoiseInstrument,
    events: EventInstrument,
}

impl InstrumentRack {
    pub fn new(
        sample_rate: f32,
        config: GardenConfig,
        tuning: &TuningConfig,
        register: RegisterRange,
        explicit_sample_triggering: bool,
    ) -> Self {
        let controls = config.controls.clamped();
        let drones = DroneInstrument::new(
            sample_rate,
            tuning.pitch_field().clone(),
            register,
            config.voice_count,
            config.seed,
            controls,
        );
        let harmonic_pad = HarmonicPadInstrument::new(
            sample_rate,
            tuning.pitch_field().clone(),
            register,
            config.voice_count,
            config.seed ^ 0xC6BC_2796_92B5_C323,
            controls,
        );
        let pulse = PulseInstrument::new(
            sample_rate,
            tuning.pitch_field().clone(),
            register,
            config.seed ^ 0x94D0_49BB_1331_11EB,
            controls,
        );
        let noise =
            NoiseInstrument::new(sample_rate, config.seed ^ 0x9E37_79B9_7F4A_7C15, controls);
        let mut sampler = (!config.sample_assets.is_empty()).then(|| {
            SamplerInstrument::new(
                sample_rate,
                config.sample_assets,
                config.seed ^ 0xF135_7AEA_2E62_A9C5,
                controls,
            )
        });
        if let Some(sampler) = &mut sampler {
            sampler.set_explicit_triggering(explicit_sample_triggering);
        }
        let events = EventInstrument::new(
            sample_rate,
            tuning.pitch_field().clone(),
            register,
            config.seed ^ 0xD1B5_4A32_D192_ED03,
            controls,
        );

        Self {
            drones,
            harmonic_pad,
            pulse,
            sampler,
            noise,
            events,
        }
    }

    pub fn voice_count(&self) -> usize {
        self.drones.voice_count()
    }

    pub fn set_controls(&mut self, controls: GardenControls) {
        self.drones.set_controls(controls);
        self.harmonic_pad.set_controls(controls);
        self.pulse.set_controls(controls);
        if let Some(sampler) = &mut self.sampler {
            sampler.set_controls(controls);
        }
        self.noise.set_controls(controls);
        self.events.set_controls(controls);
    }

    pub fn set_pitch_field(&mut self, tuning: &TuningConfig) {
        self.drones.set_pitch_field(tuning.pitch_field().clone());
        self.harmonic_pad.set_pitch_field(tuning.pitch_field().clone());
        self.pulse.set_pitch_field(tuning.pitch_field().clone());
        self.events.set_pitch_field(tuning.pitch_field().clone());
    }

    pub fn set_voice_count(&mut self, voice_count: usize) {
        self.drones.set_voice_count(voice_count);
        self.harmonic_pad.set_voice_count(voice_count);
        self.pulse.set_voice_count(voice_count);
    }

    pub fn set_register(&mut self, register: RegisterRange) {
        self.drones.set_register(register);
        self.harmonic_pad.set_register(register);
        self.pulse.set_register(register);
        self.events.set_register(register);
    }

    pub fn set_event_shape(&mut self, event_shape: EventShapeConfig) {
        self.events.set_attack_range(
            event_shape.attack_min_seconds,
            event_shape.attack_max_seconds,
        );
        self.events.set_decay_range(
            event_shape.decay_min_seconds,
            event_shape.decay_max_seconds,
        );
    }

    pub fn set_active(&mut self, family: InstrumentFamily, active: bool) {
        match family {
            InstrumentFamily::Drone => self.drones.set_active(active),
            InstrumentFamily::Harmonic => self.harmonic_pad.set_active(active),
            InstrumentFamily::Pulse => self.pulse.set_active(active),
            InstrumentFamily::Sample => {
                if let Some(sampler) = &mut self.sampler {
                    sampler.set_active(active);
                }
            }
            InstrumentFamily::Noise => self.noise.set_active(active),
            InstrumentFamily::Events => self.events.set_active(active),
            InstrumentFamily::Texture => {}
        }
    }

    pub fn set_drone_retune_seconds(&mut self, drone_retune_seconds: f32) {
        self.drones.set_retune_seconds(drone_retune_seconds);
        self.harmonic_pad.set_retune_seconds(drone_retune_seconds);
    }

    pub fn set_params(&mut self, instrument_params: InstrumentParams) {
        self.drones.set_params(instrument_params.drone());
        self.harmonic_pad.set_params(instrument_params.harmonic());
        self.pulse.set_params(instrument_params.pulse());
        if let Some(sampler) = &mut self.sampler {
            sampler.set_params(instrument_params.sample());
        }
        self.noise.set_params(instrument_params.noise());
    }

    pub fn apply_timeline_overrides(&mut self, state: TimelineState) {
        let base_controls = state.controls.clamped();

        let mut controls = base_controls;
        *controls.level_mut(InstrumentFamily::Drone) = state
            .level_override(InstrumentFamily::Drone)
            .unwrap_or(base_controls.level(InstrumentFamily::Drone));
        self.drones.set_controls(controls);

        let mut controls = base_controls;
        *controls.level_mut(InstrumentFamily::Harmonic) = state
            .level_override(InstrumentFamily::Harmonic)
            .unwrap_or(base_controls.level(InstrumentFamily::Harmonic));
        self.harmonic_pad.set_controls(controls);

        let mut controls = base_controls;
        *controls.level_mut(InstrumentFamily::Pulse) = state
            .level_override(InstrumentFamily::Pulse)
            .unwrap_or(base_controls.level(InstrumentFamily::Pulse));
        self.pulse.set_controls(controls);

        let mut controls = base_controls;
        *controls.level_mut(InstrumentFamily::Sample) = state
            .level_override(InstrumentFamily::Sample)
            .unwrap_or(base_controls.level(InstrumentFamily::Sample));
        if let Some(sampler) = &mut self.sampler {
            sampler.set_controls(controls);
        }

        let mut controls = base_controls;
        *controls.level_mut(InstrumentFamily::Noise) = state
            .level_override(InstrumentFamily::Noise)
            .unwrap_or(base_controls.level(InstrumentFamily::Noise));
        self.noise.set_controls(controls);

        let mut controls = base_controls;
        *controls.level_mut(InstrumentFamily::Events) = state
            .level_override(InstrumentFamily::Events)
            .unwrap_or(base_controls.level(InstrumentFamily::Events));
        self.events.set_controls(controls);
    }

    pub fn trigger_sample(&mut self, trigger: &SampleTriggerEvent) {
        if let Some(sampler) = &mut self.sampler {
            sampler.trigger_once(
                &trigger.sample_name,
                trigger.start_seconds,
                trigger.end_seconds,
                trigger.fade_in_seconds,
                trigger.fade_out_seconds,
                trigger.semitones,
                trigger.cents,
                trigger.gain,
                trigger.pan,
                trigger.rate,
            );
        }
    }

    pub fn next_stereo(&mut self) -> StereoSample {
        let mut dry = self.drones.next_stereo();
        dry += self.harmonic_pad.next_stereo();
        dry += self.pulse.next_stereo();
        if let Some(sampler) = &mut self.sampler {
            dry += sampler.next_stereo();
        }
        dry += self.noise.next_stereo();
        dry += self.events.next_stereo();
        dry
    }
}
