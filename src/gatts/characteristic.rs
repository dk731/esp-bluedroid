use std::{
    any,
    mem::discriminant,
    sync::{mpsc, Arc, RwLock, Weak},
};

use enumset::{enum_set, EnumSet};
use esp_idf_svc::bt::{
    ble::gatt::{AutoResponse, GattCharacteristic, GattStatus, Handle, Permission, Property},
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

pub struct CharacteristicId(BtUuid);
impl std::hash::Hash for CharacteristicId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_bytes().hash(state);
    }
}

pub trait AnyCharacteristic {
    fn as_bytes(&self) -> anyhow::Result<Vec<u8>>;
    fn update_from_bytes(&self, data: &[u8]) -> anyhow::Result<()>;
}

pub struct Characteristic<'d, T: Serialize + for<'de> Deserialize<'de> + Clone>(
    Arc<CharacteristicInner<'d, T>>,
);

impl<'d, T> AnyCharacteristic for Characteristic<'d, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone,
{
    fn as_bytes(&self) -> anyhow::Result<Vec<u8>> {
        bincode::serde::encode_to_vec(
            self.0
                .value
                .read()
                .map_err(|_| anyhow::anyhow!("Failed to read characteristic value"))?
                .clone(),
            bincode::config::standard(),
        )
        .map_err(|err| {
            anyhow::anyhow!(
                "Failed to serialize characteristic value to bytes: {:?}",
                err
            )
        })
    }

    fn update_from_bytes(&self, data: &[u8]) -> anyhow::Result<()> {
        let (value, _): (T, usize) =
            bincode::serde::decode_from_slice(data, bincode::config::standard()).map_err(
                |err| {
                    anyhow::anyhow!(
                        "Failed to deserialize bytes to characteristic value: {:?}",
                        err
                    )
                },
            )?;

        self.0
            .value
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write characteristic value"))?
            .clone_from(&value);

        Ok(())
    }
}

pub struct CharacteristicInner<'d, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone,
{
    pub service: Weak<ServiceInner<'d>>,
    value: RwLock<T>,

    pub config: CharacteristicConfig,
    pub handle: RwLock<Option<Handle>>,
}

impl<'d, T> Characteristic<'d, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone,
{
    pub fn new(
        service: Arc<ServiceInner<'d>>,
        config: CharacteristicConfig,
        value: T,
    ) -> anyhow::Result<Self> {
        let service = Arc::downgrade(&service);
        let characterstic = CharacteristicInner {
            service,
            value: RwLock::new(value),
            handle: RwLock::new(None),
            config,
        };

        let characterstic = Self(Arc::new(characterstic));

        characterstic.register_bluedroid_characteristic()?;
        characterstic.register_bluedroid_descriptors()?;

        characterstic.register_in_parent()?;

        Ok(characterstic)
    }

    fn register_bluedroid_descriptors(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn register_bluedroid_characteristic(&self) -> anyhow::Result<()> {
        let (tx, rx) = mpsc::sync_channel(1);
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
            .map_err(|_| anyhow::anyhow!("Failed to read Service handle after registration"))?
            .ok_or(anyhow::anyhow!(
                "Service handle is None, likely Service was not initialized properly"
            ))?;

        let app = service
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
            .insert(callback_key, tx);

        let current_data = bincode::serde::encode_to_vec(
            self.0
                .value
                .read()
                .map_err(|_| {
                    anyhow::anyhow!("Failed to read characteristic value after registration")
                })?
                .clone(),
            bincode::config::standard(),
        )?;

        gatts
            .gatts
            .add_characteristic(service_handle, &(&self.0.config).into(), &current_data)?;

        let callback_result = loop {
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
                        continue;
                    }

                    if service_handle != service_handle {
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
                                    "Gatt interface is already set, likely App was not initialized properly"
                                ));
                            }
                            *handle = Some(attr_handle);
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
        let service = self
            .0
            .service
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;

        if service
            .characteristics
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatt interface after registration"))?
            .insert(123, self.0.clone())
            .is_some()
        {
            // log::warn!(
            //     "App with ID {:?} already exists, replacing it",
            //     self.0.service_id
            // );
        }

        Ok(())
    }
}
