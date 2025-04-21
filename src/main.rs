<<<<<<< HEAD
use esp_bluedroid::{
    ble,
    gatts::{
        app::App,
        characteristic::{Characteristic, CharacteristicConfig, CharacteristicUpdate},
        service::Service,
    },
};
use esp_idf_svc::{
    bt::{
        ble::gatt::{GattId, GattServiceId},
        BtUuid,
    },
    hal::prelude::Peripherals,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FooBar {
    bar: String,
    foo_bar: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CoolNestedChar {
    bar: String,
    foo_bar: FooBar,

    temperature: u16,
    messages: Vec<String>,
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    if let Err(e) = run_ble_example() {
        log::error!("Error: {:?}", e);
    }
}

fn run_ble_example() -> anyhow::Result<()> {
    let peripherals = Peripherals::take()?;
    let ble = ble::Ble::new(peripherals.modem)?;

    let app = ble.gatts.register_app(App::new(0))?;
    let service = app.register_service(Service::new(
        GattServiceId {
            id: GattId {
                uuid: BtUuid::uuid128(1),
                inst_id: 0,
            },
            is_primary: true,
        },
        10,
    ))?;

    let char1 = service.register_characteristic(Characteristic::new(
        CharacteristicConfig {
            uuid: BtUuid::uuid128(2),
            value_max_len: 100,
            readable: true,
            writable: true,
            broadcasted: true,
            notifiable: true,
            indicateable: true,
        },
        123,
    ))?;

    let thread_char = char1.clone();
    std::thread::spawn(move || {
        for CharacteristicUpdate { old, new } in thread_char.0.updates_rx.iter() {
            log::info!("Characteristic was update.\tOld: {:?}\tNew: {:?}", old, new);
        }
    });

    service.start()?;
    ble.gap.start_advertising()?;

    let mut i = 0;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));

        char1.update_value(i)?;
        i += 1;
    }

    Ok(())
}
=======
use esp_bluedroid::example;

fn main() {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    // esp_idf_svc::sys::link_patches();

    // // Bind the log crate to the ESP Logging facilities
    // esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Hello, world! 123");

    let result = example::main();
    if let Err(e) = result {
        log::error!("Error: {:?}", e);
    }

    // let mut i = 0;
    // loop {
    //     i += 1;
    //     log::info!("Hello, world! {}", i);
    //     std::thread::sleep(std::time::Duration::from_secs(1));
    // }
}
>>>>>>> 3f16bc07cb88de0e0153b806fb79d81afe2aa196
