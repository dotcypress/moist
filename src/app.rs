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
    cfg: SensorConfig,
}

impl SensorState {
    pub fn new(cfg: SensorConfig) -> Self {
        Self { cfg, raw: 0 }
    }

    pub fn val(&self) -> u16 {
        let val = self.raw.saturating_sub(self.cfg.offset) as u32 * 1024 / self.cfg.slope as u32;
        val.min(u16::MAX as _) as _
    }

    pub fn raw(&self) -> u16 {
        self.raw
    }

    pub fn update(&mut self, val: u16) {
        self.raw = val;
    }
}

pub enum AppRequest {
    SetLedColor([u8; 3]),
    SaveConfig(Config),
}

pub struct App {
    moisture: SensorState,
    illuminance: SensorState,
    led_color: u16,
    write_pend: Option<u8>,
    scratch: [u8; 2],
}

impl App {
    pub fn new(cfg: Config) -> Self {
        Self {
            moisture: SensorState::new(cfg.moisture),
            illuminance: SensorState::new(cfg.illuminance),
            led_color: 0,
            write_pend: None,
            scratch: [0; 2],
        }
    }

    pub fn push_samples(&mut self, moisture: u16, illuminance: u16) {
        self.moisture.update(moisture);
        self.illuminance.update(illuminance);
    }

    pub fn poll<E>(
        &mut self,
        i2c: &mut dyn I2CPeripheral<Error = E>,
    ) -> Result<Option<AppRequest>, E> {
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
                            command::WRITE_MOISTURE_OFFSET => self.moisture.cfg.offset = val,
                            command::WRITE_MOISTURE_SLOPE => self.moisture.cfg.slope = val,
                            command::WRITE_ILLUMINANCE_OFFSET => self.illuminance.cfg.offset = val,
                            command::WRITE_ILLUMINANCE_SLOPE => self.illuminance.cfg.slope = val,
                            command::WRITE_LED => {
                                self.led_color = val;
                                let rgb = [
                                    self.led_color as u8 & 0x0f,
                                    (self.led_color >> 4) as u8 & 0x0f,
                                    (self.led_color >> 8) as u8 & 0x0f,
                                ];
                                break Ok(Some(AppRequest::SetLedColor(rgb)));
                            }
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
                            | command::WRITE_ILLUMINANCE_SLOPE
                            | command::WRITE_LED => {
                                self.write_pend = Some(cmd);
                                continue;
                            }
                            command::READ_MOISTURE => self.moisture.val().to_be_bytes(),
                            command::READ_MOISTURE_RAW => self.moisture.raw().to_be_bytes(),
                            command::READ_MOISTURE_OFFSET => self.moisture.cfg.offset.to_be_bytes(),
                            command::READ_MOISTURE_SLOPE => self.moisture.cfg.slope.to_be_bytes(),
                            command::READ_ILLUMINANCE => self.illuminance.val().to_be_bytes(),
                            command::READ_ILLUMINANCE_RAW => self.illuminance.raw().to_be_bytes(),
                            command::READ_ILLUMINANCE_OFFSET => {
                                self.illuminance.cfg.offset.to_be_bytes()
                            }
                            command::READ_ILLUMINANCE_SLOPE => {
                                self.illuminance.cfg.slope.to_be_bytes()
                            }
                            command::READ_LED => self.led_color.to_be_bytes(),
                            command::SAVE_NVM => {
                                let cfg = Config::new(self.moisture.cfg, self.illuminance.cfg);
                                break Ok(Some(AppRequest::SaveConfig(cfg)));
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
