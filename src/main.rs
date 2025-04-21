use esp_bluedroid::{
    ble,
    gatts::{
        app::App,
        attribute::{Attribute, AttributeUpdate, SerializableAttribute},
        characteristic::{Characteristic, CharacteristicConfig},
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

#[derive(Debug, Clone)]
struct Qwe(u8);
impl Attribute for Qwe {
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(vec![self.0])
    }

    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        if bytes.len() != 1 {
            return Err(anyhow::anyhow!("Invalid length for Qwe attribute"));
        }
        let value = bytes[0];
        Ok(Qwe(value))
    }
}

fn run_ble_example() -> anyhow::Result<()> {
    let peripherals = Peripherals::take()?;
    let ble = ble::Ble::new(peripherals.modem)?;

    let app = ble.gatts.register_app(&App::new(0))?;
    let service = app.register_service(&Service::new(
        GattServiceId {
            id: GattId {
                uuid: BtUuid::uuid128(1),
                inst_id: 0,
            },
            is_primary: true,
        },
        10,
    ))?;

    let char1 = service.register_characteristic(&Characteristic::new(
        Qwe(123),
        CharacteristicConfig {
            uuid: BtUuid::uuid128(2),
            value_max_len: 100,
            readable: true,
            writable: true,
            broadcasted: true,
            enable_notify: true,
        },
        None,
    ))?;

    let char2 = service.register_characteristic(&Characteristic::new(
        CoolNestedChar {
            bar: "bar".to_string(),
            foo_bar: FooBar {
                bar: "bar".to_string(),
                foo_bar: "foo_bar".to_string(),
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
    ble.gap.start_advertising()?;

    let mut i = 0;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));

        char1.update_value(Qwe(i))?;
        i += 1;
    }

    Ok(())
}
