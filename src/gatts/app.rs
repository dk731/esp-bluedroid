use std::{
    collections::HashMap,
    mem::discriminant,
    sync::{Arc, RwLock, Weak},
};

use crossbeam_channel::bounded;
use esp_idf_svc::bt::ble::gatt::{
    server::{AppId, ConnectionId},
    GattInterface, GattServiceId, GattStatus,
};

use super::{
    connection::ConnectionInner,
    service::{Service, ServiceId, ServiceInner},
    GattsEvent, GattsEventMessage, GattsInner,
};

#[derive(Clone)]
pub struct App(pub Arc<AppInner>);

pub struct AppInner {
    pub gatts: RwLock<Option<Weak<GattsInner>>>,
    pub interface: RwLock<Option<GattInterface>>,
    pub services: Arc<RwLock<HashMap<ServiceId, Arc<ServiceInner>>>>,
    pub connections: Arc<RwLock<HashMap<ConnectionId, ConnectionInner>>>,

    pub id: AppId,
}

impl App {
    pub fn new(app_id: AppId) -> anyhow::Result<Self> {
        let app = AppInner {
            gatts: RwLock::new(None),
            id: app_id,
            services: Arc::new(RwLock::new(HashMap::new())),
            interface: RwLock::new(None),
            connections: Arc::new(RwLock::new(HashMap::new())),
        };

        let app = Self(Arc::new(app));

        Ok(app)
    }

    pub fn register_bluedroid(&self, gatts: &Arc<GattsInner>) -> anyhow::Result<()> {
        self.0
            .gatts
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatt interface"))?
            .replace(Arc::downgrade(gatts));

        let (tx, rx) = bounded(1);
        let callback_key = discriminant(&GattsEvent::ServiceRegistered {
            status: GattStatus::Busy,
            app_id: 0,
        });

        gatts
            .gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events"))?
            .insert(callback_key.clone(), tx.clone());

        gatts.gatts.register_app(self.0.id)?;

        match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(GattsEventMessage(interface, GattsEvent::ServiceRegistered { status, app_id })) => {
                if app_id != self.0.id {
                    return Err(anyhow::anyhow!("Received unexpected GATT: {:?}", app_id));
                }
                if status != GattStatus::Ok {
                    return Err(anyhow::anyhow!("Failed to register: {:?}", status));
                }

                self.0
                    .interface
                    .write()
                    .map_err(|_| anyhow::anyhow!("Failed to write Gatt interface"))?
                    .replace(interface);

                Ok(())
            }
            Ok(_) => Err(anyhow::anyhow!("Received unexpected GATT event")),
            Err(_) => Err(anyhow::anyhow!("Timed out waiting for GATT event")),
        }
    }

    pub fn register_service(
        &self,
        service_id: GattServiceId,
        num_handles: u16,
    ) -> anyhow::Result<Service> {
        Service::new(self.0.clone(), service_id, num_handles)
    }
}
