pub mod app;
pub mod characteristic;
pub mod descriptor;
pub mod event;
pub mod service;

use std::{
    collections::HashMap,
    mem::{discriminant, Discriminant},
    sync::{mpsc, Arc, RwLock, Weak},
};

use app::{App, AppInner};
use crossbeam_channel::bounded;
use esp_idf_svc::bt::{
    ble::gatt::server::{AppId, EspGatts},
    BdAddr,
};
use event::{GattsEvent, GattsEventMessage};

use crate::ble::ExtBtDriver;
use esp_idf_svc as svc;

pub struct Gatts(pub Arc<GattsInner>);

pub struct GattsInner {
    gatts: EspGatts<'static, svc::bt::Ble, ExtBtDriver>,
    apps: Arc<RwLock<HashMap<AppId, Arc<AppInner>>>>,

    gatts_events: Arc<
        RwLock<HashMap<Discriminant<GattsEvent>, crossbeam_channel::Sender<GattsEventMessage>>>,
    >,
}

impl Gatts {
    pub fn new(bt: ExtBtDriver) -> anyhow::Result<Self> {
        let gatts = EspGatts::new(bt)?;
        let gatts_inner = GattsInner {
            gatts,
            apps: Arc::new(RwLock::new(HashMap::new())),
            gatts_events: Arc::new(RwLock::new(HashMap::new())),
        };

        let gatts = Self(Arc::new(gatts_inner));

        gatts.init_callback()?;
        gatts.configure_read_events()?;

        Ok(gatts)
    }

    fn configure_read_events(&self) -> anyhow::Result<()> {
        let (tx, rx) = bounded(1);

        self.0
            .gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock on Gatts events map"))?
            .insert(
                discriminant(&GattsEvent::Read {
                    conn_id: 0,
                    trans_id: 0,
                    addr: BdAddr::from_bytes([0; 6]),
                    handle: 0,
                    offset: 0,
                    is_long: false,
                    need_rsp: true,
                }),
                tx,
            );

        let gatts = Arc::downgrade(&self.0);
        std::thread::spawn(move || {
            for event in rx.iter() {
                let Some(gatts) = gatts.upgrade() else {
                    log::error!("Failed to upgrade Gatts instance");
                    return;
                };

                // if let Err(err) = gatts.handle_read_event(event) {
                //     log::error!("Failed to handle read event: {:?}", err);
                //     return;
                // }
            }
        });

        Ok(())
    }

    fn init_callback(&self) -> anyhow::Result<()> {
        let callback_inner_ref = Arc::downgrade(&self.0.gatts_events);
        self.0.gatts.subscribe(move |(interface, e)| {
            log::info!("Received event {:?}", e);

            let Some(callback_map) = callback_inner_ref.upgrade() else {
                log::error!("Failed to upgrade Gatts events map");
                return;
            };

            let Ok(callback_map) = callback_map.read() else {
                log::error!("Failed to acquire read lock on Gatts events map");
                return;
            };

            let event = GattsEvent::from(e);
            let Some(sender) = callback_map.get(&discriminant(&event)) else {
                log::warn!("No callback found for event {:?}", event);
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

    pub fn register_app(&self, app_id: AppId) -> anyhow::Result<App> {
        App::new(self.0.clone(), app_id)
    }
}

impl GattsInner {
    fn handle_read_event(&self, event: GattsEventMessage) -> anyhow::Result<()> {
        //

        Ok(())
    }
}
