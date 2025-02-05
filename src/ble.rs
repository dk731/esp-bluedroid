use std::rc::Rc;
use std::sync::Arc;

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

type ExtBtDriver<'d> = Arc<BtDriver<'d, svc::bt::Ble>>;

pub struct Ble<'d> {
    _bt: ExtBtDriver<'d>,

    gap: EspBleGap<'d, svc::bt::Ble, ExtBtDriver<'d>>,
    // gap_events: Rc<gap::BleGapEvent<'d, ExtBtDriver<'d>>>,
    gatts: EspGatts<'d, svc::bt::Ble, ExtBtDriver<'d>>,
}

impl<'d> Ble<'_> {
    pub fn new(modem: Modem) -> anyhow::Result<Self> {
        let nvs = EspDefaultNvsPartition::take()?;
        let bt = Arc::new(BtDriver::<svc::bt::Ble>::new(modem, Some(nvs.clone()))?);

        let gap = EspBleGap::new(bt.clone())?;
        let gatts = EspGatts::new(bt.clone())?;

        Ok(Ble {
            _bt: bt,
            gap,
            gatts,
        })
    }

    pub fn init(&self) -> anyhow::Result<()> {
        // Unsafe because we are using the `subscribe_nonstatic` method allowes to define a non-static callback.
        // This is safe because we have implemented proper unsubscribe logic in the `Drop` trait. for the `Ble` struct.
        unsafe {
            self.gap
                .subscribe_nonstatic(|e| self.gap_events_callback(e))?;

            self.gatts
                .subscribe_nonstatic(|e| self.gatts_events_callback(e.0, e.1))?;
        }

        self.gap.start_advertising()?;

        Ok(())
    }

    pub fn start_advertising(&self) -> anyhow::Result<()> {
        self.gap.start_advertising()?;
        Ok(())
    }

    pub fn stop_advertising(&self) -> anyhow::Result<()> {
        self.gap.stop_advertising()?;
        Ok(())
    }

