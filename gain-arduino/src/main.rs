#![no_std]
#![no_main]

use arduino_hal::prelude::*;
use gain_lib::Slider;
use panic_halt as _;
use postcard::to_slice_cobs;

// Config
const PINS_TO_READ: [usize; 6] = [0, 1, 2, 3, 4, 5];
const HYSTERESIS_THRESHOLD: i16 = 4;

#[derive(Clone, Copy)]
struct Potentiometer {
    accumulator: u32,
    last_stable_val: u16,
}

impl Potentiometer {
    fn new() -> Self {
        Self {
            accumulator: 0,
            last_stable_val: 0,
        }
    }

    fn update(&mut self, raw_input: u16) -> u16 {
        // EMA Filter
        if self.accumulator == 0 {
            self.accumulator = (raw_input as u32) << 1;
        } else {
            self.accumulator = self.accumulator - (self.accumulator >> 1) + raw_input as u32;
        }

        let smoothed_raw = (self.accumulator >> 1) as u16;

        // Hysteresis
        let diff = (smoothed_raw as i16 - self.last_stable_val as i16).abs();

        if diff > HYSTERESIS_THRESHOLD {
            self.last_stable_val = smoothed_raw;
        }

        // Edge Clamping
        if self.last_stable_val > 1018 {
            1023
        } else if self.last_stable_val < 5 {
            0
        } else {
            self.last_stable_val
        }
    }
}

#[arduino_hal::entry]
fn main() -> ! {
    let dp = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(dp);
    let mut serial = arduino_hal::default_serial!(dp, pins, 57600);

    let mut adc = arduino_hal::Adc::new(dp.ADC, Default::default());
    let a0 = pins.a0.into_analog_input(&mut adc);
    let a1 = pins.a1.into_analog_input(&mut adc);
    let a2 = pins.a2.into_analog_input(&mut adc);
    let a3 = pins.a3.into_analog_input(&mut adc);
    let a4 = pins.a4.into_analog_input(&mut adc);
    let a5 = pins.a5.into_analog_input(&mut adc);

    let mut pots = [Potentiometer::new(); 6];
    let mut last_output_values = [0u16; 6];

    let mut buf = [0; core::mem::size_of::<Slider>() * 2];

    loop {
        arduino_hal::delay_ms(25);

        let raw_reads = [
            a0.analog_read(&mut adc),
            a1.analog_read(&mut adc),
            a2.analog_read(&mut adc),
            a3.analog_read(&mut adc),
            a4.analog_read(&mut adc),
            a5.analog_read(&mut adc),
        ];

        let current_output_values: [u16; 6] = [
            pots[0].update(raw_reads[0]),
            pots[1].update(raw_reads[1]),
            pots[2].update(raw_reads[2]),
            pots[3].update(raw_reads[3]),
            pots[4].update(raw_reads[4]),
            pots[5].update(raw_reads[5]),
        ];

        for (i, &new_val) in current_output_values.iter().enumerate() {
            if !PINS_TO_READ.contains(&i) {
                continue;
            }

            if new_val != last_output_values[i] {
                last_output_values[i] = new_val;

                let slider = Slider {
                    id: i as u8,
                    value: new_val,
                };

                match to_slice_cobs(&slider, &mut buf) {
                    Ok(encoded_data) => {
                        for &mut byte in encoded_data {
                            nb::block!(serial.write(byte)).unwrap();
                        }
                    }
                    Err(_) => {
                        // Buffer error
                    }
                }
            }
        }
    }
}
