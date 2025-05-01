use std::{
    ffi::{CStr, c_char, c_int},
    sync::{Arc, Mutex},
};

use anyhow::bail;
use crossbeam::{channel::Sender, queue::ArrayQueue};
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
        sys::{esp_log_set_vprintf, va_list, vprintf_like_t},
    },
};
use lazy_static::lazy_static;

pub struct BleLoggerService {
    pub service: Service,
}

unsafe extern "C" {
    fn vsprintf(str: *mut c_char, format: *const c_char, args: va_list) -> c_int;
}

lazy_static! {
    static ref LOGGER_QUEUE: Arc<Mutex<Option<LoggerQueue>>> = Arc::new(Mutex::new(None));
}

struct LoggerQueue {
    buffer: Arc<ArrayQueue<u8>>,
    notify_sender: Sender<()>,
    original_logger: vprintf_like_t,
}

unsafe extern "C" fn custom_logger_middleware(format: *const c_char, args: va_list) -> c_int {
    const BUF_SIZE: usize = 1024;
    let mut buffer: [u8; 1024] = [0u8; BUF_SIZE];

    let message_length = unsafe { vsprintf(buffer.as_mut_ptr() as *mut c_char, format, args) };
    if message_length < 0 {
        // log::error!("Failed to format log message");
        return message_length;
    }

    let Ok(logger_queue) = LOGGER_QUEUE.lock() else {
        return -1;
    };

    let Some(logger_queue) = logger_queue.as_ref() else {
        return -1;
    };

    for byte in &buffer[..message_length as usize] {
        logger_queue.buffer.force_push(*byte);
    }
    logger_queue.notify_sender.send(()).ok();

    match logger_queue.original_logger {
        Some(original_logger) => unsafe { original_logger(format, args) },
        None => 0,
    }
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

        let (tx_logs_messages, rx_logs_messages) = crossbeam::channel::unbounded();
        let shared_buffer = Arc::new(ArrayQueue::new(1024));
        let original_logger = unsafe { esp_log_set_vprintf(Some(custom_logger_middleware)) };

        let queue = LoggerQueue {
            buffer: shared_buffer.clone(),
            notify_sender: tx_logs_messages,
            original_logger,
        };
        LOGGER_QUEUE
            .lock()
            .map_err(|err| anyhow::anyhow!("Was not able to lock static LOGGER_QUEUE: {:?}", err))?
            .replace(queue);

        std::thread::spawn(move || {
            for _ in rx_logs_messages.iter() {
                let mut message = vec![];
                while let Some(byte) = shared_buffer.pop() {
                    message.push(byte);
                }

                if message.is_empty() {
                    continue;
                }

                let errors: Vec<anyhow::Error> = message
                    .chunks(20)
                    .filter_map(|chunk| {
                        rx_characteristic
                            .update_value(BytesAttr(chunk.to_vec()))
                            .err()
                    })
                    .collect();

                if !errors.is_empty() {
                    log::error!("Failed to send log message: {:?}", errors);
                }
            }
        });

        Ok(())
    }
}
