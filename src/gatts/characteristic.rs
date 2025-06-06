use std::{
    collections::HashMap,
    mem::discriminant,
    sync::{Arc, RwLock, Weak},
};

use crossbeam_channel::bounded;
use enumset::EnumSet;
use esp_idf_svc::bt::{
    BtUuid,
    ble::gatt::{AutoResponse, GattCharacteristic, GattStatus, Handle, Permission, Property},
};

use super::{
    GattsEvent,
    attribute::{
        AnyAttribute, Attribute, AttributeInner,
        defaults::{StringAttr, U16Attr},
    },
    descriptor::{Descriptor, DescriptorAttribute, DescriptorConfig, DescritporId},
    event::GattsEventMessage,
    service::{self, ServiceInner},
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

    pub description: Option<String>,
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

pub trait CharacteristicAttribute: Send + Sync + 'static {
    fn update_from_bytes(&self, bytes: &[u8]) -> anyhow::Result<()>;
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>>;
}

pub struct Characteristic<T: Attribute>(pub Arc<CharacteristicInner<T>>);
impl<T: Attribute> Clone for Characteristic<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pub struct CharacteristicInner<T: Attribute> {
    pub service: RwLock<Weak<ServiceInner>>,
    pub config: CharacteristicConfig,
    pub descriptors: HashMap<DescritporId, Arc<dyn DescriptorAttribute<T>>>,

    pub attribute: AttributeInner<T>,
}

impl<T: Attribute> Characteristic<T> {
    pub fn new(
        value: T,
        config: CharacteristicConfig,
        descriptors: Option<Vec<Arc<dyn DescriptorAttribute<T>>>>,
    ) -> Self {
        let characterstic = CharacteristicInner {
            service: RwLock::new(Weak::new()),
            config,
            attribute: AttributeInner::new(value),
            descriptors: match descriptors {
                Some(descriptors) => descriptors
                    .into_iter()
                    .map(|descriptor| {
                        let descriptor = descriptor.clone();

                        let id: DescritporId = DescritporId(descriptor.uuid());
                        (id, descriptor)
                    })
                    .collect(),
                None => HashMap::new(),
            },
        };

        let characterstic = Self(Arc::new(characterstic));

        characterstic
    }

    pub fn register_bluedroid(&self, service: &Arc<ServiceInner>) -> anyhow::Result<()> {
        *self
            .0
            .service
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Service"))? = Arc::downgrade(service);

        self.register_characteristic()?;
        self.register_in_global()?;

        let mut descriptors_to_register: HashMap<DescritporId, Arc<dyn DescriptorAttribute<T>>> =
            HashMap::new();

        // Client Characteristic Configuration Descriptor (CCCD)
        if self.0.config.enable_notify {
            let descriptor = Descriptor::<U16Attr, T>::new(
                U16Attr(0),
                DescriptorConfig {
                    uuid: BtUuid::uuid16(0x2902),
                    readable: true,
                    writable: true,
                },
            );

            descriptors_to_register.insert(DescritporId(descriptor.uuid()), Arc::new(descriptor));
        }

        // Server Characteristic Configuration Descriptor (SCCD)
        if self.0.config.broadcasted {
            let descriptor = Descriptor::<U16Attr, T>::new(
                U16Attr(0x0001),
                DescriptorConfig {
                    uuid: BtUuid::uuid16(0x2903),
                    readable: true,
                    writable: true,
                },
            );

            descriptors_to_register.insert(DescritporId(descriptor.uuid()), Arc::new(descriptor));
        }

        // Characteristic User Description Descriptor
        if let Some(description) = &self.0.config.description {
            let descriptor = Descriptor::<StringAttr, T>::new(
                StringAttr(description.clone()),
                DescriptorConfig {
                    uuid: BtUuid::uuid16(0x2901),
                    readable: true,
                    writable: false,
                },
            );

            descriptors_to_register.insert(DescritporId(descriptor.uuid()), Arc::new(descriptor));
        }

        self.0.descriptors.iter().for_each(|(_, descriptor)| {
            descriptors_to_register.insert(DescritporId(descriptor.uuid()), descriptor.clone());
        });

        for descriptor in descriptors_to_register.values() {
            descriptor.register(&self.0)?;
        }

        Ok(())
    }

