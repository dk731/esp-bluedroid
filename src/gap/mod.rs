mod event;

use std::{
    collections::HashMap,
    mem::{discriminant, Discriminant},
    sync::{Arc, RwLock, Weak},
    time::Duration,
};

use crossbeam_channel::{bounded, Sender};
use esp_idf_svc::bt::{ble::gap::EspBleGap, BtStatus};
use event::GapEvent;

use crate::{ble::ExtBtDriver, gatts::GattsInner};
use esp_idf_svc as svc;

#[derive(Debug, Clone, Default)]
pub struct GapConfig {
    pub name: String,
    pub appearance: u16,
    pub device_id: u8,
}

#[derive(Clone)]
pub struct Gap(pub Arc<GapInner>);

pub struct GapInner {
    gatts: Weak<GattsInner>,
    gap: EspBleGap<'static, svc::bt::Ble, ExtBtDriver>,
    config: RwLock<GapConfig>,

    gap_events: Arc<RwLock<HashMap<Discriminant<GapEvent>, Sender<GapEvent>>>>,
}

impl Gap {
    pub fn new(bt: ExtBtDriver, gatts: &Arc<GattsInner>) -> anyhow::Result<Self> {
        let gap = EspBleGap::new(bt)?;

        let gap = GapInner {
            gap,
            gap_events: Arc::new(RwLock::new(HashMap::new())),
            gatts: Arc::downgrade(gatts),
            config: RwLock::new(GapConfig::default()),
        };
        let gap = Self(Arc::new(gap));

        gap.init_callback()?;
        gap.register_config()?;

        Ok(gap)
    }

    pub fn init_callback(&self) -> anyhow::Result<()> {
        let callback_channels_map = Arc::downgrade(&self.0.gap_events);
        self.0.gap.subscribe(move |e| {
            let Some(callback_channels) = callback_channels_map.upgrade() else {
                log::error!("Failed to upgrade Gap events map");
                return;
            };

            log::info!("Received event {:?}", e);

            let Ok(map_lock) = callback_channels.read() else {
                log::error!("Failed to acquire write lock for events map");
                return;
            };

            let event = GapEvent::from(e);
            let Some(callback_channel) = map_lock.get(&discriminant(&event)) else {
                log::warn!("No callback channel found for event: {:?}", event);
                return;
            };

            callback_channel.send(event).unwrap_or_else(|err| {
                log::error!("Failed to send event to callback channel: {:?}", err);
            });
        })?;

        Ok(())
    }

    pub fn start_advertising(&self) -> anyhow::Result<()> {
        let (tx, rx) = bounded(1);
        self.0
            .gap_events
            .write()
            .map_err(|err| anyhow::anyhow!("Failed to write gap_events: {:?}", err))?
            .insert(
                discriminant(&GapEvent::AdvertisingStarted(BtStatus::Done)).into(),
                tx.clone(),
            );

        self.0.gap.start_advertising()?;

        match rx.recv_timeout(Duration::from_secs(5)) {
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
        }
    }

    fn register_config(&self) -> anyhow::Result<()> {
        //
        Ok(())
    }

    pub fn set_config(&mut self, config: GapConfig) -> anyhow::Result<()> {
        *self.0.config.write().map_err(|err| {
            anyhow::anyhow!("Failed to acquire write lock for gap config: {:?}", err)
        })? = config;

        Ok(())
    }
}
