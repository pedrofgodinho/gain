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
**Prerequisites**:
- [Rust](https://www.rust-lang.org/tools/install) (for building from source)
- [avrdude](https://github.com/avrdudes/avrdude) (for flashing the firmware to the Arduino)
- [avr-gcc](https://gcc.gnu.org/wiki/avr-gcc) (for compiling the firmware)
- [ravedude](https://crates.io/crates/ravedude) (Rust wrapper for avrdude)

You can install the dependencies on Windows using [Scoop](https://scoop.sh/) and `cargo install`:

```powershell
scoop install avr-gcc
scoop install avrdude
cargo install ravedude
```

**Build & Flash**:
Navigate to the `gain-arduino` directory and run:

```bash
cargo run --release
```

## ⚙️ Configuration
Gain looks for a configuration file in two locations (in order of priority):
1. The path provided as the first command line argument to the desktop application.
2. `config.toml` in the same directory as the executable.

### Configuration Options
| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `connection.com_port` | String | N/A | The serial port to which the Arduino is connected. |
| `connection.baud_rate` | Integer | 57600 | The baud rate for serial communication. |
| `connection.vid_filter` | u16 | N/A | If specified, filters com devices by vendor ID. |
| `connection.pid_filter` | u16 | N/A | If specified, filters com devices by product ID. |
| `connection.serial_number_filter` | String | N/A | If specified, filters com devices by serial number. |
| `connection.manufacturer_filter` | String | N/A | If specified, filters com devices by manufacturer name. |
| `connection.product_filter` | String | N/A | If specified, filters com devices by product name. |
| `general.volume_step` | Float | 0.01 | The granularity of volume changes. Values from the hardware mixer will be rounded to the nearest multiple of this value. |
| `general.invert_direction` | Boolean | false | If true, inverts the slider direction (i.e., turning the potentiometer clockwise decreases volume). |
| `slider.id` | Integer | N/A | The ID of the slider, starting from 0. |
| `slider.target` | String or Table | N/A | The target controlled by this slider (`master`, `current`, `unmapped`, or a table specifying multiple applications). |

### Example Configuration File

```toml
[connection]
# If com_port is not specified, the first port that passes all filters will be used
# com_port = "COM3"
# If baud_rate is not specified, 57600 will be used
baud_rate = 57600
serial_number_filter = "my_serial_number"

[general]
# The values from the hardware mixer will be rounded to the nearest multiple of volume_step. If your potentiometers are very noisy, you may want to increase this value.
volume_step = 0.01
# If true, inverts the slider direction (i.e., turning the potentiometer clockwise decreases volume)
invert_direction = false

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
