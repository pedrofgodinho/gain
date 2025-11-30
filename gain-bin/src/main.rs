mod volume;

use gain_lib::Slider;
use log::{error, info, trace, warn};
use serialport::{SerialPort, SerialPortType};
use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader},
};

use crate::volume::{
    set_app_volume, set_current_app_volume, set_master_volume, set_unmapped_volume,
};

#[derive(serde::Deserialize, Debug, Clone)]
struct Config {
    comm_port: Option<String>,
    #[serde(default)]
    slider: Vec<SliderMappings>,
    volume_step: f64,
}

#[derive(serde::Deserialize, Debug, Clone)]
struct SliderMappings {
    id: u8,
    #[serde(default)]
    target: VolumeTarget,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
enum VolumeTarget {
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

fn get_port(comm_port: &Option<String>) -> Result<Box<dyn SerialPort>, Box<dyn std::error::Error>> {
    match comm_port {
        Some(port_name) => {
            info!("Connecting to specified port: {}...", port_name);
            let port = serialport::new(port_name, 57600)
                .timeout(std::time::Duration::from_millis(30_000))
                .open()?;
            Ok(port)
        }
        None => {
            info!("No port specified in config, trying all serial ports...");
            let ports = serialport::available_ports()?;
            let arduino_port = ports
                .iter()
                .find(|p| match &p.port_type {
                    SerialPortType::UsbPort(_info) => {
                        info!("Found USB device on {}", p.port_name);
                        true
                    }
                    _ => false,
                })
                .ok_or("No Arduino found! Is it plugged in?")?;

            info!("Connecting to {}...", arduino_port.port_name);

            let port = serialport::new(&arduino_port.port_name, 57600)
                .timeout(std::time::Duration::from_millis(30_000))
                .open()?;

            Ok(port)
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();

    volume::windows_init()?;

    let filename = "gain.toml";
    let contents = match fs::read_to_string(filename) {
        Ok(c) => c,
        Err(_) => {
            error!(
                "Config file '{}' not found. Please create it based on 'gain.example.toml'.",
                filename
            );
            return Err(format!("Failed to read config file: {}", filename).into());
        }
    };
    let config: Config = match toml::from_str(&contents) {
        Ok(d) => d,
        Err(e) => {
            error!("Failed to parse config file '{}': {}", filename, e);
            return Err(format!("Failed to parse config file: {}", e).into());
        }
    };

    let port = get_port(&config.comm_port)?;

    let mappings: HashMap<u8, SliderMappings> = config
        .slider
        .clone()
        .into_iter()
        .map(|s| (s.id, s))
        .collect();

    let mapped_apps: Vec<_> = mappings
        .values()
        .filter_map(|mapping| match &mapping.target {
            VolumeTarget::Apps(apps) => Some(apps),
            _ => None,
        })
        .flatten()
        .collect();

    let mut reader = BufReader::new(port);
    let mut buffer = Vec::new();

    info!("Listening for slider data...");

    loop {
        buffer.clear();

        match reader.read_until(0x00, &mut buffer) {
            Ok(bytes_read) if bytes_read > 0 => {
                if let Some(&0x00) = buffer.last() {
                    buffer.pop(); // Remove the null terminator
                }

                match postcard::from_bytes_cobs::<Slider>(&mut buffer) {
                    Ok(slider) => {
                        manage_slider(slider, &config, &mappings, &mapped_apps);
                    }
                    Err(e) => {
                        warn!("Failed to deserialize slider data: {}", e);
                    }
                }
            }
            Ok(_) => continue, // No data read
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(e) => {
                info!("Error reading from serial port: {}", e);
            }
        }
    }
}

fn manage_slider(
    slider: Slider,
    config: &Config,
    mappings: &HashMap<u8, SliderMappings>,
    mapped_apps: &Vec<&String>,
) {
    let multiplier = 1.0 / config.volume_step;
    let raw_val = slider.value as f64 / 1023.0;
    let adjusted_value = (raw_val * multiplier).round() / multiplier;
    let final_vol = adjusted_value.max(0.0).min(1.0);

    match mappings.get(&slider.id) {
        Some(mapping) => match &mapping.target {
            VolumeTarget::Master => set_master_volume(final_vol),
            VolumeTarget::CurrentApp => set_current_app_volume(final_vol),
            VolumeTarget::Unmapped => set_unmapped_volume(final_vol, mapped_apps),
            VolumeTarget::Apps(apps) => {
                for app in apps {
                    set_app_volume(app, final_vol);
                }
            }
        },
        None => {
            trace!("No mapping found for slider ID {}", slider.id);
        }
    }
}
