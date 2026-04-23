#[derive(Clone, Copy, Debug, Default)]
pub struct StereoSample {
    pub left: f32,
    pub right: f32,
}

impl StereoSample {
    pub fn new(left: f32, right: f32) -> Self {
        Self { left, right }
    }

    pub fn scale(self, gain: f32) -> Self {
        Self {
            left: self.left * gain,
            right: self.right * gain,
        }
    }
}

impl std::ops::AddAssign for StereoSample {
    fn add_assign(&mut self, other: Self) {
        self.left += other.left;
        self.right += other.right;
    }
}

impl std::ops::Add for StereoSample {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self {
            left: self.left + other.left,
            right: self.right + other.right,
        }
    }
}
