use std::{
    mem::discriminant,
    sync::{Arc, RwLock, Weak},
};

use crossbeam_channel::{bounded, Receiver, Sender};
use enumset::{enum_set, EnumSet};
use esp_idf_svc::bt::{
    ble::gatt::{
        AutoResponse, GattCharacteristic, GattDescriptor, GattStatus, Handle, Permission, Property,
    },
    BtUuid,
};
use serde::{Deserialize, Serialize};

use super::{
    attribute::{Attribute, AttributeInner, AttributeUpdate, SerializableAttribute},
    event::GattsEventMessage,
    service::{Service, ServiceInner},
    GattsEvent,
};

pub struct CharacteristicConfig {
    pub uuid: BtUuid,
    pub value_max_len: usize,

    pub readable: bool,
    pub writable: bool,

    // If true, the characteristic will be broadcasted to all connected devices
    // this will automatically configure SCCD descriptor
    pub broadcasted: bool,

    // If any of this are true, Characteristic will automatically configure
    // CCCD descriptor
    pub enable_notify: bool,
}

impl Into<GattCharacteristic> for &CharacteristicConfig {
    fn into(self) -> GattCharacteristic {
        let mut permissions = EnumSet::new();
        let mut properties = EnumSet::new();

        if self.readable {
            permissions.insert(Permission::Read);
            properties.insert(Property::Read);
        }

        if self.writable {
            permissions.insert(Permission::Write);
            properties.insert(Property::Write);
        }

        if self.broadcasted {
            properties.insert(Property::Broadcast);
        }

        if self.enable_notify {
            properties.insert(Property::Notify);
        }

        if self.enable_notify {
            properties.insert(Property::Indicate);
        }

        GattCharacteristic {
            uuid: self.uuid.clone(),
            permissions,
            properties,
            max_len: self.value_max_len,
            auto_rsp: AutoResponse::ByApp,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CharacteristicId(BtUuid);
impl std::hash::Hash for CharacteristicId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_bytes().hash(state);
    }
}

#[derive(Clone)]
pub struct Characteristic<T: Attribute>(
    pub Arc<CharacteristicInner<T>>,
    std::marker::PhantomData<T>,
);

pub struct CharacteristicInner<T: Attribute> {
    pub service: RwLock<Weak<ServiceInner>>,
    // value: RwLock<Arc<T>>,
    pub config: CharacteristicConfig,
    pub handle: RwLock<Option<Handle>>,

    attribute: RwLock<AttributeInner>,
    _p: std::marker::PhantomData<T>,
    // attribute: dyn TypedAttribute,
    // updates_tx: Sender<CharacteristicUpdate<T>>,
    // pub updates_rx: Receiver<CharacteristicUpdate<T>>,
}

impl<T: Attribute> Characteristic<T> {
    pub fn new(config: CharacteristicConfig, value: T) -> Self {
        let characterstic = CharacteristicInner {
            service: RwLock::new(Weak::new()),
            handle: RwLock::new(None),
            config,
            // attribute: RwLock::new(AttributeInner::new(Arc::new(value))),
            // value: RwLock::new(Arc::new(value)),
            attribute: RwLock::new(todo!()),
            _p: std::marker::PhantomData,
        };

        let characterstic = Self(Arc::new(characterstic), std::marker::PhantomData);

        characterstic
    }

    pub fn register_bluedroid(&self, service: &Arc<ServiceInner>) -> anyhow::Result<()> {
        *self
            .0
            .service
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Service"))? = Arc::downgrade(service);

        self.register_characteristic()?;

        Ok(())
    }

    fn register_descriptors(&self) -> anyhow::Result<()> {
        let service = self.0.get_service()?;
        let app = service.get_app()?;
        let gatts = app.get_gatts()?;

        let service_handle = service
            .handle
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read Service handle"))?
            .ok_or(anyhow::anyhow!(
                "Service handle is None, likely Service was not initialized properly"
            ))?;

        // gatts.gatts.add_descriptor(service_handle, descriptor)

        Ok(())
    }

    fn register_characteristic(&self) -> anyhow::Result<()> {
        let (tx, rx) = bounded(1);
        let callback_key = discriminant(&GattsEvent::CharacteristicAdded {
            status: GattStatus::Busy,
            attr_handle: 0,
            service_handle: 0,
            char_uuid: BtUuid::uuid16(0),
        });

        // self.0.
        let service = self.0.get_service()?;
        let app = service.get_app()?;
        let gatts = app.get_gatts()?;

        let service_handle = service
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

        gatts
            .gatts
            .add_characteristic(service_handle, &(&self.0.config).into(), &[])
            .map_err(|err| {
                anyhow::anyhow!(
                    "Failed to register GATT characteristic {:?}: {:?}",
                    self.0.config.uuid,
                    err
                )
            })?;

        match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(GattsEventMessage(
                _,
                GattsEvent::CharacteristicAdded {
                    status,
                    attr_handle,
                    service_handle,
                    char_uuid,
                },
            )) => {
                if char_uuid != self.0.config.uuid {
                    return Err(anyhow::anyhow!(
                        "Received unexpected GATT characteristic UUID: {:?}",
                        char_uuid
                    ));
                }

                if service_handle != service_handle {
                    return Err(anyhow::anyhow!(
                        "Received unexpected GATT service handle: {:?}",
                        service_handle
                    ));
                }

                if status != GattStatus::Ok {
                    return Err(anyhow::anyhow!(
                        "Failed to add characteristic: {:?}",
                        status
                    ));
                }

                self.0
                    .handle
                    .write()
                    .map_err(|_| anyhow::anyhow!("Failed to write Gatt interface"))?
                    .replace(attr_handle);

                Ok(())
            }
            Ok(_) => Err(anyhow::anyhow!("Received unexpected GATT")),
            Err(_) => Err(anyhow::anyhow!("Timed out waiting for GATT event")),
        }
    }

