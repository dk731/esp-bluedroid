use std::sync::mpsc;

use dashmap::DashMap;
use esp_idf_svc::bt::ble::gap::{BleGapEvent, EspBleGap};

use crate::ble::ExtBtDriver;
use esp_idf_svc as svc;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum GapEvent {
    AdvertisingConfigured,
    ScanResponseConfigured,
    ScanParameterConfigured,
    ScanResult,
    RawAdvertisingConfigured,
    RawScanResponseConfigured,
    AdvertisingStarted,
    ScanStarted,
    AuthenticationComplete,
    Key,
    SecurityRequest,
    PasskeyNotification,
    PasskeyRequest,
    OOBRequest,
    LocalIR,
    LocalER,
    NumericComparisonRequest,
    AdvertisingStopped,
    ScanStopped,
    StaticRandomAddressConfigured,
    ConnectionParamsConfigured,
    PacketLengthConfigured,
    LocalPrivacyConfigured,
    DeviceBondRemoved,
    DeviceBondCleared,
    DeviceBond,
    ReadRssiConfigured,
    WhitelistUpdated,
    DuplicateListUpdated,
    ChannelsConfigured,
    ReadFeaturesConfigured,
    PreferredDefaultPhyConfigured,
    PreferredPhyConfigured,
    ExtendedAdvertisingRandomAddressConfigured,
    ExtendedAdvertisingParametersConfigured,
    ExtendedAdvertisingConfigured,
    ExtendedAdvertisingScanResponseConfigured,
    ExtendedAdvertisingStarted,
    ExtendedAdvertisingStopped,
    ExtendedAdvertisingRemoved,
    ExtendedAdvertisingCleared,
    PeriodicAdvertisingParametersConfigured,
    PeriodicAdvertisingDataSetComplete,
    PeriodicAdvertisingStarted,
    PeriodicAdvertisingStopped,
    PeriodicAdvertisingSyncCreated,
    PeriodicAdvertisingSyncCanceled,
    PeriodicAdvertisingSyncTerminated,
    PeriodicAdvertisingDeviceListAdded,
    PeriodicAdvertisingDeviceListRemoved,
    PeriodicAdvertisingDeviceListCleared,
    ExtendedAdvertisingScanParametersConfigured,
    ExtendedAdvertisingScanStarted,
    ExtendedAdvertisingScanStopped,
    ExtendedAdvertisingExtendedConnectionParamsConfigured,
    Other,
}

pub struct Gap<'d> {
    gap: EspBleGap<'d, svc::bt::Ble, ExtBtDriver<'d>>,
    gap_events: DashMap<GapEvent, mpsc::Sender<BleGapEvent<'static>>>,
}

impl<'d> Gap<'d> {
    pub fn new(bt: ExtBtDriver<'d>) -> anyhow::Result<Self> {
        let gap = EspBleGap::new(bt)?;
        let gap_events = DashMap::new();

        Ok(Self { gap, gap_events })
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
