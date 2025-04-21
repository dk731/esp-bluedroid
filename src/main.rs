use esp_bluedroid::{
    ble,
    gatts::{
        app::App,
        attribute::{Attribute, SerializableAttribute},
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
    fn new_from_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(vec![self.0])
    }

    fn to_bytes(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        let Some(new_value) = bytes.get(0) else {
            return Err(anyhow::anyhow!("Failed to parse bytes to u8"));
        };

        *self = Qwe(new_value.clone());

        Ok(())
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
            enable_notify: true,
        },
        Qwe(123),
    ))?;

    let thread_char = char1.clone();
    // std::thread::spawn(move || {
    //     for CharacteristicUpdate { old, new } in thread_char.0.updates_rx.iter() {
    //         log::info!("Characteristic was update.\tOld: {:?}\tNew: {:?}", old, new);
    //     }
    // });

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
