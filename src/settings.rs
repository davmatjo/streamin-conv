use std::path::PathBuf;

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub port: i64,
    pub dirs: Dirs,
}

#[derive(Debug, Deserialize)]
pub struct Dirs {
    pub unprocessed: PathBuf,
    pub processed: PathBuf,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let mut s = Config::new();

        // Start off by merging in the "default" configuration file
        s.merge(File::with_name("config.yaml"))?;

        // Add in settings from the environment (with a prefix of APP)
        // Eg.. `APP_DEBUG=1 ./target/app` would set the `debug` key
        s.merge(Environment::with_prefix("streamin"))?;

        // You can deserialize (and thus freeze) the entire configuration as
        s.try_into()
    }
}