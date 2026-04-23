pub mod drone;
pub mod events;
pub mod harmonic_pad;
pub mod noise;
pub mod pulse;
pub mod rack;
pub mod sampler;

use crate::composition::garden::GardenControls;
use crate::dsp::source::StereoSource;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstrumentFamily {
    Drone,
    Harmonic,
    Pulse,
    Sample,
    Noise,
    Events,
    Texture,
}

impl InstrumentFamily {
    pub const COUNT: usize = 7;

    pub fn all() -> &'static [Self] {
        &[
            Self::Drone,
            Self::Harmonic,
            Self::Pulse,
            Self::Sample,
            Self::Noise,
            Self::Events,
            Self::Texture,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Drone => "Drone",
            Self::Harmonic => "Harmonic",
            Self::Pulse => "Pulse",
            Self::Sample => "Sample",
            Self::Noise => "Noise",
            Self::Events => "Events",
            Self::Texture => "Texture",
        }
    }

    pub fn supports_active(self) -> bool {
        !matches!(self, Self::Texture)
    }

    pub fn supports_override(self) -> bool {
        !matches!(self, Self::Texture)
    }

    pub fn index(self) -> usize {
        match self {
            Self::Drone => 0,
            Self::Harmonic => 1,
            Self::Pulse => 2,
            Self::Sample => 3,
            Self::Noise => 4,
            Self::Events => 5,
            Self::Texture => 6,
        }
    }
}

#[allow(dead_code)]
pub trait Instrument: StereoSource {
    fn set_controls(&mut self, controls: GardenControls);
    fn set_active(&mut self, active: bool);
    fn is_active(&self) -> bool;
}