    // This locks internal value, so while lock is held, characteristic value cannot be changed
    pub fn value(&self) -> anyhow::Result<Arc<T>> {
        // Ok(self
        //     .0
        //     .value
        //     .read()
        //     .map_err(|_| anyhow::anyhow!("Failed to read characteristic value"))?
        //     .clone())

        let data = self
            .0
            .attribute
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read characteristic attribute value"))?
            .get_bytes()? as T

        // let a = self
        //     .0
        //     .attribute
        //     .read()
        //     .map_err(|_| anyhow::anyhow!("Failed to read characteristic attribute value"))?;
        // a.get_bytes()?;

        todo!()
    }

    pub fn update_value(&self, value: T) -> anyhow::Result<()> {
        // let mut current_value = self
        //     .0
        //     .value
        //     .write()
        //     .map_err(|_| anyhow::anyhow!("Failed to write characteristic value"))?;

        // let update = AttributeUpdate {
        //     old: current_value.clone(),
        //     new: Arc::new(value),
        // };
        // *current_value = update.new.clone();

        // self.0.handle_value_update(update)?;

        Ok(())
    }
}

impl<T: Attribute> CharacteristicInner<T> {
    fn handle_value_update(&self, update: AttributeUpdate<T>) -> anyhow::Result<()> {
        // let (tx, rx) = bounded(1);
        let callback_key = discriminant(&GattsEvent::Confirm {
            status: GattStatus::Busy,
            conn_id: 0,
            handle: 0,
            value: None,
        });

        // self.updates_tx
        //     .send(update.clone())
        //     .map_err(|_| anyhow::anyhow!("Failed to send characteristic update"))?;

        // let service = self.get_service()?;
        // let app = service.get_app()?;
        // let gatts = app.get_gatts()?;

        // let gatts_interface = app
        //     .interface
        //     .read()
        //     .map_err(|_| anyhow::anyhow!("Failed to read Gatt interface"))?
        //     .clone()
        //     .ok_or(anyhow::anyhow!("Gatt interface is not initialized"))?;

        // let connections = app
        //     .connections
        //     .read()
        //     .map_err(|_| anyhow::anyhow!("Failed to read connections in App: {:?}", app.id))?;

        // let characteristic_handle = self
        //     .handle
        //     .read()
        //     .map_err(|_| anyhow::anyhow!("Failed to read handle"))?
        //     .ok_or(anyhow::anyhow!(
        //         "Handle in None, likely Characteristic was not initialized properly"
        //     ))?;

        // let notify_data =
        //     bincode::serde::encode_to_vec((*update.new).clone(), bincode::config::standard())?;

        // gatts
        //     .gatts_events
        //     .write()
        //     .map_err(|_| anyhow::anyhow!("Failed to write Gatts events in App: {:?}", app.id))?
        //     .insert(callback_key, tx);

        // let send_results = connections
        //     .values()
        //     .map(|connection| {
        //         let mtu = connection.mtu.ok_or(anyhow::anyhow!(
        //             "Failed to read MTU for connection: {:?}",
        //             connection.id
        //         ))?;
        //         let data_end_index = notify_data.len().min(mtu.into());

        //         if data_end_index != notify_data.len() {
        //             log::warn!(
        //                 "Data is too long to be sent, MTU is too small, cutting data: {:?}",
        //                 mtu
        //             );
        //             // return Err(anyhow::anyhow!(
        //             //     "Data is too long to be sent, MTU is too small: {:?}",
        //             //     mtu
        //             // ));
        //         }

        //         gatts
        //             .gatts
        //             .indicate(
        //                 gatts_interface,
        //                 connection.id,
        //                 characteristic_handle,
        //                 &notify_data[..data_end_index],
        //             )
        //             .map_err(|err| {
        //                 anyhow::anyhow!(
        //                     "Failed to send GATT indication to {:?}: {:?}",
        //                     connection.address,
        //                     err
        //                 )
        //             })?;

        //         match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        //             Ok(GattsEventMessage(
        //                 _,
        //                 GattsEvent::Confirm {
        //                     status,
        //                     conn_id,
        //                     handle,
        //                     ..
        //                 },
        //             )) => {
        //                 if conn_id != connection.id {
        //                     return Err(anyhow::anyhow!(
        //                         "Received unexpected GATT confirm: {:?}",
        //                         conn_id
        //                     ));
        //                 }

        //                 if handle != characteristic_handle {
        //                     return Err(anyhow::anyhow!(
        //                         "Received unexpected GATT confirm handle: {:?}",
        //                         handle
        //                     ));
        //                 }

        //                 if status != GattStatus::Ok {
        //                     return Err(anyhow::anyhow!(
        //                         "Failed to confirm characteristic indicate: {:?}",
        //                         status
        //                     ));
        //                 }

        //                 Ok(())
        //             }
        //             Ok(_) => Err(anyhow::anyhow!("Received unexpected GATT")),
        //             Err(_) => Err(anyhow::anyhow!("Timed out waiting for GATT")),
        //         }
        //     })
        //     .collect::<Vec<anyhow::Result<()>>>();

        // let errors: Vec<anyhow::Error> = send_results
        //     .into_iter()
        //     .filter_map(anyhow::Result::err)
        //     .collect();

        // if !errors.is_empty() {
        //     return Err(anyhow::anyhow!(
        //         "Failed to notify some of connections: {:?}",
        //         errors
        //     ));
        // }

        Ok(())
    }
}

impl<T: Attribute> CharacteristicInner<T> {
    fn get_service(&self) -> anyhow::Result<Arc<ServiceInner>> {
        self.service
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read Service"))?
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Service"))
    }
}
