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
    characteristic::{AnyCharacteristic, Characteristic, CharacteristicConfig, CharacteristicId},
    GattsEvent, GattsEventMessage,
};

pub struct Service<'d>(pub Arc<ServiceInner<'d>>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceId(GattServiceId);

impl std::hash::Hash for ServiceId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.id.inst_id.hash(state);
        self.0.id.uuid.as_bytes().hash(state);
    }
}

pub struct ServiceInner<'d> {
    pub app: Weak<AppInner<'d>>,
    pub id: GattServiceId,
    pub num_handles: u16,

    pub characteristics:
        Arc<RwLock<HashMap<CharacteristicId, Arc<dyn AnyCharacteristic<'d> + 'd>>>>,
    pub handle: RwLock<Option<Handle>>,
}

impl<'d> Service<'d> {
    pub fn new(
        app: Arc<AppInner<'d>>,
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
        service.configure_read_write_events()?;
        service.register_in_parent()?;

        Ok(service)
    }

    fn configure_read_events(&self) -> anyhow::Result<()> {
        let app = self
            .0
            .app
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade App"))?;

        let gatt = app
            .gatts
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;

        let mut gatts_events = gatt
            .gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events after registration"))?;

        let (tx, rx) = bounded(1);
        gatts_events.insert(
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

        let service = Arc::downgrade(&self.0);
        std::thread::spawn(move || {
            let Some(service) = service.upgrade() else {
                log::error!("Failed to upgrade service in read events thread");
                return;
            };
        });

        Ok(())
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
            .gatt_interface
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read Gatt interface after registration"))?
            .ok_or(anyhow::anyhow!(
                "Gatt interface is None, likly App was not initialized properly"
            ))?;

        let gatts = app
            .gatts
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;

        gatts
            .gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events after registration"))?
            .insert(callback_key.clone(), tx.clone());

        gatts.gatts.create_service(gatt_interface, &self.0.id, 10)?;

        let callback_result = loop {
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
                        continue;
                    }

                    if status != GattStatus::Ok {
                        break Err(anyhow::anyhow!(
                            "Failed to register GATT application: {:?}",
                            status
                        ));
                    }

                    match self.0.handle.write().map_err(|_| {
                        anyhow::anyhow!("Failed to write Gatt interface after registration")
                    }) {
                        Ok(mut handle) => {
                            if handle.is_some() {
                                break Err(anyhow::anyhow!(
                                        "Service handle already set, likely Service was not initialized properly"
                                    ));
                            }
                            *handle = Some(service_handle);
                            break Ok(());
                        }
                        Err(_) => {
                            break Err(anyhow::anyhow!(
                                "Failed to write Gatt interface after registration"
                            ));
                        }
                    };
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
        let app = self
            .0
            .app
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;

        if app
            .services
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts"))?
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
    ) -> anyhow::Result<Characteristic<'d, T>>
    where
        T: Serialize + for<'de> Deserialize<'de> + Clone,
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
                "Service handle is None, likly Service was not initialized properly"
            ))?;

        gatts
            .gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events after registration"))?
            .insert(callback_key, tx);

        gatts.gatts.start_service(handle.clone())?;

        let callback_result = loop {
            match rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(GattsEventMessage(
                    _,
                    GattsEvent::ServiceStarted {
                        status,
                        service_handle,
                    },
                )) => {
                    if service_handle != handle {
                        continue;
                    }

                    if status != GattStatus::Ok {
                        break Err(anyhow::anyhow!("Failed to start service: {:?}", status));
                    }
                    break Ok(());
                }
                Ok(_) => {
                    break Err(anyhow::anyhow!("Received unexpected GATT"));
                }
                Err(_) => {
                    break Err(anyhow::anyhow!("Timed out waiting for GATT"));
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
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events after registration"))?
            .insert(callback_key, tx);

        gatts.gatts.stop_service(handle.clone())?;

        let callback_result = loop {
            match rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(GattsEventMessage(
                    _,
                    GattsEvent::ServiceStarted {
                        status,
                        service_handle,
                    },
                )) => {
                    if service_handle != handle {
                        continue;
                    }

                    if status != GattStatus::Ok {
                        break Err(anyhow::anyhow!("Failed to stop service: {:?}", status));
                    }
                    break Ok(());
                }
                Ok(_) => {
                    break Err(anyhow::anyhow!("Received unexpected GATT"));
                }
                Err(_) => {
                    break Err(anyhow::anyhow!("Timed out waiting for GATT"));
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
}
