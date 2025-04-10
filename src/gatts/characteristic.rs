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

use super::{event::GattsEventMessage, service::ServiceInner, GattsEvent};

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
    pub notifiable: bool,
    pub indicateable: bool,
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

        if self.notifiable {
            properties.insert(Property::Notify);
        }

        if self.indicateable {
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

pub trait AnyCharacteristic: Send + Sync {
    fn as_bytes(&self) -> anyhow::Result<Vec<u8>>;
    fn update_from_bytes(&self, data: &[u8]) -> anyhow::Result<()>;
}

impl<T> AnyCharacteristic for CharacteristicInner<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Sync + Send + 'static,
{
    fn as_bytes(&self) -> anyhow::Result<Vec<u8>> {
        let value_lock = self
            .value
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read characteristic value"))?;

        bincode::serde::encode_to_vec((**value_lock).clone(), bincode::config::standard()).map_err(
            |err| {
                anyhow::anyhow!(
                    "Failed to serialize characteristic value to bytes: {:?}",
                    err
                )
            },
        )
    }

    fn update_from_bytes(&self, data: &[u8]) -> anyhow::Result<()> {
        let (new_value, _): (T, usize) =
            bincode::serde::decode_from_slice(data, bincode::config::standard()).map_err(
                |err| {
                    anyhow::anyhow!(
                        "Failed to deserialize bytes to characteristic value: {:?}",
                        err
                    )
                },
            )?;

        let mut current_value = self
            .value
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write characteristic value"))?;

        let update = CharacteristicUpdate {
            old: current_value.clone(),
            new: Arc::new(new_value),
        };
        *current_value = update.new.clone();

        self.handle_value_update(update)?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct Characteristic<T: Serialize + for<'de> Deserialize<'de> + Clone + Sync + Send + 'static>(
    pub Arc<CharacteristicInner<T>>,
);

#[derive(Clone)]
pub struct CharacteristicUpdate<T> {
    pub old: Arc<T>,
    pub new: Arc<T>,
}

pub struct CharacteristicInner<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Sync + Send + 'static,
{
    pub service: Weak<ServiceInner>,
    value: RwLock<Arc<T>>,

    pub config: CharacteristicConfig,
    pub handle: RwLock<Option<Handle>>,

    updates_tx: Sender<CharacteristicUpdate<T>>,
    pub updates_rx: Receiver<CharacteristicUpdate<T>>,
}

impl<T> Characteristic<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Sync + Send + 'static,
{
    pub fn new(
        service: Arc<ServiceInner>,
        config: CharacteristicConfig,
        value: T,
    ) -> anyhow::Result<Self> {
        let service = Arc::downgrade(&service);
        let (tx, rx) = bounded(1);

        let characterstic = CharacteristicInner {
            service,
            value: RwLock::new(Arc::new(value)),
            handle: RwLock::new(None),
            config,
            updates_tx: tx.clone(),
            updates_rx: rx.clone(),
        };

        let characterstic = Self(Arc::new(characterstic));

        characterstic.register_bluedroid_characteristic()?;
        characterstic.register_bluedroid_descriptors()?;

        characterstic.register_in_parent()?;

        Ok(characterstic)
    }

    fn register_bluedroid_descriptors(&self) -> anyhow::Result<()> {
        let service = self
            .0
            .service
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Service"))?;
        let service_handle = service
            .handle
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read Service handle"))?
            .ok_or(anyhow::anyhow!(
                "Service handle is None, likely Service was not initialized properly"
            ))?;

        let app = service
            .app
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade App"))?;

        let gatts = app
            .gatts
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;

        if self.0.config.notifiable || self.0.config.indicateable {
            gatts.gatts.add_descriptor(
                service_handle,
                &GattDescriptor {
                    uuid: BtUuid::uuid128(0x2902),
                    permissions: enum_set!(Permission::Read | Permission::Write),
                },
            )?;
        }

        Ok(())
    }

    fn register_bluedroid_characteristic(&self) -> anyhow::Result<()> {
        let (tx, rx) = bounded(1);
        let callback_key = discriminant(&GattsEvent::CharacteristicAdded {
            status: GattStatus::Busy,
            attr_handle: 0,
            service_handle: 0,
            char_uuid: BtUuid::uuid16(0),
        });

        let service = self
            .0
            .service
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Service"))?;
        let service_handle = service
            .handle
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read Service handle"))?
            .ok_or(anyhow::anyhow!(
                "Service handle is None, likely Service was not initialized properly"
            ))?;

        let app = service
            .app
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade App"))?;

        let gatts = app
            .gatts
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;
        gatts
            .gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events"))?
            .insert(callback_key, tx);

        let characteristic_value = self
            .0
            .value
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read characteristic value"))?;

        let current_data = bincode::serde::encode_to_vec(
            (**characteristic_value).clone(),
            bincode::config::standard(),
        )?;

        gatts
            .gatts
            .add_characteristic(service_handle, &(&self.0.config).into(), &current_data)?;

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

    fn register_in_parent(&self) -> anyhow::Result<()> {
        let service = self
            .0
            .service
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Service"))?;

        let handle = self
            .0
            .handle
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read handle"))?
            .ok_or(anyhow::anyhow!(
                "Handle in None, likely Characteristic was not initialized properly"
            ))?;

        if service
            .characteristics
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write service characteristics"))?
            .insert(handle.clone(), self.0.clone())
            .is_some()
        {
            log::warn!(
                "Characteristic with UUID {:?} already exists, replacing it",
                self.0.config.uuid
            );
        }

        Ok(())
    }

    // This locks internal value, so while lock is held, characteristic value cannot be changed
    pub fn value(&self) -> anyhow::Result<Arc<T>> {
        Ok(self
            .0
            .value
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read characteristic value"))?
            .clone())
    }

    pub fn update_value(&self, value: T) -> anyhow::Result<()> {
        let mut current_value = self
            .0
            .value
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write characteristic value"))?;

        let update = CharacteristicUpdate {
            old: current_value.clone(),
            new: Arc::new(value),
        };
        *current_value = update.new.clone();

        self.0.handle_value_update(update)?;

        Ok(())
    }
}

impl<T> CharacteristicInner<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Sync + Send + 'static,
{
    fn handle_value_update(&self, update: CharacteristicUpdate<T>) -> anyhow::Result<()> {
        let (tx, rx) = bounded(1);
        let callback_key = discriminant(&GattsEvent::Confirm {
            status: GattStatus::Busy,
            conn_id: 0,
            handle: 0,
            value: None,
        });

        self.updates_tx
            .send(update.clone())
            .map_err(|_| anyhow::anyhow!("Failed to send characteristic update"))?;

        let service = self
            .service
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Service"))?;

        let app = service
            .app
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade App"))?;
        let gatts_interface = app
            .interface
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read Gatt interface"))?
            .clone()
            .ok_or(anyhow::anyhow!("Gatt interface is not initialized"))?;

        let gatts = app
            .gatts
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;

        let connections = app
            .connections
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read connections in App: {:?}", app.id))?;

        let characteristic_handle = self
            .handle
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read handle"))?
            .ok_or(anyhow::anyhow!(
                "Handle in None, likely Characteristic was not initialized properly"
            ))?;

        let notify_data =
            bincode::serde::encode_to_vec((*update.new).clone(), bincode::config::standard())?;

        gatts
            .gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events in App: {:?}", app.id))?
            .insert(callback_key, tx);

        let results = connections
            .values()
            .map(|connection| {
                let mtu = connection.mtu.ok_or(anyhow::anyhow!(
                    "Failed to read MTU for connection: {:?}",
                    connection.id
                ))?;
                let data_end_index = notify_data.len().min(mtu.into());

                if data_end_index != notify_data.len() {
                    log::warn!(
                        "Data is too long to be sent, MTU is too small, cutting data: {:?}",
                        mtu
                    );
                    // return Err(anyhow::anyhow!(
                    //     "Data is too long to be sent, MTU is too small: {:?}",
                    //     mtu
                    // ));
                }

                gatts.gatts.indicate(
                    gatts_interface,
                    connection.id,
                    characteristic_handle,
                    &notify_data[..data_end_index],
                )?;

                match rx.recv_timeout(std::time::Duration::from_secs(5)) {
                    Ok(GattsEventMessage(
                        _,
                        GattsEvent::Confirm {
                            status,
                            conn_id,
                            handle,
                            ..
                        },
                    )) => {
                        if conn_id != connection.id {
                            return Err(anyhow::anyhow!(
                                "Received unexpected GATT confirm: {:?}",
                                conn_id
                            ));
                        }

                        if handle != characteristic_handle {
                            return Err(anyhow::anyhow!(
                                "Received unexpected GATT confirm handle: {:?}",
                                handle
                            ));
                        }

                        if status != GattStatus::Ok {
                            return Err(anyhow::anyhow!(
                                "Failed to confirm characteristic indicate: {:?}",
                                status
                            ));
                        }

                        Ok(())
                    }
                    Ok(_) => Err(anyhow::anyhow!("Received unexpected GATT")),
                    Err(_) => Err(anyhow::anyhow!("Timed out waiting for GATT")),
                }
            })
            .collect::<anyhow::Result<()>>();

        Ok(())
    }
}
