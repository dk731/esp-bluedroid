use esp_bluedroid::ble;
use esp_idf_svc::hal::prelude::Peripherals;

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    // example::example::main().unwrap_or_else(|err| {
    //     log::error!("Error in example: {:?}", err);
    // });

    let Ok(peripherals) = Peripherals::take() else {
        log::error!("Failed to take peripherals");
        return;
    };

    let Ok(ble) = ble::Ble::new(peripherals.modem) else {
        log::error!("Failed to create BLE instance");
        return;
    };

    if let Err(err) = ble.gap.start_advertising() {
        log::error!("Failed to start advertising: {:?}", err);
        return;
    }
    log::info!("Started advertising");

    let Ok(app) = ble.gatts.register_app(1) else {
        log::error!("Failed to register GATT application");
        return;
    };

    log::info!(
        "Registered GATT application with ID {:?}",
        app.id().unwrap()
    );

    loop {
        std::thread::sleep(std::time::Duration::from_secs(10));
        log::info!("Still running...");
    }
}
