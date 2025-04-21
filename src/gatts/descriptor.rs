use std::{
    mem::discriminant,
    sync::{Arc, RwLock, Weak},
};

use crossbeam_channel::bounded;
use enumset::EnumSet;
use esp_idf_svc::bt::{
    ble::gatt::{GattDescriptor, GattStatus, Permission},
    BtUuid,
};

use super::{
    attribute::{Attribute, AttributeInner},
    characteristic::{self, Characteristic, CharacteristicInner},
    event::{GattsEvent, GattsEventMessage},
};

pub struct DescriptorConfig {
    pub uuid: BtUuid,
    pub value_max_len: usize,

    pub readable: bool,
    pub writable: bool,
}

impl Into<GattDescriptor> for &DescriptorConfig {
    fn into(self) -> GattDescriptor {
        let mut permissions = EnumSet::new();

        if self.readable {
            permissions.insert(Permission::Read);
        }

        if self.writable {
            permissions.insert(Permission::Write);
        }

        GattDescriptor {
            uuid: self.uuid.clone(),
            permissions,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescritporId(pub BtUuid);

impl std::hash::Hash for DescritporId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_bytes().hash(state);
    }
}

pub trait DescriptorAttribute<A: Attribute>: Send + Sync + 'static {
    fn update_from_bytes(&self, bytes: &[u8]) -> anyhow::Result<()>;
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>>;
    fn register(&self, service: &Arc<CharacteristicInner<A>>) -> anyhow::Result<()>;
    fn uuid(&self) -> BtUuid;
}

#[derive(Clone)]
pub struct Descriptor<T: Attribute, A: Attribute>(pub Arc<DescriptorInner<T, A>>);

pub struct DescriptorInner<T: Attribute, A: Attribute> {
    pub characteristic: RwLock<Weak<CharacteristicInner<A>>>,
    pub config: DescriptorConfig,

    pub attribute: AttributeInner<T>,
}

impl<T: Attribute, A: Attribute> Descriptor<T, A> {
    pub fn new(value: T, config: DescriptorConfig) -> Self {
        let descriptor = DescriptorInner::<T, A> {
            characteristic: RwLock::new(Weak::new()),
            config,
            attribute: AttributeInner::new(value),
        };

        Self(Arc::new(descriptor))
    }

    pub fn register(&self, service: &Arc<CharacteristicInner<A>>) -> anyhow::Result<()> {
        *self
            .0
            .characteristic
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Service"))? = Arc::downgrade(service);

        Ok(())
    }
}

impl<T: Attribute, A: Attribute> DescriptorAttribute<A> for DescriptorInner<T, A> {
    fn update_from_bytes(&self, bytes: &[u8]) -> anyhow::Result<()> {
        self.attribute.update(Arc::new(T::from_bytes(bytes)?))
    }

    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        self.attribute.get_bytes()
    }

    fn register(&self, characteristic: &Arc<CharacteristicInner<A>>) -> anyhow::Result<()> {
        *self
            .characteristic
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Service"))? =
            Arc::downgrade(characteristic);

        let (tx, rx) = bounded(1);
        let callback_key = discriminant(&GattsEvent::DescriptorAdded {
            status: GattStatus::Busy,
            attr_handle: 0,
            service_handle: 0,
            descr_uuid: BtUuid::uuid16(0),
        });

        let service = characteristic.get_service()?;
        let app = service.get_app()?;
        let gatts = app.get_gatts()?;
        let parent_service_handle = service.get_handle()?;

        gatts
            .gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events"))?
            .insert(callback_key.clone(), tx.clone());

        gatts
            .gatts
            .add_descriptor(parent_service_handle, &(&self.config).into())
            .map_err(|err| {
                anyhow::anyhow!(
                    "Failed to register GATT descriptor {:?}: {:?}",
                    self.config.uuid,
                    err
                )
            })?;

        match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(GattsEventMessage(
                interface,
                GattsEvent::DescriptorAdded {
                    status,
                    attr_handle,
                    service_handle,
                    descr_uuid,
                },
            )) => {
                if interface != app.interface()? {
                    return Err(anyhow::anyhow!(
                        "Received unexpected GATT interface: {:?}",
                        interface
                    ));
                }

                if service_handle != parent_service_handle {
                    return Err(anyhow::anyhow!(
                        "Received unexpected GATT: {:?}",
                        service_handle
                    ));
                }

                if self.config.uuid != descr_uuid {
                    return Err(anyhow::anyhow!(
                        "Received unexpected GATT descriptor uuid: {:?}",
                        descr_uuid
                    ));
                }

                if status != GattStatus::Ok {
                    return Err(anyhow::anyhow!("Failed to register: {:?}", status));
                }

                self.attribute.set_handle(attr_handle)?;
            }
            Ok(_) => return Err(anyhow::anyhow!("Received unexpected GATT event")),
            Err(_) => return Err(anyhow::anyhow!("Timed out waiting for GATT event")),
        }

        Ok(())
    }

    fn uuid(&self) -> BtUuid {
        self.config.uuid.clone()
    }
}
