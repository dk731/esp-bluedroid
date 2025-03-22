use std::{
    collections::HashMap,
    fmt::Debug,
    mem::discriminant,
    sync::{mpsc, Arc, RwLock, Weak},
};

use crossbeam_channel::bounded;
use esp_idf_svc::bt::{
    ble::gatt::{GattId, GattServiceId, GattStatus, Handle},
    BdAddr, BtUuid,
};
use serde::{Deserialize, Serialize};

use super::{
    app::AppInner,
    characteristic::{AnyCharacteristic, Characteristic, CharacteristicConfig},
    GattsEvent, GattsEventMessage,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceId(GattServiceId);

impl std::hash::Hash for ServiceId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.id.inst_id.hash(state);
        self.0.id.uuid.as_bytes().hash(state);
    }
}

pub struct Service(pub Arc<ServiceInner>);

pub struct ServiceInner {
    pub app: Weak<AppInner>,
    pub id: GattServiceId,
    pub num_handles: u16,

    pub characteristics: Arc<RwLock<HashMap<Handle, Arc<dyn AnyCharacteristic>>>>,
    pub handle: RwLock<Option<Handle>>,
}

impl Service {
    pub fn new(
        app: Arc<AppInner>,
        service_id: GattServiceId,
        num_handles: u16,
    ) -> anyhow::Result<Self> {
        let app = Arc::downgrade(&app);
        let service = ServiceInner {
            app,
            id: service_id,
            handle: RwLock::new(None),
            num_handles,
            characteristics: Arc::new(RwLock::new(HashMap::new())),
        };

        let service = Self(Arc::new(service));

        service.register_bluedroid()?;
        service.register_in_parent()?;

        Ok(service)
    }

    fn register_bluedroid(&self) -> anyhow::Result<()> {
        let (tx, rx) = bounded(1);
        let callback_key = discriminant(&GattsEvent::ServiceCreated {
            status: GattStatus::Busy,
            service_handle: 0,
            service_id: GattServiceId {
                id: GattId {
                    uuid: BtUuid::uuid16(0),
                    inst_id: 0,
                },
                is_primary: false,
            },
        });

        let app = self
            .0
            .app
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade App"))?;
        let gatt_interface = app
            .interface
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read Gatt interface"))?
            .ok_or(anyhow::anyhow!(
                "Gatt interface is None, likely App was not initialized properly"
            ))?;

        let gatts = app
            .gatts
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;

        gatts
            .gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events"))?
            .insert(callback_key.clone(), tx.clone());

        gatts.gatts.create_service(gatt_interface, &self.0.id, 10)?;

        match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(GattsEventMessage(
                _,
                GattsEvent::ServiceCreated {
                    status,
                    service_handle,
                    service_id,
                },
            )) => {
                if service_id != self.0.id {
                    return Err(anyhow::anyhow!(
                        "Received unexpected GATT service id: {:?}",
                        service_id
                    ));
                }

                if status != GattStatus::Ok {
                    return Err(anyhow::anyhow!(
                        "Failed to create GATT service: {:?}",
                        status
                    ));
                }

                self.0
                    .handle
                    .write()
                    .map_err(|_| anyhow::anyhow!("Failed to write Service handle"))?
                    .replace(service_handle.clone());

                Ok(())
            }
            Ok(_) => Err(anyhow::anyhow!("Received unexpected GATT event")),
            Err(_) => Err(anyhow::anyhow!("Timed out waiting for GATT event")),
        }
    }

    fn register_in_parent(&self) -> anyhow::Result<()> {
        let app = self
            .0
            .app
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;

        if app
            .services
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts services"))?
            .insert(ServiceId(self.0.id.clone()), self.0.clone())
            .is_some()
        {
            log::warn!("App with ID {:?} already exists, replacing it", self.0.id);
        }

        Ok(())
    }

    pub fn register_characteristic<T>(
        &self,
        config: CharacteristicConfig,
        value: T,
    ) -> anyhow::Result<Characteristic<T>>
    where
        T: Serialize + for<'de> Deserialize<'de> + Sync + Send + Clone,
    {
        Characteristic::new(self.0.clone(), config, value)
    }

    pub fn start(&self) -> anyhow::Result<()> {
        let (tx, rx) = bounded(1);
        let callback_key = discriminant(&GattsEvent::ServiceStarted {
            status: GattStatus::Busy,
            service_handle: 0,
        });
        let app = self
            .0
            .app
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade App"))?;

        let gatts = app
            .gatts
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;

        let handle = self
            .0
            .handle
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read Service handle"))?
            .ok_or(anyhow::anyhow!(
                "Service handle is None, likely Service was not initialized properly"
            ))?;

        gatts
            .gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events"))?
            .insert(callback_key, tx);

        gatts.gatts.start_service(handle.clone())?;

        match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(GattsEventMessage(
                _,
                GattsEvent::ServiceStarted {
                    status,
                    service_handle,
                },
            )) => {
                if service_handle != handle {
                    return Err(anyhow::anyhow!(
                        "Received unexpected GATT service handle: {:?}",
                        service_handle
                    ));
                }

                if status != GattStatus::Ok {
                    return Err(anyhow::anyhow!("Failed to start service: {:?}", status));
                }

                Ok(())
            }
            Ok(_) => Err(anyhow::anyhow!("Received unexpected GATT")),
            Err(_) => Err(anyhow::anyhow!("Timed out waiting for GATT")),
        }
    }

    pub fn stop(&self) -> anyhow::Result<()> {
        let (tx, rx) = bounded(1);
        let callback_key = discriminant(&GattsEvent::ServiceStopped {
            status: GattStatus::Busy,
            service_handle: 0,
        });
        let app = self
            .0
            .app
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade App"))?;

        let gatts = app
            .gatts
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;

        let handle = self
            .0
            .handle
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read Service handle"))?
            .ok_or(anyhow::anyhow!(
                "Service handle is None, likly Service was not initialized properly"
            ))?;

        gatts
            .gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events"))?
            .insert(callback_key, tx);

        gatts.gatts.stop_service(handle.clone())?;

        match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(GattsEventMessage(
                _,
                GattsEvent::ServiceStopped {
                    status,
                    service_handle,
                },
            )) => {
                if service_handle != handle {
                    return Err(anyhow::anyhow!(
                        "Received unexpected GATT service handle: {:?}",
                        service_handle
                    ));
                }

                if status != GattStatus::Ok {
                    return Err(anyhow::anyhow!("Failed to stop service: {:?}", status));
                }

                Ok(())
            }
            Ok(_) => Err(anyhow::anyhow!("Received unexpected GATT")),
            Err(_) => Err(anyhow::anyhow!("Timed out waiting for GATT")),
        }
    }
}
