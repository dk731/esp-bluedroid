use std::{
    ffi::CStr,
    ops::Add,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicI32, AtomicUsize},
    },
};

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
        log::EspLogger,
        sys::{esp_log_system_timestamp, esp_log_timestamp},
    },
};
use lazy_static::lazy_static;
use ringbuf::{
    HeapRb, SharedRb,
    storage::Heap,
    traits::{Consumer, Observer, RingBuffer},
};

static ESP_LOGGER: EspLogger = EspLogger::new();
static BLE_LOGGER: BleLogger = BleLogger();

pub struct BleLoggerService {
    pub service: Service,
}

lazy_static! {
    static ref LOGGER_QUEUE: Arc<LoggerQueue> = Arc::new({
        let (notify_sender, notify_receiver) = crossbeam::channel::unbounded();
        LoggerQueue {
            buffer: Mutex::new(HeapRb::new(1024)),
            // buffer: ArrayQueue::new(1024),
            notify_sender,
            notify_receiver,
        }
    });
    static ref QWE: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
    static ref EWQ: Arc<Mutex<String>> = Arc::new(Mutex::new("empty ".to_string()));

}

static EEE: AtomicUsize = AtomicUsize::new(666);

struct LoggerQueue {
    buffer: Mutex<SharedRb<Heap<u8>>>,
    // buffer: ArrayQueue<u8>,
    notify_sender: Sender<()>,
    notify_receiver: crossbeam::channel::Receiver<()>,
}

impl BleLoggerService {
    pub fn new() -> Self {
        let service = Service::new(
            GattServiceId {
                id: GattId {
                    uuid: BtUuid::uuid128(0x6e400001_b5a3_f393_e0a9_e50e24dcca9e), // Nordic UART Service
                    inst_id: 0,
                },
                is_primary: true,
            },
            10,
        );

        Self { service }
    }

    pub fn logger(&self) -> &EspLogger {
        &ESP_LOGGER
    }

    pub fn initialize_default(&self) -> anyhow::Result<()> {
        log::set_logger(&BLE_LOGGER)?;
        ESP_LOGGER.initialize();

        Ok(())
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
                description: Some("esp-bluedriod-logger".to_string()),
            },
            None,
        );

        self.service.register_characteristic(&tx_characteristic)?;
        self.service.register_characteristic(&rx_characteristic)?;

        std::thread::spawn(move || {
            let mut i = 0;
            for _ in LOGGER_QUEUE.notify_receiver.iter() {
                let Ok(mut buffer) = LOGGER_QUEUE.buffer.lock() else {
                    log::error!("Failed to lock buffer");
                    continue;
                };
                let mut message = vec![0x00; buffer.occupied_len()];
                let read_size = buffer.pop_slice(&mut message);
                drop(buffer);
                // let message = vec![];

                if message.is_empty() {
                    continue;
                }

                let errors: Vec<anyhow::Error> = message
                    .chunks(20)
                    .filter_map(|chunk| {
                        i += 1;
                        EEE.store(i, std::sync::atomic::Ordering::Relaxed);

                        rx_characteristic
                            .update_value(BytesAttr(chunk.to_vec()))
                            .err()
                    })
                    .collect();

                // if !errors.is_empty() {
                //     log::error!("Failed to send log message: {:?}", errors);
                // }
            }

            log::info!("Sender thread: finished");
        });

        std::thread::spawn(|| {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));

                // let current_len = LOGGER_QUEUE.buffer.lock().unwrap().occupied_len();
                log::info!(
                    "Sender thread, last send: {:?}, buffer len: {:?}",
                    // current_len,
                    EEE.load(std::sync::atomic::Ordering::Relaxed),
                    0
                );
            }
        });

        Ok(())
    }
}

struct BleLogger();

impl log::Log for BleLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        ESP_LOGGER.enabled(metadata)
    }

    fn log(&self, record: &log::Record) {
        ESP_LOGGER.log(record);

        let metadata = record.metadata();
        if self.enabled(metadata) {
            let marker = "123";
            let target = record.metadata().target();
            let args = record.args();

            let timestamp = if cfg!(esp_idf_log_timestamp_source_rtos) {
                &unsafe { esp_log_timestamp() }.to_string()
            } else if cfg!(esp_idf_log_timestamp_source_system) {
                unsafe { CStr::from_ptr(esp_log_system_timestamp()).to_str().unwrap() }
            } else {
                ""
            };

            let log_message = format!("{} ({}) {}: {}\n", marker, timestamp, target, args);

            LOGGER_QUEUE
                .buffer
                .lock()
                .unwrap()
                .push_slice_overwrite(log_message.as_bytes());
            LOGGER_QUEUE.notify_sender.send(()).ok();
        }
    }

    fn flush(&self) {
        ESP_LOGGER.flush();
    }
}
