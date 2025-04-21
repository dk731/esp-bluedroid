use std::{
    collections::HashMap,
    mem::discriminant,
    sync::{Arc, RwLock, Weak},
};

use crossbeam_channel::bounded;
use esp_idf_svc::bt::ble::gatt::{
    server::{AppId, ConnectionId},
    GattInterface, GattStatus,
};

use super::{
    connection::ConnectionInner,
    service::{Service, ServiceId, ServiceInner},
    GattsEvent, GattsEventMessage, GattsInner,
};

#[derive(Clone)]
pub struct App(pub Arc<AppInner>);

pub struct AppInner {
    pub gatts: RwLock<Weak<GattsInner>>,
    pub interface: RwLock<Option<GattInterface>>,
    pub services: Arc<RwLock<HashMap<ServiceId, Arc<ServiceInner>>>>,
    pub connections: Arc<RwLock<HashMap<ConnectionId, ConnectionInner>>>,

    pub id: AppId,
}

impl App {
    pub fn new(app_id: AppId) -> Self {
        let app = AppInner {
            gatts: Default::default(),
            id: app_id,
            services: Default::default(),
            interface: RwLock::new(None),
            connections: Default::default(),
        };

        let app = Self(Arc::new(app));

        app
    }

    pub fn register_bluedroid(&self, gatts: &Arc<GattsInner>) -> anyhow::Result<()> {
        *self
            .0
            .gatts
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatt interface"))? =
            Arc::downgrade(gatts);

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

        gatts.gatts.register_app(self.0.id).map_err(|err| {
            anyhow::anyhow!("Failed to register GATT app {:?}: {:?}", self.0.id, err)
        })?;

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

    pub fn register_service(&self, service: &Service) -> anyhow::Result<Service> {
        service.register_bluedroid(&self.0)?;

        if self
            .0
            .services
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock on Gatts services"))?
            .insert(service.0.id.clone(), service.0.clone())
            .is_some()
        {
            return Err(anyhow::anyhow!(
                "Service with handle {:?} already exists",
                service.0.id
            ));
        }

        Ok(service.clone())
    }
}

impl AppInner {
    pub fn get_gatts(&self) -> anyhow::Result<Arc<GattsInner>> {
        self.gatts
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read Gatts"))?
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))
    }

    pub fn interface(&self) -> anyhow::Result<GattInterface> {
        self.interface
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read Gatt interface"))?
            .clone()
            .ok_or(anyhow::anyhow!("Gatt interface is not set"))
    }
}
