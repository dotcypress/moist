use crate::*;

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub moisture: SensorConfig,
    pub illuminance: SensorConfig,
}

impl Config {
    pub fn new(moisture: SensorConfig, illuminance: SensorConfig) -> Self {
        Self {
            moisture,
            illuminance,
        }
    }

    pub const PAGE: flash::FlashPage = flash::FlashPage(31);
    pub fn load() -> Self {
        let addr = Self::PAGE.to_address();
        Self {
            moisture: SensorConfig::load(addr),
            illuminance: SensorConfig::load(addr + 4),
        }
    }

    pub fn save(self, flash: FLASH) -> FLASH {
        match flash.unlock() {
            Err(flash) => flash,
            Ok(mut unlocked) => {
                unlocked.erase_page(Config::PAGE).ok();
                let addr = Config::PAGE.to_address();
                unlocked.write(addr, &self.moisture.save()).ok();
                unlocked.write(addr + 4, &self.illuminance.save()).ok();
                unlocked.lock()
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SensorConfig {
    pub offset: u16,
    pub slope: u16,
}

impl SensorConfig {
    pub fn load(addr: usize) -> Self {
        let [offset, mut slope] = unsafe { core::ptr::read(addr as *const [u16; 2]) };
        if slope == 0 {
            slope = 1024;
        }
        Self { offset, slope }
    }

    pub fn save(self) -> [u8; 4] {
        ((self.slope as u32) << 16 | self.offset as u32).to_le_bytes()
    }
}
