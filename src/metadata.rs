use crate::presets::PiecePreset;
use crate::render::{RenderMode, parse_render_mode, render_mode_name};
use crate::workflow::GenerateError;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationMetadata {
    pub piece: String,
    pub preset: PiecePreset,
    pub seed: u64,
    pub render_mode: RenderMode,
    pub output: PathBuf,
    pub events: usize,
    pub collisions: usize,
    pub voice_density: Vec<usize>,
    pub voice_render_count: usize,
    pub section_render_count: usize,
    pub voice_renders: Vec<String>,
    pub section_renders: Vec<String>,
}

impl GenerationMetadata {
    pub fn json_path(&self) -> PathBuf {
        self.output.with_extension("json")
    }

    pub fn to_json(&self) -> String {
        let densities = self
            .voice_density
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let voice_renders = json_string_array(&self.voice_renders);
        let section_renders = json_string_array(&self.section_renders);
        format!(
            "{{\n  \"preset\": \"{}\",\n  \"seed\": {},\n  \"render_mode\": \"{}\",\n  \"output\": \"{}\",\n  \"events\": {},\n  \"collisions\": {},\n  \"voice_density\": [{}],\n  \"voice_render_count\": {},\n  \"section_render_count\": {},\n  \"voice_renders\": {},\n  \"section_renders\": {}\n}}\n",
            escape_json(&self.piece),
            self.seed,
            render_mode_name(self.render_mode),
            escape_json(&self.output.display().to_string()),
            self.events,
            self.collisions,
            densities,
            self.voice_render_count,
            self.section_render_count,
            voice_renders,
            section_renders
        )
    }

