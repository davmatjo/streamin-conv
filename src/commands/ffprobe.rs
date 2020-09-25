use std::error::Error;
use std::path::Path;
use std::process::Command;

use log::debug;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct FFProbeResponse {
    pub streams: Vec<Stream>,
    pub format: Format,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Format {
    pub duration: String
}

#[derive(Deserialize, Debug, Clone)]
pub struct Stream {
    pub index: isize,
    pub codec_name: String,
    pub codec_type: String,
    pub tags: Option<Tags>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Tags {
    pub title: Option<String>,
    pub language: Option<String>,
}

pub fn get_info(file: &Path) -> Result<FFProbeResponse, Box<dyn Error>> {
    let out = Command::new("ffprobe")
        .arg("-v")
        .arg("quiet")
        .arg("-print_format")
        .arg("json")
        .arg("-show_streams")
        .arg("-show_entries")
        .arg("format=duration")
        .arg(file)
        .output()?;

    debug!("{:?}", std::str::from_utf8(&out.stdout));

    let parsed: FFProbeResponse = serde_json::from_slice(&out.stdout)?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::commands::ffprobe::get_info;

    #[test]
    fn parse() {
        println!("{:?}", get_info(Path::new("1.mkv")).unwrap())
    }
}