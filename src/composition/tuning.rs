use crate::composition::pitch::PitchField;

#[derive(Clone, Debug)]
pub struct TuningConfig {
    pitch_field: PitchField,
}

impl TuningConfig {
    pub fn default_just(root_hz: f32) -> Self {
        Self {
            pitch_field: PitchField::default_just(root_hz),
        }
    }

    pub fn root_hz(&self) -> f32 {
        self.pitch_field.root_hz()
    }

    pub fn pitch_field(&self) -> &PitchField {
        &self.pitch_field
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegisterRange {
    pub octave_min: i32,
    pub octave_max: i32,
}

impl RegisterRange {
    pub fn new(octave_min: i32, octave_max: i32) -> Self {
        Self {
            octave_min,
            octave_max,
        }
        .clamped()
    }

    pub fn clamped(self) -> Self {
        let octave_min = self.octave_min.clamp(0, 5);
        let octave_max = self.octave_max.clamp(0, 5).max(octave_min);

        Self {
            octave_min,
            octave_max,
        }
    }
}
