use std::error::Error;
use std::path::PathBuf;

use tokio::process::Command;

use crate::commands::{MediaCommandConfig, SessionError};
use crate::commands::SessionError::InvalidCommandConfig;
use crate::PROCESSED_DIR;

#[cfg(target_os = "linux")]
static DEFAULT_PATH: &str = "mp4dash";
#[cfg(target_os = "windows")]
static DEFAULT_PATH: &str = "cmd";

pub struct Config {
    files: Vec<PathBuf>,
    out_dir: Option<PathBuf>,
}

impl MediaCommandConfig for Config {
    fn build(&self) -> Result<Command, Box<dyn Error>> {
        let mut cmd = Command::new(DEFAULT_PATH);

        if cfg!(windows) {
            cmd.arg("/c")
                .arg("mp4dash");
        }

        cmd.arg("-o")
            .arg(self.out_dir.clone().unwrap_or({
                let base = *PROCESSED_DIR;
                let mut base = base.to_path_buf();
                base.push(self.files[0]
                    // Taking the stem of the file before any added hyphens and using it as a directory
                    // name under PROCESSED_DIR
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .split('-')
                    .next()
                    .unwrap()
                );
                base
            }));

        cmd.arg("--mpd-name=manifest.mpd")
            .arg("--use-segment-timeline");

        let mut i = 0;
        for file in &self.files {
            let file = file.to_str().unwrap();
            if file.contains("-aud-") {
                i += 1;
                cmd.arg(format!("[+language={}]{}", i, file));
            } else if file.contains("-sub-") {
                cmd.arg(format!("[+format=webvtt]{}", file));
            } else {
                cmd.arg(file);
            }
        }


        Ok(cmd)
    }

    fn validate(&self) -> Result<(), SessionError> {
        Ok(())
    }

    fn can_fail(&self) -> bool {
        false
    }
}

impl Config {
    pub fn new<T>(files: T) -> Self
        where T: IntoIterator<Item=PathBuf>
    {
        Config {
            files: files.into_iter().collect(),
            out_dir: None,
        }
    }

    #[allow(dead_code)]
    pub fn out_dir(&mut self, dir: PathBuf) -> Result<&mut Self, SessionError> {
        if dir.exists() {
            return Err(InvalidCommandConfig("directory already exists"));
        }
        if dir.extension().is_some() {
            return Err(InvalidCommandConfig("path must be a directory"));
        }
        self.out_dir = Some(dir);
        Ok(self)
    }
}