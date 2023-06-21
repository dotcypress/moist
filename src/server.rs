use crate::*;

impl I2CPeripheral for I2cDev {
    type Error = i2c::Error;

    fn poll(&mut self) -> Result<Option<BusEvent>, Self::Error> {
        self.slave_addressed().map(|x| {
            x.map(|(addr, dir)| match dir {
                i2c::I2cDirection::MasterWriteSlaveRead => BusEvent::Rx(addr as _),
                i2c::I2cDirection::MasterReadSlaveWrite => BusEvent::Tx(addr as _),
            })
        })
    }

    fn rx(&mut self, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.slave_sbc(false);
        self.slave_read(bytes)
    }

    fn tx(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.slave_sbc(true);
        self.slave_write(bytes)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.clear_irq(i2c::Event::Rxne);
        self.clear_irq(i2c::Event::AddressMatch);
        Ok(())
    }
}

pub mod command {
    const MOISTURE_ADDR: u8 = 0x00;
    pub const READ_MOISTURE: u8 = MOISTURE_ADDR;
    pub const READ_MOISTURE_RAW: u8 = MOISTURE_ADDR + 1;
    pub const READ_MOISTURE_OFFSET: u8 = MOISTURE_ADDR + 2;
    pub const READ_MOISTURE_SLOPE: u8 = MOISTURE_ADDR + 3;
    pub const WRITE_MOISTURE_OFFSET: u8 = MOISTURE_ADDR + 4;
    pub const WRITE_MOISTURE_SLOPE: u8 = MOISTURE_ADDR + 5;

    const ILLUMINANCE_ADDR: u8 = 0x08;
    pub const READ_ILLUMINANCE: u8 = ILLUMINANCE_ADDR;
    pub const READ_ILLUMINANCE_RAW: u8 = ILLUMINANCE_ADDR + 1;
    pub const READ_ILLUMINANCE_OFFSET: u8 = ILLUMINANCE_ADDR + 2;
    pub const READ_ILLUMINANCE_SLOPE: u8 = ILLUMINANCE_ADDR + 3;
    pub const WRITE_ILLUMINANCE_OFFSET: u8 = ILLUMINANCE_ADDR + 4;
    pub const WRITE_ILLUMINANCE_SLOPE: u8 = ILLUMINANCE_ADDR + 5;

    const LED_ADDR: u8 = 0x10;
    pub const READ_LED: u8 = LED_ADDR;
    pub const WRITE_LED: u8 = LED_ADDR + 1;

    pub const SAVE_NVM: u8 = 0xff;
}
