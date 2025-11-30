use std::{collections::HashMap, fs, time::Instant};

use log::info;

#[derive(serde::Deserialize, Debug, Clone)]
pub struct Config {
    pub comm_port: Option<String>,
    #[serde(default)]
    pub slider: Vec<SliderMappings>,
    pub volume_step: f64,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct SliderMappings {
    pub id: u8,
    #[serde(default)]
    pub target: VolumeTarget,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum VolumeTarget {
    Master,
    #[serde(rename = "current")]
    CurrentApp,
    Unmapped,
    Apps(Vec<String>),
}

impl Default for VolumeTarget {
    fn default() -> Self {
        VolumeTarget::Apps(vec![])
    }
}

pub struct LoadedConfig {
    pub config: Config,
    pub mappings: HashMap<u8, SliderMappings>,
    pub mapped_apps: Vec<String>,
    last_modified: std::time::SystemTime,
    last_checked: std::time::Instant,
}

impl LoadedConfig {
    pub fn new_from_file(filename: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config_data = std::fs::read_to_string(filename)?;
        let config: Config = toml::from_str(&config_data)?;
        let last_modified = fs::metadata(filename)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::now());
        Ok(LoadedConfig::new(config, last_modified))
    }

    pub fn new(config: Config, last_modified: std::time::SystemTime) -> Self {
        let mappings: HashMap<u8, SliderMappings> = config
            .slider
            .clone()
            .into_iter()
            .map(|s| (s.id, s))
            .collect();

        let mapped_apps: Vec<String> = mappings
            .values()
            .filter_map(|mapping| {
                if let VolumeTarget::Apps(apps) = &mapping.target {
                    Some(apps.clone())
                } else {
                    None
                }
            })
            .flatten()
            .collect();

        LoadedConfig {
            config,
            mappings,
            mapped_apps,
            last_modified,
            last_checked: Instant::now(),
        }
    }

    fn should_reload(&mut self, filename: &str) -> bool {
        let now = Instant::now();
        // Throttle checks to once every 2 seconds
        if now.duration_since(self.last_checked).as_secs() < 2 {
            return false;
        }
        self.last_checked = now;

        match fs::metadata(filename).and_then(|m| m.modified()) {
            Ok(modified_time) => {
                if modified_time > self.last_modified {
                    self.last_modified = modified_time;
                    true
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }

    pub fn reload_if_needed(&mut self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        if self.should_reload(filename) {
            let config_data = fs::read_to_string(filename)?;
            let config: Config = toml::from_str(&config_data)?;
            *self = LoadedConfig::new(config, self.last_modified);
            info!("Configuration reloaded from {}", filename);
        }
        Ok(())
    }
}
