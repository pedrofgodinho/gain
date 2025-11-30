use anyhow::Result;
use log::info;
use std::{collections::HashMap, fs, time::Instant};

/// Configuration structure for the application, deserialized from a TOML file.
#[derive(serde::Deserialize, Debug, Clone)]
pub struct Config {
    /// Optional communication port (e.g., COM3 or /dev/ttyUSB0). If None, auto-detection is used.
    pub comm_port: Option<String>,
    /// Baud rate for serial communication. Defaults to 57600 if not specified.
    #[serde(default = "default_baud_rate")]
    pub baud_rate: u32,
    /// Mappings for sliders to volume targets.
    #[serde(default)]
    pub slider: Vec<SliderMappings>,
    /// Volume adjustment step size (0.0 to 1.0) for each slider movement.
    #[serde(default = "default_volume_step")]
    pub volume_step: f64,
}

fn default_baud_rate() -> u32 {
    57600
}

fn default_volume_step() -> f64 {
    0.01
}

/// Mapping of a slider to a specific volume target.
#[derive(serde::Deserialize, Debug, Clone)]
pub struct SliderMappings {
    /// Slider ID (e.g., 0 for the first slider).
    pub id: u8,
    /// Target volume control for the slider.
    #[serde(default)]
    pub target: VolumeTarget,
}

/// Enumeration of possible volume targets for a slider.
#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum VolumeTarget {
    /// Master volume control.
    Master,
    /// Volume control for the currently active application.
    #[serde(rename = "current")]
    CurrentApp,
    /// Volume control for applications not explicitly mapped.
    Unmapped,
    /// Volume control for specific applications.
    Apps(Vec<String>),
}

impl Default for VolumeTarget {
    fn default() -> Self {
        VolumeTarget::Apps(vec![])
    }
}

/// Loaded configuration with additional runtime data.
pub struct LoadedConfig {
    /// The base configuration.
    pub config: Config,
    /// Mappings of slider IDs to their respective configurations.
    pub mappings: HashMap<u8, SliderMappings>,
    /// List of applications that have specific volume mappings.
    pub mapped_apps: Vec<String>,
    last_modified: std::time::SystemTime,
    last_checked: std::time::Instant,
}

impl LoadedConfig {
    /// Loads the configuration from a specified TOML file.
    pub fn new_from_file(filename: &str) -> Result<Self> {
        let config_data = std::fs::read_to_string(filename)?;
        let config: Config = toml::from_str(&config_data)?;
        let last_modified = fs::metadata(filename)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::now());
        Ok(LoadedConfig::new(config, last_modified))
    }

    /// Reloads the configuration from the file if it has been modified since the last load.
    pub fn reload_if_needed(&mut self, filename: &str) -> Result<()> {
        if self.should_reload(filename) {
            let config_data = fs::read_to_string(filename)?;
            let config: Config = toml::from_str(&config_data)?;
            *self = LoadedConfig::new(config, self.last_modified);
            info!("Configuration reloaded from {}", filename);
        }
        Ok(())
    }

    fn new(config: Config, last_modified: std::time::SystemTime) -> Self {
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
}
