pub mod characteristic;
pub mod descriptor;
pub mod service;

use std::{
    collections::HashMap,
    mem::Discriminant,
    sync::{mpsc, Arc, RwLock},
};

use esp_idf_svc::bt::ble::gatt::server::EspGatts;

use crate::ble::ExtBtDriver;
use esp_idf_svc as svc;

#[derive(Debug)]
enum GattsEvent {}

pub struct Gatts<'d> {
    gatts: EspGatts<'d, svc::bt::Ble, ExtBtDriver<'d>>,

    services: Arc<RwLock<HashMap<u16, Service>>>,
    gatts_events: Arc<RwLock<HashMap<Discriminant<GattsEvent>, mpsc::Sender<GattsEvent>>>>,
}

impl<'d> Gatts<'d> {
    pub fn new(bt: ExtBtDriver<'d>) -> anyhow::Result<Self> {
        let gatts = EspGatts::new(bt)?;

        let gatts = Self {
            gatts,
            gatts_events: Arc::new(RwLock::new(HashMap::new())),
        };

        gatts.init_callback()?;

        Ok(gatts)
    }

    pub fn init_callback(&self) -> anyhow::Result<()> {
        let callback_channels_map = self.gatts_events.clone();
        self.gatts.subscribe(move |e| {
            log::info!("Received event {:?}", e);

            let Ok(map_lock) = callback_channels_map.read() else {
                log::error!("Failed to acquire write lock for events map");
                return;
            };

            // let event = GattsEvent::from(e);
            // let Some(callback_channel) = map_lock.get(&discriminant(&event)) else {
            //     log::debug!("No callback channel found for event: {:?}", event);
            //     return;
            // };

            // callback_channel.send(event).unwrap_or_else(|err| {
            //     log::error!("Failed to send GAP event to channel: {:?}", err);
            // });
        })?;

        Ok(())
    }
}
