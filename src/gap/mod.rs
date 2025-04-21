mod event;

use std::{
    collections::HashMap,
    mem::{discriminant, Discriminant},
    sync::{Arc, RwLock, Weak},
    time::Duration,
};

use crossbeam_channel::{bounded, Sender};
use esp_idf_svc::{
    bt::{ble::gap::EspBleGap, BtStatus},
    hal::task::thread,
};
use event::GapEvent;

use crate::{
    ble::ExtBtDriver,
    gatts::{connection::ConnectionStatus, GattsInner},
};
use esp_idf_svc as svc;

#[derive(Debug, Clone)]
pub struct GapConfig {
    pub name: String,
    pub appearance: u16,
    pub device_id: u8,

    // Maximum number of connections for auto advertising
    // if Some passed, Gap will automatically start advertising if connections < max_connections
    pub max_connections: Option<usize>,
}

impl Default for GapConfig {
    fn default() -> Self {
        Self {
            name: "ESP32".to_string(),
            appearance: 0,
            device_id: 0,
            max_connections: Some(1),
        }
    }
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

        gap.init_callbacks()?;
        gap.apply_config()?;

        Ok(gap)
    }

    pub fn init_callbacks(&self) -> anyhow::Result<()> {
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

        let gap = self.0.clone();
        std::thread::spawn(move || {
            log::info!("Starting auto advertising thread");
            let connection_rx = gap.gatts.upgrade().unwrap().connections_rx.clone();

            for event in connection_rx.iter() {
                if gap.gatts.upgrade().is_none() {
                    log::error!("Gatts is no longer available, stopping auto advertising thread");
                    break;
                }

                match event {
                    _ => {
                        let Ok(need_advertise) = gap.check_start_advertising() else {
                            log::error!("Failed to check start advertising");
                            continue;
                        };

                        if need_advertise {
                            log::info!("Starting advertising due to new connection");
                            if let Err(err) = gap.start_advertising() {
                                log::error!("Failed to start advertising: {:?}", err);
                            }
                        } else {
                            log::info!("No need to start advertising, max connections reached");
                        }
                    }
                }
            }
        });

        Ok(())
    }

    pub fn start_advertising(&self) -> anyhow::Result<()> {
        self.0.start_advertising()
    }

    fn apply_config(&self) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn set_config(&mut self, config: GapConfig) -> anyhow::Result<()> {
        *self.0.config.write().map_err(|err| {
            anyhow::anyhow!("Failed to acquire write lock for gap config: {:?}", err)
        })? = config;

        self.apply_config()?;

        Ok(())
    }
}

impl GapInner {
    fn check_start_advertising(&self) -> anyhow::Result<bool> {
        let gatts = self
            .gatts
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("Failed to upgrade Gatts from Weak reference"))?;
        let apps = gatts
            .apps
            .read()
            .map_err(|err| anyhow::anyhow!("Failed to acquire read lock for apps: {:?}", err))?;
        let current_connection = apps
            .values()
            .map(|app| app.connections.read().unwrap().len())
            .sum::<usize>();

        let config = self.config.read().map_err(|err| {
            anyhow::anyhow!("Failed to acquire read lock for gap config: {:?}", err)
        })?;
        let max_connection = config
            .max_connections
            .ok_or(anyhow::anyhow!("Max connections not set in gap config"))?;

        Ok(max_connection <= current_connection)
    }

    pub fn start_advertising(&self) -> anyhow::Result<()> {
        let (tx, rx) = bounded(1);
        self.gap_events
            .write()
            .map_err(|err| anyhow::anyhow!("Failed to write gap_events: {:?}", err))?
            .insert(
                discriminant(&GapEvent::AdvertisingStarted(BtStatus::Done)).into(),
                tx.clone(),
            );

        self.gap.start_advertising()?;

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
}
