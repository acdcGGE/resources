use std::path::Path;
use std::str::FromStr;
use std::sync::LazyLock;
use std::time::SystemTime;

use anyhow::{Context, Result};
use lazy_regex::Regex;
use log::trace;

pub static NUM_CPUS: LazyLock<usize> = LazyLock::new(num_cpus::get);

#[must_use]
pub fn unix_as_millis() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[must_use]
pub fn unix_as_secs_f64() -> f64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

pub fn read_parsed<T: FromStr>(path: impl AsRef<Path>) -> Result<T>
where
    <T as FromStr>::Err: std::error::Error + Send + Sync + 'static,
{
    let path = path.as_ref();

    let content = std::fs::read_to_string(path)
        .map(|content| content.trim().to_string())
        .inspect_err(|e| trace!("Unable to read {path:?} → {e}"))?;

    let type_name = std::any::type_name::<T>();

    content
        .parse::<T>()
        .inspect(|_| trace!("Successfully read {path:?} to {type_name} → {content}"))
        .inspect_err(|e| trace!("Unable to parse {path:?} to {type_name} → {e}"))
        .with_context(|| format!("error parsing file {}", path.display()))
}

pub fn extract_from_regex<U: FromStr + Default, S: AsRef<str>>(
    regex: &Regex,
    string: S,
) -> Option<U> {
    regex
        .captures(string.as_ref())
        .and_then(|captures| captures.get(1))
        .map(|capture| capture.as_str())
        .unwrap_or_default()
        .parse::<U>()
        .ok()
}
