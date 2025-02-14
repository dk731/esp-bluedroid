use std::{
    collections::HashMap,
    mem::{discriminant, Discriminant},
    sync::{mpsc, Arc, Mutex, RwLock},
    time::Duration,
};

use esp_idf_svc::{
    bt::{
        ble::{
            gap::{BleGapEvent, EspBleGap},
            gatt::server::EspGatts,
        },
        BdAddr, BtStatus,
    },
    hal::task::block_on,
};

use crate::ble::ExtBtDriver;
use esp_idf_svc as svc;

#[derive(Debug)]
enum GattsEvent {}

pub struct Gatts<'d> {
    gatts: EspGatts<'d, svc::bt::Ble, ExtBtDriver<'d>>,

    gatts_events: Arc<RwLock<HashMap<Discriminant<GattsEvent>, mpsc::Sender<GattsEvent>>>>,
}

impl<'d> Gatts<'d> {
    pub fn new(bt: ExtBtDriver<'d>) -> anyhow::Result<Self> {
        let gatts = EspGatts::new(bt)?;

        let gap = Self {
            gatts,
            gatts_events: Arc::new(RwLock::new(HashMap::new())),
        };

        gap.init_callback()?;

        Ok(gap)
    }

    pub fn init_callback(&self) -> anyhow::Result<()> {
        let callback_channels_map = self.gatts_events.clone();
        self.gatts.subscribe(move |e| {
            log::info!("Received GAP event {:?}", e);

            let Ok(map_lock) = callback_channels_map.read() else {
                log::error!("Failed to acquire write lock for GAP events");
                return;
            };

            let event = GattsEvent::from(e);
            let Some(callback_channel) = map_lock.get(&discriminant(&event)) else {
                log::debug!("No callback channel found for event: {:?}", event);
                return;
            };

            callback_channel.send(event).unwrap_or_else(|err| {
                log::error!("Failed to send GAP event to channel: {:?}", err);
            });
        })?;

        Ok(())
    }

    pub fn start_advertising(&self) -> anyhow::Result<()> {
        let (tx, rx) = mpsc::channel::<GattsEvent>();
        match self.gatts_events.write() {
            Ok(mut events_map) => {
                events_map.insert(
                    discriminant(&GattsEvent::AdvertisingStarted(BtStatus::Done)),
                    tx.clone(),
                );
            }
            Err(err) => {
                return Err(anyhow::anyhow!("Failed to acquire write lock: {:?}", err));
            }
        }

        self.gatts.start_advertising()?;

        let recv_result = match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(status) => match status {
                GattsEvent::AdvertisingStarted(bt_status) => match bt_status {
                    BtStatus::Success => Ok(()),
                    _ => Err(anyhow::anyhow!(
                        "Failed to start advertising: {:?}",
                        bt_status
                    )),
                },
                _ => Err(anyhow::anyhow!("Unexpected event: {:?}", status)),
            },
            Err(_) => Err(anyhow::anyhow!(
                "Timeout waiting for advertising started event"
            )),
        };

        match self.gatts_events.write() {
            Ok(mut qwe) => {
                qwe.remove(&discriminant(&GattsEvent::AdvertisingStarted(
                    BtStatus::Done,
                )));
            }
            Err(err) => {
                return Err(anyhow::anyhow!("Failed to acquire write lock: {:?}", err));
            }
        };

        recv_result
    }
}
