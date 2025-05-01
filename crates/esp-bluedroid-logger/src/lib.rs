use std::ffi::{CStr, c_char, c_int};

use esp_bluedroid::{
    gatts::{
        attribute::defaults::BytesAttr,
        characteristic::{Characteristic, CharacteristicConfig},
        service::Service,
    },
    svc::{
        bt::{
            BtUuid,
            ble::gatt::{GattId, GattServiceId},
        },
        sys::{esp_log_set_vprintf, va_list},
    },
};

pub struct BleLoggerService {
    pub service: Service,
}

unsafe extern "C" {
    fn vsprintf(str: *mut c_char, format: *const c_char, args: va_list) -> c_int;
}

unsafe extern "C" fn custom_logger_middleware(format: *const c_char, args: va_list) -> c_int {
    const BUF_SIZE: usize = 1024;
    let mut buffer = [0u8; BUF_SIZE];

    let result = unsafe { vsprintf(buffer.as_mut_ptr() as *mut c_char, format, args) };
    if result < 0 {
        log::error!("Failed to format log message");
        return result;
    }

    let c_str = unsafe { CStr::from_ptr(buffer.as_ptr() as *const c_char) };
    let bytes = c_str.to_bytes().to_vec();

    // Your custom logging implementation here
    // For example, you might want to format the message and send it over BLE

    // This is a placeholder implementation that just passes through to the
    // standard vprintf function from libc
    // libc::vprintf(format, args)

    bytes.len() as c_int
}

impl BleLoggerService {
    pub fn new() -> Self {
        let service = Service::new(
            GattServiceId {
                id: GattId {
                    uuid: BtUuid::uuid128(0x6e400001_b5a3_f393_e0a9_e50e24dcca9e), // Nordic UART Service
                    inst_id: 0,
                },
                is_primary: false,
            },
            10,
        );

        Self { service }
    }

    pub fn register(&self) -> anyhow::Result<()> {
        let tx_characteristic = Characteristic::new(
            BytesAttr(vec![0x00; 20]),
            CharacteristicConfig {
                uuid: BtUuid::uuid128(0x6e400002_b5a3_f393_e0a9_e50e24dcca9e),
                value_max_len: 20,
                readable: true,
                writable: true,
                broadcasted: false,
                enable_notify: false,
                description: None,
            },
            None,
        );

        let rx_characteristic = Characteristic::new(
            BytesAttr(vec![0x00; 20]),
            CharacteristicConfig {
                uuid: BtUuid::uuid128(0x6e400003_b5a3_f393_e0a9_e50e24dcca9e),
                value_max_len: 20,
                readable: true,
                writable: false,
                broadcasted: false,
                enable_notify: true,
                description: Some("esp-bluedriod Logging".to_string()),
            },
            None,
        );

        self.service.register_characteristic(&tx_characteristic)?;
        self.service.register_characteristic(&rx_characteristic)?;

        let _original_logger = unsafe { esp_log_set_vprintf(Some(custom_logger_middleware)) };

        Ok(())
    }
}
