use anyhow;
use esp_bluedroid::ble;
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
}
