use std::{
    collections::HashMap,
    fmt::Debug,
    mem::discriminant,
    sync::{Arc, RwLock, Weak},
};

use crossbeam_channel::bounded;
use esp_idf_svc::bt::{
    ble::gatt::{GattId, GattServiceId, GattStatus, Handle},
    BtUuid,
};

use super::{
    app::AppInner,
    attribute::Attribute,
    characteristic::{Characteristic, CharacteristicAttribute},
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

#[derive(Clone)]
pub struct Service(pub Arc<ServiceInner>);

pub struct ServiceInner {
    pub app: RwLock<Weak<AppInner>>,
    pub id: ServiceId,
    pub num_handles: u16,

    pub characteristics: Arc<RwLock<HashMap<Handle, Arc<dyn CharacteristicAttribute>>>>,
    pub handle: RwLock<Option<Handle>>,
}

impl Service {
    pub fn new(service_id: GattServiceId, num_handles: u16) -> Self {
        let service = ServiceInner {
            app: Default::default(),
            id: ServiceId(service_id),
            handle: RwLock::new(None),
            num_handles,
            characteristics: Default::default(),
        };

        Self(Arc::new(service))
    }

    pub fn register_bluedroid(&self, app: &Arc<AppInner>) -> anyhow::Result<()> {
        *self
            .0
            .app
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatt interface"))? = Arc::downgrade(app);

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

        let gatt_interface = app.interface()?;
        let gatts = app.get_gatts()?;

        gatts
            .gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events"))?
            .insert(callback_key.clone(), tx.clone());

        gatts
            .gatts
            .create_service(gatt_interface, &self.0.id.0, 10)
            .map_err(|err| {
                anyhow::anyhow!("Failed to create GATT service {:?}: {:?}", self.0.id, err)
            })?;

        match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(GattsEventMessage(
                interface,
                GattsEvent::ServiceCreated {
                    status,
                    service_handle,
                    service_id,
                },
            )) => {
                if interface != gatt_interface {
                    return Err(anyhow::anyhow!(
                        "Received unexpected GATT interface: {:?}",
                        interface
                    ));
                }

                if service_id != self.0.id.0 {
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

    pub fn register_characteristic<T: Attribute>(
        &self,
        characteristic: &Characteristic<T>,
    ) -> anyhow::Result<Characteristic<T>> {
        characteristic.register_bluedroid(&self.0)?;
        let characteristic_handle = characteristic.0.handle()?;
        let app = self.0.get_app()?;
        let gatts = app.get_gatts()?;

        if self
            .0
            .characteristics
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock on Gatts services"))?
            .insert(characteristic_handle, characteristic.0.clone())
            .is_some()
        {
            return Err(anyhow::anyhow!(
                "Characteristic with handle {:?} already exists",
                characteristic_handle
            ));
        }

        let global_attributes = gatts
            .attributes
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock on Gatts services"))?;

        if global_attributes
            .insert(characteristic_handle, characteristic.0.clone())
            .is_some()
        {
            return Err(anyhow::anyhow!("Failed to write Gatt attributes"));
        }

        for descriptor in characteristic.0.descriptors {
            let descriptor_handle = descriptor.1.handle()?;

            if global_attributes
                .insert(descriptor_handle, descriptor.1.clone())
                .is_some()
            {
                return Err(anyhow::anyhow!(
                    "Descriptor with handle {:?} already exists",
                    descriptor_handle
                ));
            }
        }

        Ok(characteristic.clone())
    }

    pub fn start(&self) -> anyhow::Result<()> {
        let (tx, rx) = bounded(1);
        let callback_key = discriminant(&GattsEvent::ServiceStarted {
            status: GattStatus::Busy,
            service_handle: 0,
        });

        let app = self.0.get_app()?;
        let gatts = app.get_gatts()?;
        let handle = self.0.get_handle()?;

        gatts
            .gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events"))?
            .insert(callback_key, tx);

        gatts.gatts.start_service(handle.clone()).map_err(|err| {
            anyhow::anyhow!("Failed to start GATT service {:?}: {:?}", handle, err)
        })?;

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
        let app = self.0.get_app()?;
        let gatts = app.get_gatts()?;
        let handle = self.0.get_handle()?;

        gatts
            .gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events"))?
            .insert(callback_key, tx);

        gatts.gatts.stop_service(handle.clone()).map_err(|err| {
            anyhow::anyhow!("Failed to stop GATT service {:?}: {:?}", handle, err)
        })?;

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

impl ServiceInner {
    pub fn get_app(&self) -> anyhow::Result<Arc<AppInner>> {
        self.app
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read App"))?
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))
    }

    pub fn get_handle(&self) -> anyhow::Result<Handle> {
        self.handle
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read Service handle"))?
            .ok_or(anyhow::anyhow!("Service handle is not set"))
    }
}
