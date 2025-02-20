use std::{
    any,
    collections::HashMap,
    mem::discriminant,
    sync::{mpsc, Arc, RwLock, Weak},
};

use esp_idf_svc::bt::{
    ble::gatt::{server::AppId, GattInterface, GattServiceId, GattStatus},
    BtUuid,
};

use super::{
    service::{Service, ServiceInner},
    GattsEvent, GattsEventMessage, GattsInner,
};

pub struct App<'d>(pub Arc<AppInner<'d>>);

#[derive(Debug)]
pub struct AppInner<'d> {
    pub gatts: Weak<GattsInner<'d>>,
    pub gatt_interface: RwLock<Option<GattInterface>>,
    pub services: Arc<RwLock<HashMap<GattServiceId, Arc<ServiceInner<'d>>>>>,

    pub id: AppId,
}

impl<'d> App<'d> {
    pub fn new(gatts: Arc<GattsInner<'d>>, app_id: AppId) -> anyhow::Result<Self> {
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
        let (tx, rx) = mpsc::sync_channel(1);
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

        let callback_result = loop {
            match rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(GattsEventMessage(
                    interface,
                    GattsEvent::ServiceRegistered { status, app_id },
                )) => {
                    if app_id == self.0.id {
                        if status != GattStatus::Ok {
                            break Err(anyhow::anyhow!(
                                "Failed to register GATT application: {:?}",
                                status
                            ));
                        }

                        *self.0.gatt_interface.write().map_err(|_| {
                            anyhow::anyhow!("Failed to write Gatt interface after registration")
                        })? = Some(interface);
                        break Ok(());
                    }
                }
                Ok(_) => {
                    break Err(anyhow::anyhow!(
                        "Received unexpected GATT application registration event"
                    ));
                }
                Err(_) => {
                    break Err(anyhow::anyhow!(
                        "Timed out waiting for GATT application registration event"
                    ));
                }
            }
        };

        gatts
            .gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events after registration"))?
            .remove(&callback_key);

        callback_result?;

        Ok(())
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

    pub fn register_service(&self, service_id: GattServiceId) -> anyhow::Result<Service<'d>> {
        Service::new(self.0.clone(), service_id)
    }
}
