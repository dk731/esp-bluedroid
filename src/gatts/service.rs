use std::{
    any,
    mem::discriminant,
    sync::{mpsc, Arc, RwLock, Weak},
};

use esp_idf_svc::bt::{
    ble::gatt::{server::AppId, GattServiceId, GattStatus, ServiceUuid},
    BtUuid,
};

use super::{app::AppInner, GattsEvent, GattsEventMessage, GattsInner};

pub struct Service<'d>(pub Arc<ServiceInner<'d>>);
// pub struct ServiceIdKey(pub )

#[derive(Debug)]
pub struct ServiceInner<'d> {
    app: Weak<AppInner<'d>>,
    service_id: GattServiceId,
}

impl<'d> Service<'d> {
    pub fn new(app: Arc<AppInner<'d>>, service_id: GattServiceId) -> anyhow::Result<Self> {
        let app = Arc::downgrade(&app);
        let service = ServiceInner { app, service_id };

        let service = Self(Arc::new(service));

        service.register_bluedroid()?;
        service.register_in_parent()?;

        Ok(service)
    }

    fn register_bluedroid(&self) -> anyhow::Result<()> {
        // let (tx, rx) = mpsc::sync_channel(1);
        // let callback_key = discriminant(&GattsEvent::ServiceRegistered {
        //     status: GattStatus::Busy,
        //     app_id: 0,
        // });

        // let gatts = self
        //     .0
        //     .app
        //     .upgrade()
        //     .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;

        // gatts
        //     .gatts_events
        //     .write()
        //     .map_err(|_| anyhow::anyhow!("Failed to write Gatts events"))?
        //     .insert(callback_key.clone(), tx.clone());

        // gatts.gatts.register_app(self.0.id)?;

        // let callback_result = loop {
        //     match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        //         Ok(GattsEventMessage(
        //             interface,
        //             GattsEvent::ServiceRegistered { status, app_id },
        //         )) => {
        //             if app_id == self.0.id {
        //                 if status != GattStatus::Ok {
        //                     break Err(anyhow::anyhow!(
        //                         "Failed to register GATT application: {:?}",
        //                         status
        //                     ));
        //                 }

        //                 *self.0.gatt_interface.write().map_err(|_| {
        //                     anyhow::anyhow!("Failed to write Gatt interface after registration")
        //                 })? = Some(interface);
        //                 break Ok(());
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

        // gatts.gatts.create_service(gatt_interface, service_id, num_handles)
        // gatts.gatts.add_characteristic(service_handle, characteristic, data)
        // gatts.gatts.start_service(service_handle)

        // gatts.create_service(gatt_if, service_id, num_handles);

        // gatts
        //     .gatts_events
        //     .write()
        //     .map_err(|_| anyhow::anyhow!("Failed to write Gatts events after registration"))?
        //     .remove(&callback_key);

        // callback_result?;

        Ok(())
    }

    fn register_in_parent(&self) -> anyhow::Result<()> {
        let app = self
            .0
            .app
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;

        let mut qwe = app
            .services
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts"))?;

        // if app
        //     .services
        //     .write()
        //     .map_err(|_| anyhow::anyhow!("Failed to write Gatts"))?.
        //     // .insert(self.0.service_id.clone(), self.0.clone())
        //     // .is_some()
        // {
        //     // log::warn!("App with ID {:?} already exists, replacing it", self.0.id);
        // }

        Ok(())
    }

    pub fn register_characteristic(&self, service_uuid: BtUuid) -> anyhow::Result<()> {
        // let gatts = self.gatts.upgrade().ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;
        // gatts.register_service(self.id)?;
        Ok(())
    }
}
