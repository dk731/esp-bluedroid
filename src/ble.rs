use std::sync::Arc;

use esp_idf_svc as svc;
use esp_idf_svc::hal::modem::Modem;

use svc::bt::BtDriver;
use svc::nvs::EspDefaultNvsPartition;

use crate::gap::Gap;
use crate::gatts::Gatts;

pub type ExtBtDriver<'d> = Arc<BtDriver<'d, svc::bt::Ble>>;

pub struct Ble<'d> {
    _bt: ExtBtDriver<'d>,
    pub gap: Gap<'d>,
    pub gatts: Gatts<'d>,
}

impl<'d> Ble<'_> {
    pub fn new(modem: Modem) -> anyhow::Result<Self> {
        let nvs = EspDefaultNvsPartition::take()?;
        let bt = Arc::new(BtDriver::<svc::bt::Ble>::new(modem, Some(nvs.clone()))?);

        let gap = Gap::new(bt.clone())?;
        let gatts = Gatts::new(bt.clone())?;

        let ble = Ble {
            _bt: bt,
            gap,
            gatts,
        };

        Ok(ble)
    }
}
