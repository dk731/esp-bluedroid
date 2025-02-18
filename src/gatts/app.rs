use std::{
    any,
    sync::{mpsc, Arc, RwLock, Weak},
};

use esp_idf_svc::bt::{
    ble::gatt::server::{AppId, GattsEvent},
    BtUuid,
};

use super::GattsInner;

pub struct App<'d>(pub Arc<AppInner<'d>>);

#[derive(Debug, Clone)]
pub struct AppInner<'d> {
    gatts: Weak<GattsInner<'d>>,

    pub id: AppId,
}

impl<'d> App<'d> {
    pub fn new(gatts: Arc<GattsInner<'d>>, app_id: AppId) -> anyhow::Result<Self> {
        let gatts = Arc::downgrade(&gatts);
        let app = AppInner { gatts, id: app_id };

        let app = Self(Arc::new(app));

        app.register_bluedroid()?;
        app.register_in_parent()?;

        Ok(app)
    }

    fn register_bluedroid(&self) -> anyhow::Result<()> {
        let (tx, rx) = mpsc::sync_channel::<GattsEvent>(0);
        let gatts = self
            .0
            .gatts
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;

        // gatts
        //     .gatts_events
        //     .write()
        //     .map_err(|_| anyhow::anyhow!("Failed to write Gatts events"))?
        //     .insert(discriminant(GattsEvent::Foo), tx.clone());

        gatts.gatts.register_app(self.0.id)?;

        // Register the BLE app with the GATT server
        // gatts.register_app(app.id)?;

        Ok(())
    }

    fn register_in_parent(&self) -> anyhow::Result<()> {
        let gatts = self
            .0
            .gatts
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;

        if gatts
            .apps
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts"))?
            .insert(self.0.id, self.0.clone())
            .is_some()
        {
            log::warn!("App with ID {:?} already exists, replacing it", self.0.id);
        }

        Ok(())
    }

    pub fn register_service(&self, service_uuid: BtUuid) -> anyhow::Result<()> {
        // let gatts = self.gatts.upgrade().ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;
        // gatts.register_service(self.id)?;
        Ok(())
    }
}
