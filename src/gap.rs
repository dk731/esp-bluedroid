use std::{
    collections::HashMap,
    mem::{discriminant, Discriminant},
    sync::{mpsc, Arc, Mutex, RwLock},
    time::Duration,
};

use dashmap::DashMap;
use esp_idf_svc::{
    bt::{
        ble::gap::{BleGapEvent, EspBleGap},
        BdAddr, BtStatus,
    },
    hal::task::block_on,
};

use crate::ble::ExtBtDriver;
use esp_idf_svc as svc;

#[derive(Debug)]
enum GapEvent {
    AdvertisingConfigured(BtStatus),
    ScanResponseConfigured(BtStatus),
    ScanParameterConfigured(BtStatus),
    RawAdvertisingConfigured(BtStatus),
    RawScanResponseConfigured(BtStatus),
    AdvertisingStarted(BtStatus),
    ScanStarted(BtStatus),
    AuthenticationComplete {
        bd_addr: BdAddr,
        status: BtStatus,
    },
    Key,
    SecurityRequest,
    PasskeyNotification {
        addr: BdAddr,
        passkey: u32,
    },
    PasskeyRequest,
    LocalIR,
    LocalER,
    NumericComparisonRequest,
    AdvertisingStopped(BtStatus),
    ScanStopped(BtStatus),
    StaticRandomAddressConfigured(BtStatus),
    ConnectionParamsConfigured {
        addr: BdAddr,
        status: BtStatus,
        min_int_ms: u32,
        max_int_ms: u32,
        latency_ms: u32,
        conn_int: u16,
        timeout_ms: u32,
    },
    PacketLengthConfigured {
        status: BtStatus,
        rx_len: u16,
        tx_len: u16,
    },
    LocalPrivacyConfigured(BtStatus),
    DeviceBondRemoved {
        bd_addr: BdAddr,
        status: BtStatus,
    },
    DeviceBondCleared(BtStatus),
    ReadRssiConfigured {
        bd_addr: BdAddr,
        rssdi: i8,
        status: BtStatus,
    },
    WhitelistUpdated {
        status: BtStatus,
        wl_operation: u32,
    },
    ChannelsConfigured(BtStatus),
    PreferredDefaultPhyConfigured(BtStatus),
    PreferredPhyConfigured(BtStatus),
    ExtendedAdvertisingRandomAddressConfigured(BtStatus),
    ExtendedAdvertisingParametersConfigured(BtStatus),
    ExtendedAdvertisingConfigured(BtStatus),
    ExtendedAdvertisingScanResponseConfigured(BtStatus),
    ExtendedAdvertisingStarted(BtStatus),
    ExtendedAdvertisingStopped(BtStatus),
    ExtendedAdvertisingRemoved(BtStatus),
    ExtendedAdvertisingCleared(BtStatus),
    PeriodicAdvertisingParametersConfigured(BtStatus),
    PeriodicAdvertisingDataSetComplete(BtStatus),
    PeriodicAdvertisingStarted(BtStatus),
    PeriodicAdvertisingStopped(BtStatus),
    PeriodicAdvertisingSyncCreated(BtStatus),
    PeriodicAdvertisingSyncCanceled(BtStatus),
    PeriodicAdvertisingSyncTerminated(BtStatus),
    PeriodicAdvertisingDeviceListAdded(BtStatus),
    PeriodicAdvertisingDeviceListRemoved(BtStatus),
    PeriodicAdvertisingDeviceListCleared(BtStatus),
    ExtendedAdvertisingScanParametersConfigured(BtStatus),
    ExtendedAdvertisingScanStarted(BtStatus),
    ExtendedAdvertisingScanStopped(BtStatus),
    ExtendedAdvertisingExtendedConnectionParamsConfigured(BtStatus),

    Other,
}

