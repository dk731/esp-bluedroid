use std::{
    any,
    mem::discriminant,
    sync::{mpsc, Arc, RwLock, Weak},
};

use esp_idf_svc::bt::{
    ble::gatt::{
        server::AppId, GattCharacteristic, GattId, GattServiceId, GattStatus, Handle, ServiceUuid,
    },
    BtStatus, BtUuid,
};

use super::{app::AppInner, service::ServiceInner, GattsEvent, GattsEventMessage, GattsInner};

pub struct Characteristic<'d>(pub Arc<CharacteristicInner<'d>>);

#[derive(Debug)]
pub struct CharacteristicInner<'d> {
    pub service: Weak<ServiceInner<'d>>,
    // pub parameters: RwLock<GattCharacteristic>,
}

impl<'d> Characteristic<'d> {
    pub fn new(service: Arc<ServiceInner<'d>>) -> anyhow::Result<Self> {
        let service = Arc::downgrade(&service);
        let characterstic = CharacteristicInner {
            service,
            // parameters,
        };

        let characterstic = Self(Arc::new(characterstic));

        characterstic.register_bluedroid()?;
        characterstic.register_in_parent()?;

        Ok(characterstic)
    }

    fn register_bluedroid(&self) -> anyhow::Result<()> {
        // let (tx, rx) = mpsc::sync_channel(1);
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

        // gatts
        //     .gatts
        //     .add_characteristic(service_handle, characteristic, data)?;

        // gatts
        //     .gatts_events
        //     .write()
        //     .map_err(|_| anyhow::anyhow!("Failed to write Gatts events after registration"))?
        //     .insert(callback_key.clone(), tx.clone());

        // gatts
        //     .gatts
        //     .create_service(gatt_interface, &self.0.service_id, 10)?;

        // let callback_result = loop {
        //     match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        //         Ok(GattsEventMessage(
        //             _,
        //             GattsEvent::ServiceCreated {
        //                 status,
        //                 service_handle,
        //                 service_id,
        //             },
        //         )) => {
        //             if service_id == self.0.service_id {
        //                 if status != GattStatus::Ok {
        //                     break Err(anyhow::anyhow!(
        //                         "Failed to register GATT application: {:?}",
        //                         status
        //                     ));
        //                 }

        //                 match self.0.handle.write().map_err(|_| {
        //                     anyhow::anyhow!("Failed to write Gatt interface after registration")
        //                 }) {
        //                     Ok(mut handle) => {
        //                         if handle.is_some() {
        //                             break Err(anyhow::anyhow!(
        //                                 "Service handle already set, likely Service was not initialized properly"
        //                             ));
        //                         }
        //                         *handle = Some(service_handle);
        //                         break Ok(());
        //                     }
        //                     Err(_) => {
        //                         break Err(anyhow::anyhow!(
        //                             "Failed to write Gatt interface after registration"
        //                         ));
        //                     }
        //                 };
        //             }
        //         }
        //         Ok(_) => {
        //             break Err(anyhow::anyhow!(
        //                 "Received unexpected GATT application registration event"
        //             ));
        //         }
        //         Err(_) => {
        //             break Err(anyhow::anyhow!(
        //                 "Timed out waiting for GATT application registration event"
        //             ));
        //         }
        //     }
        // };

        // gatts
        //     .gatts_events
        //     .write()
        //     .map_err(|_| anyhow::anyhow!("Failed to write Gatts events after registration"))?
        //     .remove(&callback_key);

        // callback_result?;

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

    pub fn register_descriptor(&self) -> anyhow::Result<()> {
        // let gatts = self.gatts.upgrade().ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;
        // gatts.register_service(self.id)?;
        Ok(())
    }
}
