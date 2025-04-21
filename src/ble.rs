use std::sync::Arc;

use esp_idf_svc as svc;
use esp_idf_svc::hal::modem::Modem;

use svc::bt::BtDriver;
use svc::nvs::EspDefaultNvsPartition;

use crate::gap::Gap;
use crate::gatts::Gatts;

pub type ExtBtDriver = Arc<BtDriver<'static, svc::bt::Ble>>;

pub struct Ble {
    _bt: ExtBtDriver,
    pub gap: Gap,
    pub gatts: Gatts,
}

impl Ble {
    pub fn new(modem: Modem) -> anyhow::Result<Self> {
        let nvs = EspDefaultNvsPartition::take()?;
        let bt = Arc::new(BtDriver::<svc::bt::Ble>::new(modem, Some(nvs.clone()))?);

        let gatts = Gatts::new(bt.clone())?;
        let gap = Gap::new(bt.clone(), &gatts.0)?;

        let ble = Ble {
            _bt: bt,
            gap,
            gatts,
        };

        Ok(ble)
    }
}