    pub fn parse_json(source: &str) -> Result<Self, GenerateError> {
        let piece = json_string(source, "preset")?;
        let seed = json_u64(source, "seed")?;
        let render_mode = parse_render_mode(&json_string(source, "render_mode")?)
            .ok_or_else(|| GenerateError::Config("metadata has unknown render_mode".to_string()))?;
        let output = PathBuf::from(json_string(source, "output")?);
        let events = json_usize(source, "events")?;
        let collisions = json_usize(source, "collisions")?;
        let voice_density = json_usize_array(source, "voice_density")?;
        let voice_render_count = json_optional_usize(source, "voice_render_count")?.unwrap_or(0);
        let section_render_count =
            json_optional_usize(source, "section_render_count")?.unwrap_or(0);
        let voice_renders =
            json_optional_string_array(source, "voice_renders")?.unwrap_or_default();
        let section_renders =
            json_optional_string_array(source, "section_renders")?.unwrap_or_default();
        let preset = PiecePreset::parse(&piece).unwrap_or(PiecePreset::Example);

        Ok(Self {
            piece,
            preset,
            seed,
            render_mode,
            output,
            events,
            collisions,
            voice_density,
            voice_render_count,
            section_render_count,
            voice_renders,
            section_renders,
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MetadataFilter {
    pub min_collisions: Option<usize>,
    pub max_collisions: Option<usize>,
    pub min_events: Option<usize>,
    pub max_events: Option<usize>,
    pub voice_dominates: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanEntry {
    pub metadata_path: PathBuf,
    pub metadata: GenerationMetadata,
}

pub fn scan_metadata(
    dir: impl AsRef<Path>,
    filter: &MetadataFilter,
) -> Result<Vec<ScanEntry>, GenerateError> {
    let mut files = Vec::new();
    collect_metadata_files(dir.as_ref(), &mut files)?;
    files.sort();

    let mut entries = Vec::new();
    for path in files {
        let source = fs::read_to_string(&path)?;
        let metadata = GenerationMetadata::parse_json(&source)?;
        if metadata_matches(&metadata, filter) {
            entries.push(ScanEntry {
                metadata_path: path,
                metadata,
            });
        }
    }

    Ok(entries)
}

fn collect_metadata_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), GenerateError> {
    if dir.is_file() {
        if dir.extension().is_some_and(|extension| extension == "json") {
            files.push(dir.to_path_buf());
        }
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_metadata_files(&path, files)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            files.push(path);
        }
    }

    Ok(())
}

fn metadata_matches(metadata: &GenerationMetadata, filter: &MetadataFilter) -> bool {
    if filter
        .min_collisions
        .is_some_and(|min| metadata.collisions < min)
    {
        return false;
    }
    if filter
        .max_collisions
        .is_some_and(|max| metadata.collisions > max)
    {
        return false;
    }
    if filter.min_events.is_some_and(|min| metadata.events < min) {
        return false;
    }
    if filter.max_events.is_some_and(|max| metadata.events > max) {
        return false;
    }
    if let Some(voice) = filter.voice_dominates {
        let Some(&target) = metadata.voice_density.get(voice) else {
            return false;
        };
        if metadata
            .voice_density
            .iter()
            .enumerate()
            .any(|(index, &count)| index != voice && count >= target)
        {
            return false;
        }
    }

    true
}

fn json_string(source: &str, key: &str) -> Result<String, GenerateError> {
    let value = json_value(source, key)?;
    let value = value.trim();
    if !value.starts_with('"') {
        return Err(GenerateError::Config(format!(
            "metadata key '{key}' is not a string"
        )));
    }

    let mut escaped = false;
    let mut output = String::new();
    for character in value[1..].chars() {
        if escaped {
            output.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Ok(output);
        } else {
            output.push(character);
        }
    }

    Err(GenerateError::Config(format!(
        "metadata key '{key}' has unterminated string"
    )))
}

fn json_u64(source: &str, key: &str) -> Result<u64, GenerateError> {
    json_value(source, key)?
        .trim_matches(',')
        .trim()
        .parse()
        .map_err(|_| GenerateError::Config(format!("metadata key '{key}' is not an integer")))
}

fn json_usize(source: &str, key: &str) -> Result<usize, GenerateError> {
    json_value(source, key)?
        .trim_matches(',')
        .trim()
        .parse()
        .map_err(|_| GenerateError::Config(format!("metadata key '{key}' is not an integer")))
}

fn json_optional_usize(source: &str, key: &str) -> Result<Option<usize>, GenerateError> {
    let Some(value) = json_value_optional(source, key)? else {
        return Ok(None);
    };
    value
        .trim_matches(',')
        .trim()
        .parse()
        .map(Some)
        .map_err(|_| GenerateError::Config(format!("metadata key '{key}' is not an integer")))
}

fn json_usize_array(source: &str, key: &str) -> Result<Vec<usize>, GenerateError> {
    let value = json_value(source, key)?;
    let start = value
        .find('[')
        .ok_or_else(|| GenerateError::Config(format!("metadata key '{key}' is not an array")))?;
    let end = value
        .find(']')
        .ok_or_else(|| GenerateError::Config(format!("metadata key '{key}' is not an array")))?;
    let inner = value[start + 1..end].trim();
    if inner.is_empty() {
        return Ok(Vec::new());
    }

    inner
        .split(',')
        .map(|value| {
            value.trim().parse().map_err(|_| {
                GenerateError::Config(format!("metadata key '{key}' contains a non-integer"))
            })
        })
        .collect()
}

fn json_optional_string_array(
    source: &str,
    key: &str,
) -> Result<Option<Vec<String>>, GenerateError> {
    let Some(value) = json_value_optional(source, key)? else {
        return Ok(None);
    };
    Ok(Some(parse_json_string_array_value(value, key)?))
}

fn parse_json_string_array_value(value: &str, key: &str) -> Result<Vec<String>, GenerateError> {
    let start = value
        .find('[')
        .ok_or_else(|| GenerateError::Config(format!("metadata key '{key}' is not an array")))?;
    let end = value
        .rfind(']')
        .ok_or_else(|| GenerateError::Config(format!("metadata key '{key}' is not an array")))?;
    let inner = value[start + 1..end].trim();
    if inner.is_empty() {
        return Ok(Vec::new());
    }

    let mut values = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;
    for character in inner.chars() {
        if !in_string {
            if character == '"' {
                in_string = true;
            }
            continue;
        }

        if escaped {
            current.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            values.push(std::mem::take(&mut current));
            in_string = false;
        } else {
            current.push(character);
        }
    }

    if in_string {
        return Err(GenerateError::Config(format!(
            "metadata key '{key}' has unterminated string"
        )));
    }

    Ok(values)
}

fn json_value<'a>(source: &'a str, key: &str) -> Result<&'a str, GenerateError> {
    json_value_optional(source, key)?
        .ok_or_else(|| GenerateError::Config(format!("metadata missing key '{key}'")))
}

fn json_value_optional<'a>(source: &'a str, key: &str) -> Result<Option<&'a str>, GenerateError> {
    let needle = format!("\"{key}\"");
    let Some(key_start) = source.find(&needle) else {
        return Ok(None);
    };
    let after_key = &source[key_start + needle.len()..];
    let colon = after_key
        .find(':')
        .ok_or_else(|| GenerateError::Config(format!("metadata key '{key}' has no value")))?;
    let after_colon = after_key[colon + 1..].trim_start();
    let line_end = after_colon.find('\n').unwrap_or(after_colon.len());
    Ok(Some(after_colon[..line_end].trim().trim_end_matches(',')))
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn json_string_array(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| format!("\"{}\"", escape_json(value)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}
