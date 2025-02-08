use std::any::Any;
use std::mem::{discriminant, Discriminant};
use std::rc::Rc;
use std::sync::Arc;

use dashmap::DashMap;
use esp_idf_svc as svc;
use esp_idf_svc::bt::ble::gap::{self, BleGapEvent};
use esp_idf_svc::bt::ble::gatt::server::GattsEvent;
use esp_idf_svc::bt::ble::gatt::GattInterface;
use esp_idf_svc::hal::modem::Modem;
use log::info;

use svc::bt::ble::gap::EspBleGap;
use svc::bt::ble::gatt::server::EspGatts;
use svc::bt::BtDriver;
use svc::hal::prelude::Peripherals;
use svc::nvs::EspDefaultNvsPartition;

use crate::gap::Gap;

pub type ExtBtDriver<'d> = Arc<BtDriver<'d, svc::bt::Ble>>;

pub struct Ble<'d> {
    _bt: ExtBtDriver<'d>,
    pub gap: Gap<'d>,

    // gap_events: Rc<gap::BleGapEvent<'d, ExtBtDriver<'d>>>,
    gatts: EspGatts<'d, svc::bt::Ble, ExtBtDriver<'d>>,
}

impl<'d> Ble<'_> {
    pub fn new(modem: Modem) -> anyhow::Result<Self> {
        let nvs = EspDefaultNvsPartition::take()?;
        let bt = Arc::new(BtDriver::<svc::bt::Ble>::new(modem, Some(nvs.clone()))?);

        let gap = Gap::new(bt.clone())?;
        let gatts = EspGatts::new(bt.clone())?;

        let ble = Ble {
            _bt: bt,
            gap,
            gatts,
        };

        ble.init_event_handlers()?;

        Ok(ble)
    }

    fn init_event_handlers(&self) -> anyhow::Result<()> {
        // Unsafe because we are using the `subscribe_nonstatic` method allowes to define a non-static callback.
        // This is safe because we have implemented proper unsubscribe logic in the `Drop` trait. for the `Ble` struct.
        // unsafe {
        //     self.gatts
        //         .subscribe_nonstatic(|e| self.gatts_events_callback(e.0, e.1))?;
        //     log::debug!("Subscribed to GATTS events");
        // }

        Ok(())
    }

    // fn gatts_events_callback(&self, inteface: GattInterface, event: GattsEvent<'_>) {
    //     info!("Received GATT event: {:?}", event);
    // }
}
