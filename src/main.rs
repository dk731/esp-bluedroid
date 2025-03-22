use bincode::{Decode, Encode};
use esp_bluedroid::{ble, gatts::characteristic::CharacteristicConfig};
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
            value_max_len: 100,
            readable: true,
            writable: true,
            broadcasted: true,
            notifiable: true,
            indicateable: true,
        },
        vec![
            0x0000, 0xffff, 0x0002, 0xffff, 0x0004, 0xffff, 0x0006, 0xffff,
        ],
    )?;

    service.start()?;
    ble.gap.start_advertising()?;

    loop {
        std::thread::sleep(std::time::Duration::from_secs(10));
        log::info!("Still running...");
    }

    Ok(())
}
