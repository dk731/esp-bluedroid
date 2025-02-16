use std::{
    any,
    sync::{Arc, RwLock, Weak},
};

use esp_idf_svc::bt::ble::gatt::server::AppId;

use super::GattsInner;

pub struct App<'d> {
    inner: Arc<RwLock<AppInner<'d>>>,
}

#[derive(Debug, Clone)]
pub struct AppInner<'d> {
    gatts: Weak<RwLock<GattsInner<'d>>>,

    pub id: AppId,
}

impl<'d> App<'d> {
    pub fn new(gatts: Arc<RwLock<GattsInner<'d>>>, app_id: AppId) -> anyhow::Result<Self> {
        let gatts = Arc::downgrade(&gatts);
        let app = AppInner { gatts, id: app_id };

        let app = App {
            inner: Arc::new(RwLock::new(app)),
        };

        app.register_in_parent()?;

        Ok(app)
    }

    fn register_in_parent(&self) -> anyhow::Result<()> {
        let app = self
            .inner
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read App"))?;
        let gatts = app
            .gatts
            .upgrade()
            .ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;

        if gatts
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts"))?
            .apps
            .insert(app.id, self.inner.clone())
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

    pub fn id(&self) -> anyhow::Result<AppId> {
        Ok(self
            .inner
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read App"))?
            .id)
    }
}
