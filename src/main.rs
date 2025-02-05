use anyhow;
use esp_bluedroid::ble;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::prelude::Peripherals;

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let Ok(peripherals) = Peripherals::take() else {
        log::error!("Failed to take peripherals");
        return;
    };

    let Ok(ble) = ble::Ble::new(peripherals.modem) else {
        log::error!("Failed to create BLE instance");
        return;
    };

    log::info!("Hello, world! 123");

    if let Err(err) = ble.start_advertising() {
        log::error!("Failed to start advertising: {:?}", err);
        return;
    }

    log::info!("Advertising started");

    loop {
        FreeRtos::delay_ms(1000);
    }
}