impl<'d> From<BleGapEvent<'d>> for GapEvent {
    fn from(event: BleGapEvent<'d>) -> Self {
        match event {
            BleGapEvent::AdvertisingConfigured(bt_status) => {
                GapEvent::AdvertisingConfigured(bt_status)
            }
            BleGapEvent::ScanResponseConfigured(bt_status) => {
                GapEvent::ScanResponseConfigured(bt_status)
            }
            BleGapEvent::ScanParameterConfigured(bt_status) => {
                GapEvent::ScanParameterConfigured(bt_status)
            }
            BleGapEvent::RawAdvertisingConfigured(bt_status) => {
                GapEvent::RawAdvertisingConfigured(bt_status)
            }
            BleGapEvent::RawScanResponseConfigured(bt_status) => {
                GapEvent::RawScanResponseConfigured(bt_status)
            }
            BleGapEvent::AdvertisingStarted(bt_status) => GapEvent::AdvertisingStarted(bt_status),
            BleGapEvent::ScanStarted(bt_status) => GapEvent::ScanStarted(bt_status),
            BleGapEvent::AuthenticationComplete { bd_addr, status } => {
                GapEvent::AuthenticationComplete { bd_addr, status }
            }
            BleGapEvent::Key => GapEvent::Key,
            BleGapEvent::SecurityRequest => GapEvent::SecurityRequest,
            BleGapEvent::PasskeyNotification { addr, passkey } => {
                GapEvent::PasskeyNotification { addr, passkey }
            }
            BleGapEvent::PasskeyRequest => GapEvent::PasskeyRequest,
            BleGapEvent::LocalIR => GapEvent::LocalIR,
            BleGapEvent::LocalER => GapEvent::LocalER,
            BleGapEvent::NumericComparisonRequest => GapEvent::NumericComparisonRequest,
            BleGapEvent::AdvertisingStopped(bt_status) => GapEvent::AdvertisingStopped(bt_status),
            BleGapEvent::ScanStopped(bt_status) => GapEvent::ScanStopped(bt_status),
            BleGapEvent::StaticRandomAddressConfigured(bt_status) => {
                GapEvent::StaticRandomAddressConfigured(bt_status)
            }
            BleGapEvent::ConnectionParamsConfigured {
                addr,
                status,
                min_int_ms,
                max_int_ms,
                latency_ms,
                conn_int,
                timeout_ms,
            } => GapEvent::ConnectionParamsConfigured {
                addr,
                status,
                min_int_ms,
                max_int_ms,
                latency_ms,
                conn_int,
                timeout_ms,
            },
            BleGapEvent::PacketLengthConfigured {
                status,
                rx_len,
                tx_len,
            } => GapEvent::PacketLengthConfigured {
                status,
                rx_len,
                tx_len,
            },
            BleGapEvent::LocalPrivacyConfigured(bt_status) => {
                GapEvent::LocalPrivacyConfigured(bt_status)
            }
            BleGapEvent::DeviceBondRemoved { bd_addr, status } => {
                GapEvent::DeviceBondRemoved { bd_addr, status }
            }
            BleGapEvent::DeviceBondCleared(bt_status) => GapEvent::DeviceBondCleared(bt_status),
            BleGapEvent::ReadRssiConfigured {
                bd_addr,
                rssdi,
                status,
            } => GapEvent::ReadRssiConfigured {
                bd_addr,
                rssdi,
                status,
            },
            BleGapEvent::WhitelistUpdated {
                status,
                wl_operation,
            } => GapEvent::WhitelistUpdated {
                status,
                wl_operation,
            },
            BleGapEvent::ChannelsConfigured(bt_status) => GapEvent::ChannelsConfigured(bt_status),
            BleGapEvent::PreferredDefaultPhyConfigured(bt_status) => {
                GapEvent::PreferredDefaultPhyConfigured(bt_status)
            }
            BleGapEvent::PreferredPhyConfigured(bt_status) => {
                GapEvent::PreferredPhyConfigured(bt_status)
            }
            BleGapEvent::ExtendedAdvertisingRandomAddressConfigured(bt_status) => {
                GapEvent::ExtendedAdvertisingRandomAddressConfigured(bt_status)
            }
            BleGapEvent::ExtendedAdvertisingParametersConfigured(bt_status) => {
                GapEvent::ExtendedAdvertisingParametersConfigured(bt_status)
            }
            BleGapEvent::ExtendedAdvertisingConfigured(bt_status) => {
                GapEvent::ExtendedAdvertisingConfigured(bt_status)
            }
            BleGapEvent::ExtendedAdvertisingScanResponseConfigured(bt_status) => {
                GapEvent::ExtendedAdvertisingScanResponseConfigured(bt_status)
            }
            BleGapEvent::ExtendedAdvertisingStarted(bt_status) => {
                GapEvent::ExtendedAdvertisingStarted(bt_status)
            }
            BleGapEvent::ExtendedAdvertisingStopped(bt_status) => {
                GapEvent::ExtendedAdvertisingStopped(bt_status)
            }
            BleGapEvent::ExtendedAdvertisingRemoved(bt_status) => {
                GapEvent::ExtendedAdvertisingRemoved(bt_status)
            }
            BleGapEvent::ExtendedAdvertisingCleared(bt_status) => {
                GapEvent::ExtendedAdvertisingCleared(bt_status)
            }
            BleGapEvent::PeriodicAdvertisingParametersConfigured(bt_status) => {
                GapEvent::PeriodicAdvertisingParametersConfigured(bt_status)
            }
            BleGapEvent::PeriodicAdvertisingDataSetComplete(bt_status) => {
                GapEvent::PeriodicAdvertisingDataSetComplete(bt_status)
            }
            BleGapEvent::PeriodicAdvertisingStarted(bt_status) => {
                GapEvent::PeriodicAdvertisingStarted(bt_status)
            }
            BleGapEvent::PeriodicAdvertisingStopped(bt_status) => {
                GapEvent::PeriodicAdvertisingStopped(bt_status)
            }
            BleGapEvent::PeriodicAdvertisingSyncCreated(bt_status) => {
                GapEvent::PeriodicAdvertisingSyncCreated(bt_status)
            }
            BleGapEvent::PeriodicAdvertisingSyncCanceled(bt_status) => {
                GapEvent::PeriodicAdvertisingSyncCanceled(bt_status)
            }
            BleGapEvent::PeriodicAdvertisingSyncTerminated(bt_status) => {
                GapEvent::PeriodicAdvertisingSyncTerminated(bt_status)
            }
            BleGapEvent::PeriodicAdvertisingDeviceListAdded(bt_status) => {
                GapEvent::PeriodicAdvertisingDeviceListAdded(bt_status)
            }
            BleGapEvent::PeriodicAdvertisingDeviceListRemoved(bt_status) => {
                GapEvent::PeriodicAdvertisingDeviceListRemoved(bt_status)
            }
            BleGapEvent::PeriodicAdvertisingDeviceListCleared(bt_status) => {
                GapEvent::PeriodicAdvertisingDeviceListCleared(bt_status)
            }
            BleGapEvent::ExtendedAdvertisingScanParametersConfigured(bt_status) => {
                GapEvent::ExtendedAdvertisingScanParametersConfigured(bt_status)
            }
            BleGapEvent::ExtendedAdvertisingScanStarted(bt_status) => {
                GapEvent::ExtendedAdvertisingScanStarted(bt_status)
            }
            BleGapEvent::ExtendedAdvertisingScanStopped(bt_status) => {
                GapEvent::ExtendedAdvertisingScanStopped(bt_status)
            }
            BleGapEvent::ExtendedAdvertisingExtendedConnectionParamsConfigured(bt_status) => {
                GapEvent::ExtendedAdvertisingExtendedConnectionParamsConfigured(bt_status)
            }

            _ => GapEvent::Other,
        }
    }
}

