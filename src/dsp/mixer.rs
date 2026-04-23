use crate::dsp::sample::StereoSample;
use crate::dsp::source::StereoSource;

pub struct VoiceBank<T> {
    voices: Vec<T>,
    output_gain: f32,
}

impl<T> VoiceBank<T> {
    pub fn new(voices: Vec<T>, output_gain: f32) -> Self {
        Self {
            voices,
            output_gain,
        }
    }

    pub fn voices_mut(&mut self) -> &mut [T] {
        &mut self.voices
    }
}

impl<T> StereoSource for VoiceBank<T>
where
    T: StereoSource,
{
    fn next_stereo(&mut self) -> StereoSample {
        let mut mixed = StereoSample::default();

        for voice in &mut self.voices {
            mixed += voice.next_stereo();
        }

        mixed.scale(self.output_gain)
    }
}