    fn gap_events_callback(&self, event: BleGapEvent<'_>) {
        log::info!("Received GAP event: {:?}", event);
        // match event {
        //     BleGapEvent::AdvertisingConfigured(bt_status) => todo!(),
        //     BleGapEvent::ScanResponseConfigured(bt_status) => todo!(),
        //     BleGapEvent::ScanParameterConfigured(bt_status) => todo!(),
        //     BleGapEvent::ScanResult(esp_ble_gap_cb_param_t_ble_scan_result_evt_param) => todo!(),
        //     BleGapEvent::RawAdvertisingConfigured(bt_status) => todo!(),
        //     BleGapEvent::RawScanResponseConfigured(bt_status) => todo!(),
        //     BleGapEvent::AdvertisingStarted(bt_status) => todo!(),
        //     BleGapEvent::ScanStarted(bt_status) => todo!(),
        //     BleGapEvent::AuthenticationComplete { bd_addr, status } => todo!(),
        //     BleGapEvent::Key => todo!(),
        //     BleGapEvent::SecurityRequest => todo!(),
        //     BleGapEvent::PasskeyNotification { addr, passkey } => todo!(),
        //     BleGapEvent::PasskeyRequest => todo!(),
        //     BleGapEvent::OOBRequest { oob_c, oob_r } => todo!(),
        //     BleGapEvent::LocalIR => todo!(),
        //     BleGapEvent::LocalER => todo!(),
        //     BleGapEvent::NumericComparisonRequest => todo!(),
        //     BleGapEvent::AdvertisingStopped(bt_status) => todo!(),
        //     BleGapEvent::ScanStopped(bt_status) => todo!(),
        //     BleGapEvent::StaticRandomAddressConfigured(bt_status) => todo!(),
        //     BleGapEvent::ConnectionParamsConfigured {
        //         addr,
        //         status,
        //         min_int_ms,
        //         max_int_ms,
        //         latency_ms,
        //         conn_int,
        //         timeout_ms,
        //     } => todo!(),
        //     BleGapEvent::PacketLengthConfigured {
        //         status,
        //         rx_len,
        //         tx_len,
        //     } => todo!(),
        //     BleGapEvent::LocalPrivacyConfigured(bt_status) => todo!(),
        //     BleGapEvent::DeviceBondRemoved { bd_addr, status } => todo!(),
        //     BleGapEvent::DeviceBondCleared(bt_status) => todo!(),
        //     BleGapEvent::DeviceBond(esp_ble_gap_cb_param_t_ble_get_bond_dev_cmpl_evt_param) => {
        //         todo!()
        //     }
        //     BleGapEvent::ReadRssiConfigured {
        //         bd_addr,
        //         rssdi,
        //         status,
        //     } => todo!(),
        //     BleGapEvent::WhitelistUpdated {
        //         status,
        //         wl_operation,
        //     } => todo!(),
        //     BleGapEvent::DuplicateListUpdated(
        //         esp_ble_gap_cb_param_t_ble_update_duplicate_exceptional_list_cmpl_evt_param,
        //     ) => todo!(),
        //     BleGapEvent::ChannelsConfigured(bt_status) => todo!(),
        //     BleGapEvent::ReadFeaturesConfigured(
        //         esp_ble_gap_cb_param_t_ble_read_phy_cmpl_evt_param,
        //     ) => todo!(),
        //     BleGapEvent::PreferredDefaultPhyConfigured(bt_status) => todo!(),
        //     BleGapEvent::PreferredPhyConfigured(bt_status) => todo!(),
        //     BleGapEvent::ExtendedAdvertisingRandomAddressConfigured(bt_status) => todo!(),
        //     BleGapEvent::ExtendedAdvertisingParametersConfigured(bt_status) => todo!(),
        //     BleGapEvent::ExtendedAdvertisingConfigured(bt_status) => todo!(),
        //     BleGapEvent::ExtendedAdvertisingScanResponseConfigured(bt_status) => todo!(),
        //     BleGapEvent::ExtendedAdvertisingStarted(bt_status) => todo!(),
        //     BleGapEvent::ExtendedAdvertisingStopped(bt_status) => todo!(),
        //     BleGapEvent::ExtendedAdvertisingRemoved(bt_status) => todo!(),
        //     BleGapEvent::ExtendedAdvertisingCleared(bt_status) => todo!(),
        //     BleGapEvent::PeriodicAdvertisingParametersConfigured(bt_status) => todo!(),
        //     BleGapEvent::PeriodicAdvertisingDataSetComplete(bt_status) => todo!(),
        //     BleGapEvent::PeriodicAdvertisingStarted(bt_status) => todo!(),
        //     BleGapEvent::PeriodicAdvertisingStopped(bt_status) => todo!(),
        //     BleGapEvent::PeriodicAdvertisingSyncCreated(bt_status) => todo!(),
        //     BleGapEvent::PeriodicAdvertisingSyncCanceled(bt_status) => todo!(),
        //     BleGapEvent::PeriodicAdvertisingSyncTerminated(bt_status) => todo!(),
        //     BleGapEvent::PeriodicAdvertisingDeviceListAdded(bt_status) => todo!(),
        //     BleGapEvent::PeriodicAdvertisingDeviceListRemoved(bt_status) => todo!(),
        //     BleGapEvent::PeriodicAdvertisingDeviceListCleared(bt_status) => todo!(),
        //     BleGapEvent::ExtendedAdvertisingScanParametersConfigured(bt_status) => todo!(),
        //     BleGapEvent::ExtendedAdvertisingScanStarted(bt_status) => todo!(),
        //     BleGapEvent::ExtendedAdvertisingScanStopped(bt_status) => todo!(),
        //     BleGapEvent::ExtendedAdvertisingExtendedConnectionParamsConfigured(bt_status) => {
        //         todo!()
        //     }
        //     BleGapEvent::Other {
        //         raw_event,
        //         raw_data,
        //     } => todo!(),
        // }
    }

    fn gatts_events_callback(&self, inteface: GattInterface, event: GattsEvent<'_>) {
        info!("Received GATT event: {:?}", event);
    }
}

impl Drop for Ble<'_> {
    fn drop(&mut self) {
        if let Err(err) = self.gap.unsubscribe() {
            log::error!("Failed to unsubscribe from gap events: {:?}", err);
        }

        if let Err(err) = self.gatts.unsubscribe() {
            log::error!("Failed to unsubscribe from gatts events: {:?}", err);
        }
    }
}
