use std::{
    collections::HashMap,
    mem::{discriminant, Discriminant},
    sync::{mpsc, Arc},
};

use dashmap::DashMap;
use esp_idf_svc::bt::{
    ble::gap::{BleGapEvent, EspBleGap},
    BtStatus,
};

use crate::ble::ExtBtDriver;
use esp_idf_svc as svc;

pub struct SendableGapEvent<'d>(pub BleGapEvent<'d>);
unsafe impl<'d> Send for SendableGapEvent<'d> {}

pub struct Gap<'d> {
    gap: EspBleGap<'d, svc::bt::Ble, ExtBtDriver<'d>>,
    gap_events: DashMap<Discriminant<BleGapEvent<'d>>, mpsc::Sender<SendableGapEvent<'d>>>,
}

impl<'d> Gap<'d> {
    pub fn new(bt: ExtBtDriver<'d>) -> anyhow::Result<Self> {
        let gap = EspBleGap::new(bt)?;

        let gap = Self {
            gap,
            gap_events: DashMap::default(),
        };

        let mut handlers = HashMap::new();
        let ewq = discriminant(&BleGapEvent::AdvertisingConfigured);
        // Add a handler
        handlers.insert(
            discriminant(&BleGapEvent::AdvertisingConfigured(BtStatus::Success)),
            || { /* handler code */ },
        );

        gap.init()?;
        log::debug!("GAP initialized");

        Ok(gap)
    }

    pub fn init(&self) -> anyhow::Result<()> {
        unsafe {
            self.gap.subscribe_nonstatic(|e| self.events_callback(e))?;
            log::debug!("Subscribed to GAP events");
        }

        Ok(())
    }

    fn events_callback(&self, event: BleGapEvent) {
        log::debug!("GAP event: {:?}", event);

        match event {
            BleGapEvent::AdvertisingConfigured(status) => {
                log::debug!("Advertising configured: {:?}", status);
            }
            BleGapEvent::AdvertisingStarted(status) => {
                log::debug!("Advertising started: {:?}", status);

                self.gap_events.get(&discriminant(&event)).map(|sender| {
                    let qwe = SendableGapEvent(Arc::new(event));
                    if let Err(err) = sender.send(qwe) {
                        log::error!("Failed to send GAP event: {:?}", err);
                    }
                });
            }
            BleGapEvent::AdvertisingStopped(status) => {
                log::debug!("Advertising stopped: {:?}", status);
            }
            _ => {
                log::debug!("Other GAP event: {:?}", event);
            }
        }
    }

    // self.gap
    //             .subscribe_nonstatic(|e| self.gap_events_callback(e))?;
    //         log::debug!("Subscribed to GAP events");
}

impl Drop for Gap<'_> {
    fn drop(&mut self) {
        if let Err(err) = self.gap.unsubscribe() {
            log::error!("Failed to unsubscribe from GAP events: {:?}", err);
        }
    }
}
