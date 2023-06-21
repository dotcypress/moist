#![no_std]
#![no_main]

extern crate panic_halt;
extern crate rtic;
extern crate stm32g0xx_hal as hal;

mod app;
mod config;
mod server;

use defmt_rtt as _;

use app::*;
use config::*;
use hal::analog::adc;
use hal::flash::{self, WriteErase};
use hal::gpio::*;
use hal::i2c;
use hal::pac::FLASH;
use hal::prelude::*;
use hal::stm32;
use hal::timer::pwm::PwmPin;
use hal::timer::*;

pub const ADDRESS: Address = 0x22;

pub type I2cClk = PB8<Output<OpenDrain>>;
pub type I2cSda = PB9<Output<OpenDrain>>;
pub type I2cDev = i2c::I2c<stm32::I2C1, I2cSda, I2cClk>;
pub type Sense = (adc::Adc, PA5<Analog>, PA1<Analog>);
pub type Led = (
    PwmPin<stm32::TIM3, Channel3>,
    PwmPin<stm32::TIM3, Channel2>,
    PwmPin<stm32::TIM3, Channel1>,
);

#[rtic::app(device = hal::stm32, peripherals = true, dispatchers = [CEC, PVD])]
mod moist {
    use super::*;

    #[shared]
    struct Shared {
        app: App,
    }

    #[local]
    struct Local {
        sense: Sense,
        i2c: I2cDev,
        led: Led,
        timer: Timer<stm32::TIM16>,
        flash: Option<stm32::FLASH>,
    }

    #[init]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        defmt::info!("init");

        let flash = Some(ctx.device.FLASH);
        let mut rcc = ctx.device.RCC.constrain();

        let port_a = ctx.device.GPIOA.split(&mut rcc);
        let port_b = ctx.device.GPIOB.split(&mut rcc);

        let mut i2c_cfg = i2c::Config::new(400.kHz());
        i2c_cfg.slave_address(ADDRESS);

        let mut i2c = ctx.device.I2C1.i2c(
            port_b.pb9.into_open_drain_output(),
            port_b.pb8.into_open_drain_output(),
            i2c_cfg,
            &mut rcc,
        );
        i2c.listen(i2c::Event::AddressMatch);

        let moisture_sense = port_a.pa5;
        let photo_sense = port_a.pa1;
        let mut adc = ctx.device.ADC.constrain(&mut rcc);
        adc.set_sample_time(adc::SampleTime::T_160);
        adc.set_precision(adc::Precision::B_12);
        adc.set_oversampling_ratio(adc::OversamplingRatio::X_16);
        adc.set_oversampling_shift(16);
        adc.oversampling_enable(true);

        let pwm = ctx.device.TIM14.pwm(200.kHz(), &mut rcc);
        let mut pwm = pwm.bind_pin(port_a.pa4);
        pwm.set_duty(pwm.get_max_duty() / 8);
        pwm.enable();

        let cfg = Config::load();
        let app = App::new(cfg);

        let mut timer = ctx.device.TIM16.timer(&mut rcc);
        timer.start(100.millis());
        timer.listen();

        let led_pwm = ctx.device.TIM3.pwm(1.kHz(), &mut rcc);
        let mut pwm_r = led_pwm.bind_pin(port_b.pb0);
        let mut pwm_g = led_pwm.bind_pin(port_a.pa7);
        let mut pwm_b = led_pwm.bind_pin(port_a.pa6);
        let duty = pwm_r.get_max_duty() + 1;
        pwm_r.set_duty(duty);
        pwm_g.set_duty(duty);
        pwm_b.set_duty(duty);
        pwm_r.enable();
        pwm_g.enable();
        pwm_b.enable();

        let led = (pwm_r, pwm_g, pwm_b);

        defmt::info!("init completed");
        (
            Shared { app },
            Local {
                i2c,
                timer,
                led,
                flash,
                sense: (adc, moisture_sense, photo_sense),
            },
            init::Monotonics(),
        )
    }

    #[task(binds = TIM16, local = [timer, sense], shared = [app])]
    fn timer_tick(ctx: timer_tick::Context) {
        let timer_tick::LocalResources { timer, sense } = ctx.local;
        let mut app = ctx.shared.app;
        let moisture = u16::MAX - sense.0.read(&mut sense.1).unwrap_or(0);
        let illuminance = sense.0.read(&mut sense.2).unwrap_or(0);
        app.lock(|app| app.push_samples(moisture, illuminance));

        timer.clear_irq();
    }

    #[task(priority = 2, binds = I2C1, local = [i2c], shared=[app])]
    fn i2c_rx(ctx: i2c_rx::Context) {
        let mut app = ctx.shared.app;
        match app.lock(|app| app.poll(ctx.local.i2c)) {
            Ok(Some(AppRequest::SetLedColor(color))) => {
                update_led::spawn(color).ok();
            }
            Ok(Some(AppRequest::SaveConfig(cfg))) => {
                save_nvm::spawn(cfg).ok();
            }
            Err(err) => {
                match err {
                    i2c::Error::Overrun => defmt::error!("Overrun"),
                    i2c::Error::Nack => defmt::error!("Nack"),
                    i2c::Error::PECError => defmt::error!("PEC Fault"),
                    i2c::Error::BusError => defmt::error!("Bus Fault"),
                    i2c::Error::ArbitrationLost => defmt::error!("Arbitration lost"),
                    i2c::Error::IncorrectFrameSize(s) => {
                        defmt::error!("Incorrect frame size: {}", s)
                    }
                };
            }
            _ => (),
        }
    }

    #[task(priority = 2, local = [led])]
    fn update_led(ctx: update_led::Context, color: [u8; 3]) {
        let max = ctx.local.led.0.get_max_duty() as u32 + 1;
        ctx.local.led.0.set_duty(max - color[0] as u32 * 256);
        ctx.local.led.1.set_duty(max - color[1] as u32 * 256);
        ctx.local.led.2.set_duty(max - color[2] as u32 * 256);
    }

    #[task(priority = 3, local = [flash])]
    fn save_nvm(ctx: save_nvm::Context, cfg: Config) {
        if let Some(flash) = ctx.local.flash.take() {
            *ctx.local.flash = Some(cfg.save(flash));
        }
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            rtic::export::nop();
        }
    }
}
