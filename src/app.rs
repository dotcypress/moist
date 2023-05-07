use crate::config::*;
use crate::server::command;

pub type Address = u8;

pub enum BusEvent {
    Rx(Address),
    Tx(Address),
}

pub trait I2CPeripheral {
    type Error;

    fn poll(&mut self) -> Result<Option<BusEvent>, Self::Error>;

    fn rx(&mut self, bytes: &mut [u8]) -> Result<(), Self::Error>;

    fn tx(&mut self, bytes: &[u8]) -> Result<(), Self::Error>;

    fn flush(&mut self) -> Result<(), Self::Error>;
}

pub struct SensorState {
    raw: u16,
    offset: u16,
    slope: u16,
}

impl Default for SensorState {
    fn default() -> Self {
        Self::new()
    }
}

impl SensorState {
    pub fn new() -> Self {
        Self {
            raw: 0,
            offset: 0,
            slope: 0,
        }
    }

    pub fn val(&self) -> u16 {
        let val = self.raw.saturating_sub(self.offset) as u32 * 1024 / self.slope as u32;
        val.min(u16::MAX as _) as _
    }

    pub fn raw(&self) -> u16 {
        self.raw
    }

    pub fn set_raw(&mut self, val: u16) {
        self.raw = val;
    }

    pub fn offset(&self) -> u16 {
        self.offset
    }

    pub fn slope(&self) -> u16 {
        self.slope
    }

    pub fn set_slope(&mut self, slope: u16) {
        self.slope = slope;
    }

    pub fn set_offset(&mut self, offset: u16) {
        self.offset = offset;
    }
}

pub struct App {
    moisture: SensorState,
    illuminance: SensorState,
    write_pend: Option<u8>,
    scratch: [u8; 2],
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            moisture: SensorState::new(),
            illuminance: SensorState::new(),
            write_pend: None,
            scratch: [0; 2],
        }
    }

    pub fn push_samples(&mut self, moisture: u16, illuminance: u16) {
        self.moisture.set_raw(moisture);
        self.illuminance.set_raw(illuminance);
        defmt::info!("{}\t\t{}", self.moisture.val() >> 8, self.moisture.raw());
    }

    pub fn poll_i2c<E>(
        &mut self,
        i2c: &mut dyn I2CPeripheral<Error = E>,
    ) -> Result<Option<Config>, E> {
        loop {
            match i2c.poll()? {
                None => {
                    i2c.flush()?;
                    break Ok(None);
                }
                Some(BusEvent::Tx(_)) => {
                    i2c.tx(&self.scratch)?;
                }
                Some(BusEvent::Rx(_)) => {
                    if let Some(cmd) = self.write_pend {
                        self.write_pend = None;

                        let mut inbox = [0; 2];
                        i2c.rx(&mut inbox)?;
                        let val = u16::from_be_bytes(inbox);

                        match cmd {
                            command::WRITE_MOISTURE_OFFSET => self.moisture.set_offset(val),
                            command::WRITE_MOISTURE_SLOPE => self.moisture.set_slope(val),
                            command::WRITE_ILLUMINANCE_OFFSET => self.illuminance.set_offset(val),
                            command::WRITE_ILLUMINANCE_SLOPE => self.illuminance.set_slope(val),
                            _ => unreachable!(),
                        }
                    } else {
                        let mut inbox = [0; 1];
                        i2c.rx(&mut inbox)?;
                        let cmd = inbox[0];

                        self.scratch = match cmd {
                            command::WRITE_MOISTURE_OFFSET
                            | command::WRITE_MOISTURE_SLOPE
                            | command::WRITE_ILLUMINANCE_OFFSET
                            | command::WRITE_ILLUMINANCE_SLOPE => {
                                self.write_pend = Some(cmd);
                                continue;
                            }
                            command::READ_MOISTURE => self.moisture.val().to_be_bytes(),
                            command::READ_MOISTURE_RAW => self.moisture.raw().to_be_bytes(),
                            command::READ_MOISTURE_OFFSET => self.moisture.offset().to_be_bytes(),
                            command::READ_MOISTURE_SLOPE => self.moisture.slope().to_be_bytes(),
                            command::READ_ILLUMINANCE => self.illuminance.val().to_be_bytes(),
                            command::READ_ILLUMINANCE_RAW => self.illuminance.raw().to_be_bytes(),
                            command::READ_ILLUMINANCE_OFFSET => {
                                self.illuminance.offset().to_be_bytes()
                            }
                            command::READ_ILLUMINANCE_SLOPE => {
                                self.illuminance.slope().to_be_bytes()
                            }
                            command::SAVE_NVM => {
                                let cfg =
                                    Config::new(self.moisture.offset(), self.moisture.slope());
                                break Ok(Some(cfg));
                            }
                            _ => self.moisture.val().to_be_bytes(),
                        }
                    }
                }
            }
        }
    }

    pub fn moisture(&mut self) -> &mut SensorState {
        &mut self.moisture
    }

    pub fn illuminance(&mut self) -> &mut SensorState {
        &mut self.illuminance
    }
}
