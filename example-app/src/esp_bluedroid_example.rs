use esp_bluedroid::{
    ble,
    gap::GapConfig,
    gatts::{
        app::App,
        attribute::AttributeUpdate,
        characteristic::{Characteristic, CharacteristicConfig},
        service::Service,
    },
    svc::{
        bt::{
            BtUuid,
            ble::gatt::{GattId, GattServiceId},
        },
        hal::prelude::Peripherals,
    },
};
use esp_idf_svc::hal::{
    ledc::{LedcDriver, LedcTimerDriver, config::TimerConfig},
    units::Hertz,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct LedConfiguration {
    pwm_duty: f32,
    pwm_frequency: f32,
    enabled: bool,
}

pub fn main() -> anyhow::Result<()> {
    esp_bluedroid::svc::sys::link_patches();
    esp_bluedroid::svc::log::EspLogger::initialize_default();

    run_ble_example()?;

    Ok(())
}

fn run_ble_example() -> anyhow::Result<()> {
    let peripherals = Peripherals::take()?;
    let ble = ble::Ble::new(peripherals.modem)?;
    let app = ble.gatts.register_app(&App::new(0))?;

    let mut led_timer = LedcTimerDriver::new(peripherals.ledc.timer3, &TimerConfig::new())?;
    led_timer.set_frequency(Hertz::from(1000))?;
    led_timer.resume()?;

    let mut led_pwd = LedcDriver::new(
        peripherals.ledc.channel0,
        &led_timer,
        peripherals.pins.gpio5,
    )?;
    led_pwd.set_duty(led_pwd.get_max_duty() / 2)?;
    led_pwd.enable()?;

    let service = app.register_service(&Service::new(
        GattServiceId {
            id: GattId {
                uuid: BtUuid::uuid128(424242),
                inst_id: 0,
            },
            is_primary: true,
        },
        20,
    ))?;

    let leds_characteristic = service.register_characteristic(&Characteristic::new(
        LedConfiguration {
            pwm_duty: 0.5,
            pwm_frequency: 1000.0,
            enabled: true,
        },
        CharacteristicConfig {
            uuid: BtUuid::uuid128(42424242),
            value_max_len: 100,
            readable: true,
            writable: true,
            broadcasted: true,
            enable_notify: true,
            description: Some("LEDs Configuration".to_string()),
        },
        None,
    ))?;

    service.start()?;
    ble.gap.set_config(GapConfig {
        device_name: "esp-bluedroid LED Example".to_string(),
        max_connections: Some(3),
        manufacturer_data: Some("ESP-IDF".as_bytes().to_vec()),
        ..GapConfig::default()
    })?;
    ble.gap.start_advertising()?;

    for AttributeUpdate { new, .. } in leds_characteristic.0.attribute.updates_rx.iter() {
        log::info!("Received new LED configuration: {:?}", new);

        led_timer.set_frequency(Hertz(new.pwm_frequency as u32))?;
        led_pwd.set_duty((new.pwm_duty * led_pwd.get_max_duty() as f32) as u32)?;

        if new.enabled {
            led_pwd.enable()?;
        } else {
            led_pwd.disable()?;
        }
    }

    Ok(())
}
