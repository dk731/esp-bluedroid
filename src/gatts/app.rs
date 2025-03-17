use std::{
    collections::HashMap,
    mem::discriminant,
    sync::{Arc, RwLock, Weak},
};

use crossbeam_channel::bounded;
use esp_idf_svc::bt::ble::gatt::{server::AppId, GattInterface, GattServiceId, GattStatus};

use super::{
    service::{Service, ServiceId, ServiceInner},
    GattsEvent, GattsEventMessage, GattsInner,
};

pub struct App(pub Arc<AppInner>);

pub struct AppInner {
    pub gatts: Weak<GattsInner>,
    pub gatt_interface: RwLock<Option<GattInterface>>,
    pub services: Arc<RwLock<HashMap<ServiceId, Arc<ServiceInner>>>>,

    pub id: AppId,
}

impl App {
    pub fn new(gatts: Arc<GattsInner>, app_id: AppId) -> anyhow::Result<Self> {
        let gatts = Arc::downgrade(&gatts);
        let app = AppInner {
            gatts,
            id: app_id,
            services: Arc::new(RwLock::new(HashMap::new())),
            gatt_interface: RwLock::new(None),
        };

        let app = Self(Arc::new(app));

        app.register_bluedroid()?;
        app.register_in_parent()?;

        Ok(app)
    }

    fn register_bluedroid(&self) -> anyhow::Result<()> {
        let (tx, rx) = bounded(1);
        let callback_key = discriminant(&GattsEvent::ServiceRegistered {
            status: GattStatus::Busy,
            app_id: 0,
        });

        let gatts = self
            .0
            .gatts
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;

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
                    .gatt_interface
                    .write()
                    .map_err(|_| anyhow::anyhow!("Failed to write Gatt interface"))?
                    .replace(interface);

                Ok(())
            }
            Ok(_) => Err(anyhow::anyhow!("Received unexpected GATT event")),
            Err(_) => Err(anyhow::anyhow!("Timed out waiting for GATT event")),
        }
    }

    fn register_in_parent(&self) -> anyhow::Result<()> {
        let gatts = self
            .0
            .gatts
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;

        if gatts
            .apps
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts"))?
            .insert(self.0.id, self.0.clone())
            .is_some()
        {
            log::warn!("App with ID {:?} already exists, replacing it", self.0.id);
        }

        Ok(())
    }

    pub fn register_service(
        &self,
        service_id: GattServiceId,
        num_handles: u16,
    ) -> anyhow::Result<Service> {
        Service::new(self.0.clone(), service_id, num_handles)
    }
}
