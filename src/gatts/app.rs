use std::sync::{Arc, Weak};

use esp_idf_svc::bt::ble::gatt::server::AppId;

use super::{Gatts, GattsInner};

#[derive(Debug, Clone)]
pub struct App<'d> {
    gatts: Weak<Gatts<'d>>,
    pub id: AppId,
}

impl<'d> App<'d> {
    // pub fn new(app_id: AppId) -> anyhow::Result<Self> {
    //     Ok(App { id: app_id })
    // }

    pub fn register_service() -> anyhow::Result<()> {
        // let gatts = self.gatts.upgrade().ok_or(anyhow::anyhow!("Failed to upgrade Gatts"))?;
        // gatts.register_service(self.id)?;
        Ok(())
    }
}
