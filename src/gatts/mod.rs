pub mod app;
pub mod characteristic;
pub mod descriptor;
pub mod event;
pub mod service;

use std::{
    collections::HashMap,
    mem::{discriminant, Discriminant},
    sync::{mpsc, Arc, RwLock},
};

use app::{App, AppInner};
use esp_idf_svc::bt::ble::gatt::server::{AppId, EspGatts};
use event::{GattsEvent, GattsEventMessage};

use crate::ble::ExtBtDriver;
use esp_idf_svc as svc;

pub struct Gatts<'d>(pub Arc<GattsInner<'d>>);

pub struct GattsInner<'d> {
    gatts: EspGatts<'d, svc::bt::Ble, ExtBtDriver<'d>>,
    apps: Arc<RwLock<HashMap<AppId, Arc<AppInner<'d>>>>>,

    gatts_events:
        Arc<RwLock<HashMap<Discriminant<GattsEvent>, mpsc::SyncSender<GattsEventMessage>>>>,
}

impl<'d> Gatts<'d> {
    pub fn new(bt: ExtBtDriver<'d>) -> anyhow::Result<Self> {
        let gatts = EspGatts::new(bt)?;
        let gatts_inner = GattsInner {
            gatts,
            apps: Arc::new(RwLock::new(HashMap::new())),
            gatts_events: Arc::new(RwLock::new(HashMap::new())),
        };
        gatts_inner.init_callback()?;

        let gatts = Self(Arc::new(gatts_inner));

        Ok(gatts)
    }

    pub fn register_app(&self, app_id: AppId) -> anyhow::Result<App<'d>> {
        App::new(self.0.clone(), app_id)
    }
}

impl<'d> GattsInner<'d> {
    fn init_callback(&self) -> anyhow::Result<()> {
        let callback_inner_ref = Arc::downgrade(&self.gatts_events);
        self.gatts.subscribe(move |(interface, e)| {
            log::info!("Received event {:?}", e);

            let Some(callback_map) = callback_inner_ref.upgrade() else {
                log::error!("Failed to upgrade Gatts reference");
                return;
            };

            let Ok(callback_map) = callback_map.read() else {
                log::error!("Failed to acquire read lock on Gatts events map");
                return;
            };

            let event = GattsEvent::from(e);
            let Some(sender) = callback_map.get(&discriminant(&event)) else {
                log::error!("No callback found for event {:?}", event);
                return;
            };

            sender
                .send(GattsEventMessage(interface, event))
                .unwrap_or_else(|err| {
                    log::error!("Failed to send event: {:?}", err);
                });
        })?;

        Ok(())
    }
}
