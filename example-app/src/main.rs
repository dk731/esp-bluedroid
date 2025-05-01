use std::sync::Arc;

use esp_bluedroid::{
    ble,
    gap::GapConfig,
    gatts::{
        app::App,
        attribute::{
            AttributeUpdate,
            defaults::{BytesAttr, StringAttr, U8Attr, U16Attr, U32Attr},
        },
        characteristic::{Characteristic, CharacteristicConfig},
        descriptor::{Descriptor, DescriptorConfig},
        service::Service,
    },
    svc::{
        bt::{
            BtUuid,
            ble::{
                gap::AppearanceCategory,
                gatt::{GattId, GattServiceId},
            },
        },
        hal::prelude::Peripherals,
    },
};
use esp_bluedroid_logger::BleLoggerService;
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
    esp_bluedroid::svc::sys::link_patches();
    esp_bluedroid::svc::log::EspLogger::initialize_default();

    if let Err(e) = run_ble_example() {
        log::error!("Error: {:?}", e);
    }
}

fn run_ble_example() -> anyhow::Result<()> {
    let peripherals = Peripherals::take()?;
    let ble = ble::Ble::new(peripherals.modem)?;

    let app = ble.gatts.register_app(&App::new(0))?;
    let logger_service = BleLoggerService::new();

    app.register_service(&logger_service.service)?;

    let service = app.register_service(&Service::new(
        GattServiceId {
            id: GattId {
                uuid: BtUuid::uuid128(1),
                inst_id: 0,
            },
            is_primary: true,
        },
        20,
    ))?;

    let char1 = service.register_characteristic(&Characteristic::new(
        U16Attr(0),
        CharacteristicConfig {
            uuid: BtUuid::uuid128(2),
            value_max_len: 100,
            readable: true,
            writable: true,
            broadcasted: true,
            enable_notify: true,
            description: Some("Test characteristic".to_string()),
        },
        Some(vec![
            Arc::new(Descriptor::new(
                U32Attr(777),
                DescriptorConfig {
                    uuid: BtUuid::uuid128(123),
                    readable: true,
                    writable: true,
                },
            )),
            Arc::new(Descriptor::new(
                U32Attr(0),
                DescriptorConfig {
                    uuid: BtUuid::uuid128(124),
                    readable: true,
                    writable: true,
                },
            )),
            Arc::new(Descriptor::new(
                U8Attr(0),
                DescriptorConfig {
                    uuid: BtUuid::uuid128(125),
                    readable: true,
                    writable: true,
                },
            )),
            Arc::new(Descriptor::new(
                StringAttr("Hello world".to_string()),
                DescriptorConfig {
                    uuid: BtUuid::uuid128(126),
                    readable: true,
                    writable: true,
                },
            )),
            Arc::new(Descriptor::new(
                BytesAttr(vec![1, 2, 3, 5, 6]),
                DescriptorConfig {
                    uuid: BtUuid::uuid128(127),
                    readable: true,
                    writable: true,
                },
            )),
        ]),
    ))?;

    let char2 = service.register_characteristic(&Characteristic::new(
        CoolNestedChar {
            bar: "Hello".to_string(),
            foo_bar: FooBar {
                bar: "World".to_string(),
                foo_bar: "FooBar".to_string(),
            },
            temperature: 0,
            messages: vec!["Hello".to_string(), "World".to_string()],
        },
        CharacteristicConfig {
            uuid: BtUuid::uuid128(3),
            value_max_len: 100,
            readable: true,
            writable: true,
            broadcasted: true,
            enable_notify: true,
            description: Some("Test characteristic".to_string()),
        },
        None,
    ))?;

    let thread_char = char1.clone();
    std::thread::spawn(move || {
        for AttributeUpdate { old, new } in thread_char.0.attribute.updates_rx.iter() {
            log::info!("Characteristic was update.\tOld: {:?}\tNew: {:?}", old, new);
        }
    });

    service.start()?;
    ble.gap.set_config(GapConfig {
        device_name: "Supa Test Name Please Work".to_string(),
        max_connections: Some(6),
        service_uuid: Some(service.uuid()),
        service_data: Some(vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]),
        manufacturer_data: Some("Test Manufacturer".as_bytes().to_vec()),
        appearance: AppearanceCategory::Computer,
        ..GapConfig::default()
    })?;
    ble.gap.start_advertising()?;

    let mut i = 0;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(5));

        char1.update_value(U16Attr(i))?;
        i += 1;
    }

    Ok(())
}
