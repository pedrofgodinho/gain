# 🎚️ Gain

Gain is a physical hardware audio mixer interface powered by an **Arduino Uno** and written in **Rust**. It allows you to control audio levels for specific applications, the currently focused window, or the master volume on Windows using physical sliders/potentiometers.

## ✨ Features
- **Application Specific Control**: Bind a physical slider to specific apps (e.g., Spotify, Discord).
- **Context Aware**: Control the volume of the currently focused application.
- **Master Volume**: Direct control over the system audio.
- **Smart Fallback**: Map a slider to "unmapped" apps (any app not explicitly controlled by another slider).
- **Jitter Free**: Firmware implements an EMA (Exponential Moving Average) filter to smooth out potentiometer noise.
- **High Performance**: Desktop client built with Rust. Firmware only sends updates when changes need to be made. 

## 🛠️ Installation
### Desktop Application
**Prerequisites**:

- [Rust](https://www.rust-lang.org/tools/install) (for building from source)

**Build**: Run a cargo build command in the repository root:

```bash
cargo build --release
```

### Arduino Firmware
TODO: Instructions for building and uploading the Arduino firmware.


## ⚙️ Configuration
Gain looks for a configuration file in two locations (in order of priority):
1. The path provided as the first command line argument to the desktop application.
2. `config.toml` in the same directory as the executable.

### Configuration Options
| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `comm_port` | String | First available port | The serial port to which the Arduino is connected. |
| `baud_rate` | Integer | 57600 | The baud rate for serial communication. |
| `volume_step` | Float | 0.01 | The granularity of volume changes. Values from the hardware mixer will be rounded to the nearest multiple of this value. |
| `slider` | Table Array | N/A | An array of slider configurations. Each slider configuration specifies the ID and target for a slider. |
| `slider.id` | Integer | N/A | The ID of the slider, starting from 0. |
| `slider.target` | String or Table | N/A | The target controlled by this slider (`master`, `current`, `unmapped`, or a table specifying multiple applications). |

### Example Configuration File

```toml
# If comm_port is not specified, the first port found will be used
comm_port = "COM3"
# If baud_rate is not specified, 57600 will be used
baud_rate = 57600
# The values from the hardware mixer will be rounded to the nearest multiple of volume_step. If your potentiometers are very noisy, you may want to increase this value.
volume_step = 0.01

[[slider]]
# The ID of the slider, starting from 0
id = 0
# The target that is controller by this slider. `master` controls the master volume
target = "master"

[[slider]]
id = 1
# `current` controls the system's current focused application volume
target = "current"

[[slider]]
id = 2
# You can also specify multiple applications for a single slider
target = { apps = ["spotify.exe", "firefox.exe"] }

[[slider]]
id = 3
# `unmapped` controls the volume of all applications that are not mapped to any other slider
target = "unmapped"
```

## 🧠 Under the Hood

The Arduino firmware reads potentiometer values via the analog pins. It applies **EMA filtering** to smooth out the readings, and only sends updates when a significant change is detected. The updates are serialized using the [postcard](https://crates.io/crates/postcard) crate and sent over serial to the desktop application.

### Repository Structure
- `gain-arduino/`: Contains the Arduino firmware code.
- `gain-bin/`: Contains the Rust desktop application code.
- `gain-lib/`: Contains the structures that are serialized and shared between the firmware and desktop application.
