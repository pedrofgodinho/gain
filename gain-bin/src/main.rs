mod config;
mod volume;

use anyhow::{Result, anyhow};
use gain_lib::Slider;
use log::{error, info, trace, warn};
use serialport::{SerialPort, SerialPortType};
use std::{
    io::{BufRead, BufReader},
    time::Duration,
};

use crate::{
    config::{LoadedConfig, VolumeTarget},
    volume::{set_app_volume, set_current_app_volume, set_master_volume, set_unmapped_volume},
};

fn main() -> Result<()> {
    pretty_env_logger::init();
    volume::windows_init()?;

    let mut config = LoadedConfig::new_from_file("gain.toml")?;

    loop {
        if let Err(e) = config.reload_if_needed("gain.toml") {
            warn!("Failed to reload config: {}", e);
        }

        let port_name_result = resolve_port_name(&config.config.comm_port);

        match port_name_result {
            Ok(name) => {
                info!("Connecting to {}...", name);

                match serialport::new(&name, config.config.baud_rate)
                    .timeout(Duration::from_secs(30))
                    .open()
                {
                    Ok(port) => {
                        if let Err(e) = process_serial_stream(port, &mut config) {
                            error!("Serial connection lost: {}", e);
                        }
                    }
                    Err(e) => warn!("Failed to open port {}: {}", name, e),
                }
            }
            Err(e) => warn!("Port detection failed: {}", e),
        }

        std::thread::sleep(Duration::from_secs(5));
    }
}

fn resolve_port_name(configured_port: &Option<String>) -> Result<String> {
    match configured_port {
        Some(name) => Ok(name.clone()),
        None => {
            info!("No port specified, scanning for USB devices...");
            let ports = serialport::available_ports()?;

            ports
                .into_iter()
                .find(|p| matches!(p.port_type, SerialPortType::UsbPort(_)))
                .map(|p| {
                    info!("Found USB device on {}", p.port_name);
                    p.port_name
                })
                .ok_or_else(|| anyhow!("No USB serial device found"))
        }
    }
}

fn process_serial_stream(port: Box<dyn SerialPort>, config: &mut LoadedConfig) -> Result<()> {
    let mut reader = BufReader::new(port);
    let mut buffer = Vec::new();

    info!("Listening for slider data...");

    loop {
        buffer.clear();

        match reader.read_until(0x00, &mut buffer) {
            Ok(bytes_read) if bytes_read > 0 => {
                if let Err(e) = config.reload_if_needed("gain.toml") {
                    warn!("Config reload failed: {}", e);
                }

                if buffer.last() == Some(&0x00) {
                    buffer.pop();
                }

                match postcard::from_bytes_cobs::<Slider>(&mut buffer) {
                    Ok(slider) => {
                        if let Err(e) = manage_slider(slider, config) {
                            warn!("Logic Error: {}", e);
                        }
                    }
                    Err(e) => warn!("Deserialization failed: {}", e),
                }
            }
            Ok(_) => continue, // 0 bytes read, just loop
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(e) => return Err(e.into()), // Critical IO error, break the loop to reconnect
        }
    }
}

fn manage_slider(slider: Slider, config: &LoadedConfig) -> Result<()> {
    let step = config.config.volume_step;
    let raw_percent = slider.value as f64 / 1023.0;

    // Snap to nearest step (e.g., if step is 0.05, snaps to 0.00, 0.05, 0.10)
    let quantized = (raw_percent / step).round() * step;
    let final_vol = quantized.clamp(0.0, 1.0);

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
            trace!("Unmapped slider ID: {}", slider.id);
            Ok(())
        }
    }
}