    fn register_in_global(&self) -> anyhow::Result<()> {
        let service = self.0.get_service()?;
        let app = service.get_app()?;
        let gatts = app.get_gatts()?;
        let handle = self.0.handle()?;

        if gatts
            .attributes
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatt attributes"))?
            .insert(handle, self.0.clone())
            .is_some()
        {
            return Err(anyhow::anyhow!("Failed to write Gatt attributes"));
        }

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

        let service = self.0.get_service()?;
        let app = service.get_app()?;
        let gatts = app.get_gatts()?;
        let gatts_interface = app.interface()?;
        let service_handle = service.get_handle()?;

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
                interface,
                GattsEvent::CharacteristicAdded {
                    status,
                    attr_handle,
                    service_handle,
                    char_uuid,
                },
            )) => {
                if interface != gatts_interface {
                    return Err(anyhow::anyhow!(
                        "Received unexpected GATT interface: {:?}",
                        interface
                    ));
                }

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

                self.0.attribute.set_handle(attr_handle)?;

                Ok(())
            }
            Ok(_) => Err(anyhow::anyhow!("Received unexpected GATT")),
            Err(_) => Err(anyhow::anyhow!("Timed out waiting for GATT event")),
        }
    }

    pub fn value(&self) -> anyhow::Result<Arc<T>> {
        self.0.attribute.get_value()
    }

    pub fn update_value(&self, value: T) -> anyhow::Result<()> {
        AnyAttribute::update_from_bytes(&*self.0, &value.get_bytes()?)
    }
}

impl<T: Attribute> CharacteristicInner<T> {
    pub fn get_service(&self) -> anyhow::Result<Arc<ServiceInner>> {
        self.service
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read Service"))?
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Service"))
    }

    pub fn handle(&self) -> anyhow::Result<Handle> {
        self.attribute.handle()
    }
}

impl<T: Attribute> CharacteristicAttribute for CharacteristicInner<T> {
    fn update_from_bytes(&self, bytes: &[u8]) -> anyhow::Result<()> {
        self.attribute.update(Arc::new(T::from_bytes(bytes)?))
    }

    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        self.attribute.get_bytes()
    }
}

impl<T: Attribute> AnyAttribute for CharacteristicInner<T> {
    fn update_from_bytes(&self, bytes: &[u8]) -> anyhow::Result<()> {
        self.attribute.update(Arc::new(T::from_bytes(bytes)?))?;

        let (tx, rx) = bounded(1);
        let callback_key = discriminant(&GattsEvent::Confirm {
            status: GattStatus::Busy,
            conn_id: 0,
            handle: 0,
            value: None,
        });

        let service = self.get_service()?;
        let app = service.get_app()?;
        let gatts = app.get_gatts()?;
        let gatts_interface = app.interface()?;
        let characteristic_handle = self.attribute.handle()?;

        let connections = app
            .connections
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read connections in App: {:?}", app.id))?;
        let notify_data = self.attribute.get_bytes()?;

        gatts
            .gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events in App: {:?}", app.id))?
            .insert(callback_key, tx);

        let send_results = connections
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

                gatts
                    .gatts
                    .indicate(
                        gatts_interface,
                        connection.id,
                        characteristic_handle,
                        &notify_data[..data_end_index],
                    )
                    .map_err(|err| {
                        anyhow::anyhow!(
                            "Failed to send GATT indication to {:?}: {:?}",
                            connection.address,
                            err
                        )
                    })?;

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
            .collect::<Vec<anyhow::Result<()>>>();

        let errors: Vec<anyhow::Error> = send_results
            .into_iter()
            .filter_map(anyhow::Result::err)
            .collect();

        if !errors.is_empty() {
            return Err(anyhow::anyhow!(
                "Failed to notify some of connections: {:?}",
                errors
            ));
        }

        Ok(())
    }

    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        self.attribute.get_bytes()
    }
}
