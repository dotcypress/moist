use crate::*;

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub offset: u16,
    pub slope: u16,
}

impl Config {
    pub const PAGE: flash::FlashPage = flash::FlashPage(31);

    pub fn new(offset: u16, slope: u16) -> Self {
        Self { offset, slope }
    }

    pub fn load() -> Self {
        let addr = Self::PAGE.to_address();
        let [offset, mut slope] = unsafe { core::ptr::read(addr as *const [u16; 2]) };
        if slope == 0 {
            slope = 1024;
        }
        Self { slope, offset }
    }

    pub fn save(self, flash: FLASH) -> FLASH {
        let val = (self.slope as u32) << 16 | self.offset as u32;
        match flash.unlock() {
            Err(flash) => flash,
            Ok(mut unlocked) => {
                unlocked.erase_page(Config::PAGE).ok();
                let addr = Config::PAGE.to_address();
                unlocked.write(addr, &val.to_le_bytes()).ok();
                unlocked.lock()
            }
        }
    }
}
