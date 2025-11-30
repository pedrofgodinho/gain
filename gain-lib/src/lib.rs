#![no_std]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slider {
    pub id: u8,
    pub value: u16,
}
