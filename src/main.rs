use esp_bluedroid::ble;
use esp_idf_svc::{
    bt::{
        ble::gatt::{GattId, GattServiceId},
        BtUuid,
    },
    hal::prelude::Peripherals,
};

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

    let Ok(app1) = ble.gatts.register_app(1) else {
        log::error!("Failed to register GATT application");
        return;
    };
    log::info!("Registered GATT application with ID {:?}", app1.0);

    let Ok(app2) = ble.gatts.register_app(2) else {
        log::error!("Failed to register GATT application");
        return;
    };
    log::info!("Registered GATT application with ID {:?}", app2.0);

    let Ok(service1) = app1.register_service(
        GattServiceId {
            id: GattId {
                uuid: BtUuid::uuid128(0x12345678901234567890123456789012),
                inst_id: 0,
            },
            is_primary: true,
        },
        10,
    ) else {
        log::error!("Failed to register service 1");
        return;
    };
    log::info!("Registered service 1 with UUID {:?}", service1.0.service_id);

    let Ok(service2) = app1.register_service(
        GattServiceId {
            id: GattId {
                uuid: BtUuid::uuid128(0x12345678901234567890123456789013),
                inst_id: 0,
            },
            is_primary: true,
        },
        20,
    ) else {
        log::error!("Failed to register service 1");
        return;
    };
    log::info!("Registered service 2 with UUID {:?}", service2.0.service_id);

    loop {
        std::thread::sleep(std::time::Duration::from_secs(10));
        log::info!("Still running...");
    }
}