pub struct Gap<'d> {
    gap: EspBleGap<'d, svc::bt::Ble, ExtBtDriver<'d>>,
    // gap_events_rx: Arc<mpsc::Receiver<GapEvent>>,
    // gap_events_tx: mpsc::Sender<GapEvent>,
    // gap_events: Arc<Mutex<HashMap<Discriminant<GapEvent>, mpsc::Sender<GapEvent>>>>,
}

impl<'d> Gap<'d> {
    pub fn new(bt: ExtBtDriver<'d>) -> anyhow::Result<Self> {
        let gap = EspBleGap::new(bt)?;

        let gap = Self {
            gap,
            // gap_events: Arc::new(Mutex::new(HashMap::new())),
        };

        gap.init()?;
        log::debug!("GAP initialized");

        Ok(gap)
    }

    pub fn init(&self) -> anyhow::Result<()> {
        log::info!("GAP init");

        let (gap_events_tx, gap_events_rx) = mpsc::channel::<GapEvent>();
        let text_mutex = Arc::new(Mutex::new(123));

        let test_lock = text_mutex.clone();
        std::thread::spawn(move || {
            log::info!("Locking mutex in thread");
            let guard = test_lock.lock();
            std::thread::sleep(Duration::from_secs(5));
            log::info!("Dropping mutex lock from thread {:?}", guard);
        });

        let qweqwe = Arc::new(gap_events_tx);
        unsafe {
            self.gap.subscribe_nonstatic(|e| {
                log::info!("Received GAP event: {:?}", e);

                log::info!("Attempting to lock mutex in callback");

                if let Ok(guard) = text_mutex.lock() {
                    log::info!("Mutex guard value: {:?}", *guard);
                } else {
                    log::error!("Failed to acquire lock on mutex");
                }

                // let event = GapEvent::from(e);
            })?;
            log::debug!("Subscribed to GAP events");
        }

        std::thread::spawn(move || loop {
            match gap_events_rx.recv() {
                Ok(event) => {
                    log::info!("Received GAP event: {:?}", event);
                    // Handle the event here
                    // For example, you can send it to a channel or process it directly
                }
                Err(_) => {
                    log::error!("Timeout waiting for GAP event");
                    break;
                }
            }
        });

        Ok(())
    }

