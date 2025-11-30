mod config;
mod volume;

use anyhow::{Result, anyhow};
use gain_lib::Slider;
use log::{info, trace, warn};
use serialport::{SerialPort, SerialPortType};
use std::io::{BufRead, BufReader};

use crate::{
    config::{LoadedConfig, VolumeTarget},
    volume::{set_app_volume, set_current_app_volume, set_master_volume, set_unmapped_volume},
};

fn get_port(comm_port: &Option<String>) -> Result<Box<dyn SerialPort>> {
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
                .ok_or(anyhow!("No Arduino found! Is it plugged in?"))?;

            info!("Connecting to {}...", arduino_port.port_name);

            let port = serialport::new(&arduino_port.port_name, 57600)
                .timeout(std::time::Duration::from_millis(30_000))
                .open()?;

            Ok(port)
        }
    }
}

fn main() -> Result<()> {
    pretty_env_logger::init();
    volume::windows_init()?;

    let mut config = LoadedConfig::new_from_file("gain.toml")?;

    let port = get_port(&config.config.comm_port)?;

    let mut reader = BufReader::new(port);
    let mut buffer = Vec::new();

    info!("Listening for slider data...");

    loop {
        buffer.clear();

        match reader.read_until(0x00, &mut buffer) {
            Ok(bytes_read) if bytes_read > 0 => {
                if let Err(e) = config.reload_if_needed("gain.toml") {
                    warn!("Failed to reload config: {}", e);
                }
                if let Some(&0x00) = buffer.last() {
                    buffer.pop(); // Remove the null terminator
                }

                match postcard::from_bytes_cobs::<Slider>(&mut buffer) {
                    Ok(slider) => {
                        if let Err(e) = manage_slider(slider, &config) {
                            warn!("Failed to manage slider: {}", e);
                        }
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

fn manage_slider(slider: Slider, config: &LoadedConfig) -> Result<()> {
    let multiplier = 1.0 / config.config.volume_step;
    let raw_val = slider.value as f64 / 1023.0;
    let adjusted_value = (raw_val * multiplier).round() / multiplier;
    let final_vol = adjusted_value.max(0.0).min(1.0);

    match config.mappings.get(&slider.id) {
        Some(mapping) => match &mapping.target {
            VolumeTarget::Master => set_master_volume(final_vol),
            VolumeTarget::CurrentApp => set_current_app_volume(final_vol),
            VolumeTarget::Unmapped => set_unmapped_volume(final_vol, &config.mapped_apps),
            VolumeTarget::Apps(apps) => {
                for app in apps {
                    if let Err(e) = set_app_volume(app, final_vol) {
                        warn!("Failed to set volume for app {}: {}", app, e);
                    }
                }
                Ok(())
            }
        },
        None => {
            trace!("No mapping found for slider ID {}", slider.id);
            Ok(())
        }
    }
}
