use super::mix::StereoSample;
use std::fs::File;
use std::io::{self, BufWriter, Write};

pub(crate) fn write_wav_stereo_i16(
    path: impl AsRef<std::path::Path>,
    sample_rate: u32,
    samples: &[StereoSample],
) -> io::Result<()> {
    let channels = 2_u16;
    let bytes_per_sample = 2_u16;
    let data_len = samples.len() as u32 * channels as u32 * bytes_per_sample as u32;
    let mut writer = BufWriter::new(File::create(path)?);

    writer.write_all(b"RIFF")?;
    writer.write_all(&(36 + data_len).to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_all(&16_u32.to_le_bytes())?;
    writer.write_all(&1_u16.to_le_bytes())?;
    writer.write_all(&channels.to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&(sample_rate * channels as u32 * bytes_per_sample as u32).to_le_bytes())?;
    writer.write_all(&(channels * bytes_per_sample).to_le_bytes())?;
    writer.write_all(&16_u16.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_len.to_le_bytes())?;

    for sample in samples {
        let left = (sample.left.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        let right = (sample.right.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer.write_all(&left.to_le_bytes())?;
        writer.write_all(&right.to_le_bytes())?;
    }

    writer.flush()
}