    fn events_callback(&self) {
        // let rwlock_addr = &self.gap_events as *const _ as usize;
        // log::info!(
        //     "gap_events RwLock address in events_callback: 0x{:x}",
        //     rwlock_addr
        // );

        // loop {
        //     match self.gap_events_rx.recv_timeout(Duration::from_secs(5)) {
        //         Ok(event) => {
        //             log::info!("Received GAP event: {:?}", event);
        //             // Handle the event here
        //             // For example, you can send it to a channel or process it directly
        //         }
        //         Err(_) => {
        //             log::error!("Timeout waiting for GAP event");
        //             break;
        //         }
        //     }
        // }

        // match event {
        //     GapEvent::AdvertisingStarted(_) => {
        //         log::info!("Received GAP event, sending to channel: {:?}", event);

        //         match self.gap_events.lock() {
        //             Ok(gap_events) => {
        //                 if let Some(tx) = gap_events.get(&discriminant(&event)) {
        //                     if let Err(err) = tx.send(event) {
        //                         log::error!("Failed to send GAP event to channel {:?}", err);
        //                     }
        //                 } else {
        //                     log::error!(
        //                         "No channel found for GAP event: {:?}",
        //                         discriminant(&event)
        //                     );
        //                     log::error!("All gav events: {:?}", gap_events);
        //                 }
        //             }
        //             Err(_) => {
        //                 log::error!("Failed to acquire read lock for GAP events");
        //             }
        //         }
        //     }
        //     _ => {
        //         log::debug!("GAP event: {:?}", event);
        //     }
        // }
    }

    pub fn start_advertising(&self) -> anyhow::Result<()> {
        // let (tx, rx) = mpsc::channel::<GapEvent>();
        // let Ok(mut gap_events) = self.gap_events.lock() else {
        //     log::error!("Failed to acquire write lock for GAP events");
        //     return Err(anyhow::anyhow!(
        //         "Failed to acquire write lock for GAP events"
        //     ));
        // };
        // let qwe = discriminant(&GapEvent::AdvertisingStarted(BtStatus::Success));
        // gap_events.insert(qwe, tx);

        // log::info!("AdvertisingStarted event discriminant: {:?}", qwe);
        // log::info!("AdvertisingStarted event discriminant: {:?}", gap_events);

        // let rwlock_addr = &self.gap_events as *const _ as usize;
        // log::info!(
        //     "gap_events RwLock address in start_advertising: 0x{:x}",
        //     rwlock_addr
        // );

        // drop(gap_events);

        self.gap.start_advertising()?;

        // let recv_result = match rx.recv_timeout(Duration::from_secs(5)) {
        //     Ok(status) => match status {
        //         GapEvent::AdvertisingStarted(bt_status) => {
        //             log::info!("Received GAP event from channel: {:?}", status);

        //             match bt_status {
        //                 BtStatus::Success => Ok(()),
        //                 _ => {
        //                     log::error!("Failed to start advertising: {:?}", bt_status);
        //                     Err(anyhow::anyhow!(
        //                         "Failed to start advertising: {:?}",
        //                         bt_status
        //                     ))
        //                 }
        //             }
        //         }
        //         _ => {
        //             log::error!("Unexpected event: {:?}", status);
        //             Err(anyhow::anyhow!("Unexpected event: {:?}", status))
        //         }
        //     },
        //     Err(_) => {
        //         log::error!("Timeout waiting for advertising started event");
        //         Err(anyhow::anyhow!(
        //             "Timeout waiting for advertising started event"
        //         ))
        //     }
        // };

        // // gap_events.remove(&discriminant(&GapEvent::AdvertisingStarted(BtStatus::Fail)));

        // log::debug!("Removed channel for AdvertisingStarted event");
        // recv_result
        Ok(())
    }

    // self.gap
    //             .subscribe_nonstatic(|e| self.gap_events_callback(e))?;
    //         log::debug!("Subscribed to GAP events");
}

impl Drop for Gap<'_> {
    fn drop(&mut self) {
        if let Err(err) = self.gap.unsubscribe() {
            log::error!("Failed to unsubscribe from GAP events: {:?}", err);
        }
    }
}
