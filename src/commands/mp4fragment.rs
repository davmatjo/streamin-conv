use std::error::Error;
use std::path::PathBuf;

use tokio::process::Command;

use crate::commands::{MediaCommandConfig, SessionError};
use crate::commands::SessionError::InvalidCommandConfig;

pub struct Config {
    file: PathBuf,
    out_file: Option<PathBuf>,
    can_fail: bool,
}

impl MediaCommandConfig for Config {
    fn build(&self) -> Result<Command, Box<dyn Error>> {
        let mut cmd = Command::new("mp4fragment");

        let out = self.out_file.clone().unwrap_or({
            let mut base = std::env::temp_dir();
            let mut stem = self.file.file_stem().unwrap().to_os_string();
            stem.push("-f.mp4");
            base.push(stem);
            base
        });

        cmd.arg(&self.file)
            .arg(&out);
        Ok(cmd)
    }

    fn validate(&self) -> Result<(), SessionError> {
        if !self.file.exists() {
            return Err(InvalidCommandConfig("File does not exist"));
        }
        Ok(())
    }

    fn can_fail(&self) -> bool {
        self.can_fail
    }
}

impl Config {
    pub fn new(file: PathBuf) -> Self {
        Config {
            file,
            out_file: None,
            can_fail: false,
        }
    }

    pub fn can_fail(&mut self) -> &mut Self {
        self.can_fail = true;
        self
    }

    #[allow(dead_code)]
    pub fn out_file(&mut self, out: PathBuf) -> &mut Self {
        self.out_file = Some(out);
        self
    }
}