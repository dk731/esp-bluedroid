mod events;

use std::{
    collections::HashMap,
    mem::{discriminant, Discriminant},
    sync::{Arc, RwLock},
    time::Duration,
};

use crossbeam_channel::bounded;
use esp_idf_svc::bt::{ble::gap::EspBleGap, BtStatus};
use events::GapEvent;

use crate::ble::ExtBtDriver;
use esp_idf_svc as svc;

pub struct Gap<'d> {
    gap: EspBleGap<'d, svc::bt::Ble, ExtBtDriver<'d>>,

    gap_events: Arc<RwLock<HashMap<Discriminant<GapEvent>, crossbeam_channel::Sender<GapEvent>>>>,
}

impl<'d> Gap<'d> {
    pub fn new(bt: ExtBtDriver<'d>) -> anyhow::Result<Self> {
        let gap = EspBleGap::new(bt)?;

        let gap = Self {
            gap,
            gap_events: Arc::new(RwLock::new(HashMap::new())),
        };

        gap.init_callback()?;

        Ok(gap)
    }

    pub fn init_callback(&self) -> anyhow::Result<()> {
        let callback_channels_map = self.gap_events.clone();
        self.gap.subscribe(move |e| {
            log::info!("Received event {:?}", e);

            let Ok(map_lock) = callback_channels_map.read() else {
                log::error!("Failed to acquire write lock for events map");
                return;
            };

            let event = GapEvent::from(e);
            let Some(callback_channel) = map_lock.get(&discriminant(&event)) else {
                log::debug!("No callback channel found for event: {:?}", event);
                return;
            };

            log::info!("Sending event {:?} to callback channel", event);
            callback_channel.send(event).unwrap_or_else(|err| {
                log::error!("Failed to send event to callback channel: {:?}", err);
            });
            log::info!("Sent event to callback channel");
        })?;

        Ok(())
    }

    pub fn start_advertising(&self) -> anyhow::Result<()> {
        let (tx, rx) = bounded(0);
        self.gap_events
            .write()
            .map_err(|err| anyhow::anyhow!("Failed to write gap_events: {:?}", err))?
            .insert(
                discriminant(&GapEvent::AdvertisingStarted(BtStatus::Done)).into(),
                tx.clone(),
            );

        self.gap.start_advertising()?;

        log::info!("Listening for advertising started event");
        let recv_result = match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(status) => match status {
                GapEvent::AdvertisingStarted(bt_status) => match bt_status {
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

        self.gap_events
            .write()
            .map_err(|err| anyhow::anyhow!("Failed to write gap_events: {:?}", err))?
            .remove(&discriminant(&GapEvent::AdvertisingStarted(BtStatus::Done)));

        recv_result
    }
}
