use std::{
    mem::discriminant,
    sync::{Arc, RwLock, Weak},
};

use crossbeam_channel::bounded;
use enumset::EnumSet;
use esp_idf_svc::bt::{
    ble::gatt::{GattDescriptor, GattStatus, Handle, Permission},
    BtUuid,
};

use super::{
    attribute::{AnyAttribute, Attribute, AttributeInner},
    characteristic::{self, CharacteristicInner},
    event::{GattsEvent, GattsEventMessage},
    service,
};

pub struct DescriptorConfig {
    pub uuid: BtUuid,

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

pub trait DescriptorAttribute<T: Attribute>: Send + Sync + 'static {
    fn update_from_bytes(&self, bytes: &[u8]) -> anyhow::Result<()>;
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>>;
    fn register(&self, service: &Arc<CharacteristicInner<T>>) -> anyhow::Result<()>;
    fn uuid(&self) -> BtUuid;
    fn handle(&self) -> anyhow::Result<Handle>;
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
}

impl<T: Attribute, A: Attribute> DescriptorInner<T, A> {
    fn get_characteristic(&self) -> anyhow::Result<Arc<CharacteristicInner<A>>> {
        self.characteristic
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read characteristic"))?
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade characteristic"))
    }

    fn handle(&self) -> anyhow::Result<Handle> {
        self.attribute
            .handle
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read attribute"))?
            .ok_or_else(|| anyhow::anyhow!("Attribute handle not set"))
    }
}

impl<T: Attribute, A: Attribute> AnyAttribute for DescriptorInner<T, A> {
    fn update_from_bytes(&self, bytes: &[u8]) -> anyhow::Result<()> {
        self.attribute.update(Arc::new(T::from_bytes(bytes)?))
    }

    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        self.attribute.get_bytes()
    }
}

impl<T: Attribute, A: Attribute> DescriptorAttribute<A> for Descriptor<T, A> {
    fn update_from_bytes(&self, bytes: &[u8]) -> anyhow::Result<()> {
        self.0.attribute.update(Arc::new(T::from_bytes(bytes)?))
    }

    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        self.0.attribute.get_bytes()
    }

    fn handle(&self) -> anyhow::Result<Handle> {
        self.0
            .attribute
            .handle
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read attribute"))?
            .ok_or_else(|| anyhow::anyhow!("Attribute handle not set"))
    }

    fn register(&self, characteristic: &Arc<CharacteristicInner<A>>) -> anyhow::Result<()> {
        *self
            .0
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
            .add_descriptor(parent_service_handle, &(&self.0.config).into())
            .map_err(|err| {
                anyhow::anyhow!(
                    "Failed to register GATT descriptor {:?}: {:?}",
                    self.0.config.uuid,
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

                if self.0.config.uuid != descr_uuid {
                    return Err(anyhow::anyhow!(
                        "Received unexpected GATT descriptor uuid: {:?}",
                        descr_uuid
                    ));
                }

                if status != GattStatus::Ok {
                    return Err(anyhow::anyhow!("Failed to register: {:?}", status));
                }

                self.0.attribute.set_handle(attr_handle)?;
            }
            Ok(_) => return Err(anyhow::anyhow!("Received unexpected GATT event")),
            Err(_) => return Err(anyhow::anyhow!("Timed out waiting for GATT event")),
        }

        let characteristic = self.0.get_characteristic()?;
        let service = characteristic.get_service()?;
        let app = service.get_app()?;
        let gatts = app.get_gatts()?;

        if gatts
            .attributes
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write GATT attributes"))?
            .insert(self.handle()?, self.0.clone())
            .is_some()
        {
            return Err(anyhow::anyhow!(
                "Failed to register GATT descriptor {:?}: already exists",
                self.0.config.uuid
            ));
        }

        Ok(())
    }

    fn uuid(&self) -> BtUuid {
        self.0.config.uuid.clone()
    }
}
