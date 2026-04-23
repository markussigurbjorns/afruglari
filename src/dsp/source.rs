use crate::dsp::sample::StereoSample;

pub trait StereoSource {
    fn next_stereo(&mut self) -> StereoSample;
}
