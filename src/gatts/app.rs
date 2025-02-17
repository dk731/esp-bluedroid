use std::{
    any,
    sync::{Arc, RwLock, Weak},
};

use esp_idf_svc::bt::ble::gatt::server::AppId;

use super::GattsInner;

pub struct App<'d> {
    pub inner: Arc<AppInner<'d>>,
}

#[derive(Debug, Clone)]
pub struct AppInner<'d> {
    gatts: Weak<GattsInner<'d>>,

    pub id: AppId,
}

impl<'d> App<'d> {
    pub fn new(gatts: Arc<GattsInner<'d>>, app_id: AppId) -> anyhow::Result<Self> {
        let gatts = Arc::downgrade(&gatts);
        let app = AppInner { gatts, id: app_id };

        let app = App {
            inner: Arc::new(app),
        };

        app.register_in_parent()?;

        Ok(app)
    }

    fn register_ble(&self) -> anyhow::Result<()> {
        // self.inner.read()

        // Register the BLE app with the GATT server
        // gatts.register_app(app.id)?;

        Ok(())
    }

    fn register_in_parent(&self) -> anyhow::Result<()> {
        let gatts = self
            .inner
            .gatts
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;

        if gatts
            .apps
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts"))?
            .insert(self.inner.id, self.inner.clone())
            .is_some()
        {
            log::warn!("App with ID {:?} already exists, replacing it", app.id);
        }

        Ok(())
    }

    pub fn register_service() -> anyhow::Result<()> {
        // let gatts = self.gatts.upgrade().ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;
        // gatts.register_service(self.id)?;
        Ok(())
    }
}
