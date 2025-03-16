use std::{
    collections::HashMap,
    fmt::Debug,
    mem::discriminant,
    sync::{mpsc, Arc, RwLock, Weak},
};

use crossbeam_channel::bounded;
use esp_idf_svc::bt::{
    ble::gatt::{GattId, GattResponse, GattServiceId, GattStatus, Handle},
    BdAddr, BtUuid,
};
use serde::{Deserialize, Serialize};

use super::{
    app::AppInner,
    characteristic::{
        self, AnyCharacteristic, Characteristic, CharacteristicConfig, CharacteristicId,
    },
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

    pub characteristics: Arc<RwLock<HashMap<Handle, Arc<dyn AnyCharacteristic<'d> + 'd>>>>,
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
        service.configure_read_events()?;
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

        let service = self.0.clone();
        std::thread::spawn(move || {
            for event in rx.iter() {
                let send_response = || {
                    let GattsEventMessage(
                        _,
                        GattsEvent::Read {
                            conn_id,
                            trans_id,
                            addr,
                            handle,
                            offset,
                            is_long,
                            need_rsp,
                        },
                    ) = event
                    else {
                        return Err(anyhow::anyhow!("Received unexpected GATT event"));
                    };

                    // let characteristics = service
                    //     .characteristics
                    //     .read()
                    //     .map_err(|_| anyhow::anyhow!("Failed to read characteristics"))?;

                    // let Some(characteristic) = characteristics.get(&handle) else {
                    //     log::warn!("Received read request for unknown handle: {:?}", handle);
                    //     continue;
                    // };

                    // let Ok(characteristic_bytes) = characteristic.as_bytes() else {
                    //     log::error!("Failed to convert characteristic to bytes");
                    //     continue;
                    // };

                    // let Ok(app) = service
                    //     .app
                    //     .upgrade()
                    //     .ok_or(anyhow::anyhow!("Failed to upgrade App"))
                    // else {
                    //     log::error!("Failed to upgrade App");
                    //     continue;
                    // };

                    // let Ok(gatts) = app
                    //     .gatts
                    //     .upgrade()
                    //     .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))
                    // else {
                    //     log::error!("Failed to upgrade Gatts");
                    //     continue;
                    // };

                    // let Ok(response) = GattResponse::new()
                    //     .attr_handle(handle)
                    //     .auth_req(0)
                    //     .offset(offset)
                    //     .value(&characteristic_bytes)
                    // else {
                    //     log::error!("Failed to create GattResponse");
                    //     continue;
                    // };

                    // if let Err(err) = gatts.gatts.send_response(
                    //     app.gatt_interface.read().unwrap().unwrap(),
                    //     conn_id,
                    //     trans_id,
                    //     GattStatus::Ok,
                    //     Some(&response),
                    // ) {
                    //     log::error!("Failed to send response: {:?}", err);
                    //     continue;
                    // }

                    Ok(())
                };

                let a = send_response();

                // if let Err(err) = qwe {
                //     log::error!("Failed to handle GATT event: {:?}", err);
                // }
            }
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
                        "Received unexpected GATT application registration event: {:?}",
                        service_id
                    ));
                }

                if status != GattStatus::Ok {
                    return Err(anyhow::anyhow!(
                        "Failed to register GATT application: {:?}",
                        status
                    ));
                }

                self.0
                    .handle
                    .write()
                    .map_err(|_| {
                        anyhow::anyhow!("Failed to write Gatt interface after registration")
                    })?
                    .replace(service_handle.clone());

                Ok(())
            }
            Ok(_) => Err(anyhow::anyhow!(
                "Received unexpected GATT application registration event"
            )),
            Err(_) => Err(anyhow::anyhow!(
                "Timed out waiting for GATT application registration event"
            )),
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
                "Service handle is None, likly Service was not initialized properly"
            ))?;

        gatts
            .gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events after registration"))?
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
                        "Received unexpected GATT application registration event: {:?}",
                        service_handle
                    ));
                }

                if status != GattStatus::Ok {
                    return Err(anyhow::anyhow!(
                        "Failed to register GATT application: {:?}",
                        status
                    ));
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
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events after registration"))?
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
                        "Received unexpected GATT application registration event: {:?}",
                        service_handle
                    ));
                }

                if status != GattStatus::Ok {
                    return Err(anyhow::anyhow!(
                        "Failed to register GATT application: {:?}",
                        status
                    ));
                }

                Ok(())
            }
            Ok(_) => Err(anyhow::anyhow!("Received unexpected GATT")),
            Err(_) => Err(anyhow::anyhow!("Timed out waiting for GATT")),
        }
    }
}
