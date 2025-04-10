use esp_bluedroid::{
    ble,
    gatts::characteristic::{CharacteristicConfig, CharacteristicUpdate},
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

    let app = ble.gatts.register_app(0)?;
    let service = app.register_service(
        GattServiceId {
            id: GattId {
                uuid: BtUuid::uuid128(1),
                inst_id: 0,
            },
            is_primary: true,
        },
        10,
    )?;

    let char1 = service.register_characteristic(
        CharacteristicConfig {
            uuid: BtUuid::uuid128(2),
            value_max_len: 100,
            readable: true,
            writable: true,
            broadcasted: true,
            notifiable: true,
            indicateable: true,
        },
        CoolNestedChar {
            bar: "bar".to_string(),
            foo_bar: FooBar {
                bar: "bar".to_string(),
                foo_bar: "foo_bar".to_string(),
            },
            temperature: 0,
            messages: vec!["Hello".to_string(), "World".to_string()],
        },
    )?;

    let char2 = service.register_characteristic(
        CharacteristicConfig {
            uuid: BtUuid::uuid128(3),
            value_max_len: 4096,
            readable: true,
            writable: true,
            broadcasted: true,
            notifiable: true,
            indicateable: true,
        },
        0x25252525u128,
    )?;

    let thread_char = char2.clone();
    std::thread::spawn(move || {
        for CharacteristicUpdate { old, new } in thread_char.0.updates_rx.iter() {
            log::info!("Characteristic was update. Old: {:?}   New: {:?}", old, new);
        }
    });

    service.start()?;
    ble.gap.start_advertising()?;

    let mut i = 0;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(10));

        char2.update_value(i)?;
        i += 1;
    }

    Ok(())
}
