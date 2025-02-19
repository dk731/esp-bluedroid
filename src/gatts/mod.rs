pub mod app;
pub mod characteristic;
pub mod descriptor;
pub mod service;

use std::{
    collections::HashMap,
    mem::{discriminant, Discriminant},
    sync::{mpsc, Arc, RwLock},
};

use app::{App, AppInner};
use esp_idf_svc::bt::{
    ble::gatt::{
        self,
        server::{AppId, ConnectionId, EspGatts, TransferId},
        GattConnParams, GattConnReason, GattServiceId, GattStatus, Handle,
    },
    BdAddr, BtUuid,
};

use crate::ble::ExtBtDriver;
use esp_idf_svc as svc;

#[derive(Debug, Clone)]
enum GattsEvent {
    ServiceRegistered {
        status: GattStatus,
        app_id: AppId,
    },
    Read {
        conn_id: ConnectionId,
        trans_id: TransferId,
        addr: BdAddr,
        handle: Handle,
        offset: u16,
        is_long: bool,
        need_rsp: bool,
    },
    Write {
        conn_id: ConnectionId,
        trans_id: TransferId,
        addr: BdAddr,
        handle: Handle,
        offset: u16,
        need_rsp: bool,
        is_prep: bool,
        value: Vec<u8>,
    },
    ExecWrite {
        conn_id: ConnectionId,
        trans_id: TransferId,
        addr: BdAddr,
        canceled: bool,
    },
    Mtu {
        conn_id: ConnectionId,
        mtu: u16,
    },
    Confirm {
        status: GattStatus,
        conn_id: ConnectionId,
        handle: Handle,
        value: Option<Vec<u8>>,
    },
    ServiceUnregistered {
        status: GattStatus,
        service_handle: Handle,
        service_id: GattServiceId,
    },
    ServiceCreated {
        status: GattStatus,
        service_handle: Handle,
        service_id: GattServiceId,
    },
    IncludedServiceAdded {
        status: GattStatus,
        attr_handle: Handle,
        service_handle: Handle,
    },
    CharacteristicAdded {
        status: GattStatus,
        attr_handle: Handle,
        service_handle: Handle,
        char_uuid: BtUuid,
    },
    DescriptorAdded {
        status: GattStatus,
        attr_handle: Handle,
        service_handle: Handle,
        descr_uuid: BtUuid,
    },
    ServiceDeleted {
        status: GattStatus,
        service_handle: Handle,
    },
    ServiceStarted {
        status: GattStatus,
        service_handle: Handle,
    },
    ServiceStopped {
        status: GattStatus,
        service_handle: Handle,
    },
    PeerConnected {
        conn_id: ConnectionId,
        link_role: u8,
        addr: BdAddr,
        conn_params: GattConnParams,
    },
    PeerDisconnected {
        conn_id: ConnectionId,
        addr: BdAddr,
        reason: GattConnReason,
    },
    Open {
        status: GattStatus,
    },
    Close {
        status: GattStatus,
        conn_id: ConnectionId,
    },
    Listen {
        conn_id: ConnectionId,
        congested: bool,
    },
    Congest {
        conn_id: ConnectionId,
        congested: bool,
    },
    ResponseComplete {
        status: GattStatus,
        handle: Handle,
    },
    AttributeTableCreated {
        status: GattStatus,
        svc_uuid: BtUuid,
        svc_inst_id: u8,
        handles: Vec<Handle>,
    },
    AttributeValueModified {
        srvc_handle: Handle,
        attr_handle: Handle,
        status: GattStatus,
    },
    ServiceChanged {
        status: GattStatus,
    },

    Other,
}

impl<'d> From<gatt::server::GattsEvent<'d>> for GattsEvent {
    fn from(event: gatt::server::GattsEvent<'d>) -> Self {
        match event {
            gatt::server::GattsEvent::ServiceRegistered { status, app_id } => {
                GattsEvent::ServiceRegistered { status, app_id }
            }
            gatt::server::GattsEvent::Read {
                conn_id,
                trans_id,
                addr,
                handle,
                offset,
                is_long,
                need_rsp,
            } => GattsEvent::Read {
                conn_id,
                trans_id,
                addr,
                handle,
                offset,
                is_long,
                need_rsp,
            },
            gatt::server::GattsEvent::Write {
                conn_id,
                trans_id,
                addr,
                handle,
                offset,
                need_rsp,
                is_prep,
                value,
            } => GattsEvent::Write {
                conn_id,
                trans_id,
                addr,
                handle,
                offset,
                need_rsp,
                is_prep,
                value: value.to_vec(),
            },
            gatt::server::GattsEvent::ExecWrite {
                conn_id,
                trans_id,
                addr,
                canceled,
            } => GattsEvent::ExecWrite {
                conn_id,
                trans_id,
                addr,
                canceled,
            },
            gatt::server::GattsEvent::Mtu { conn_id, mtu } => GattsEvent::Mtu { conn_id, mtu },
            gatt::server::GattsEvent::Confirm {
                status,
                conn_id,
                handle,
                value,
            } => GattsEvent::Confirm {
                status,
                conn_id,
                handle,
                value: value.map(|v| v.to_vec()),
            },
            gatt::server::GattsEvent::ServiceUnregistered {
                status,
                service_handle,
                service_id,
            } => GattsEvent::ServiceUnregistered {
                status,
                service_handle,
                service_id,
            },
            gatt::server::GattsEvent::ServiceCreated {
                status,
                service_handle,
                service_id,
            } => GattsEvent::ServiceCreated {
                status,
                service_handle,
                service_id,
            },
            gatt::server::GattsEvent::IncludedServiceAdded {
                status,
                attr_handle,
                service_handle,
            } => GattsEvent::IncludedServiceAdded {
                status,
                attr_handle,
                service_handle,
            },
            gatt::server::GattsEvent::CharacteristicAdded {
                status,
                attr_handle,
                service_handle,
                char_uuid,
            } => GattsEvent::CharacteristicAdded {
                status,
                attr_handle,
                service_handle,
                char_uuid,
            },
            gatt::server::GattsEvent::DescriptorAdded {
                status,
                attr_handle,
                service_handle,
                descr_uuid,
            } => GattsEvent::DescriptorAdded {
                status,
                attr_handle,
                service_handle,
                descr_uuid,
            },
            gatt::server::GattsEvent::ServiceDeleted {
                status,
                service_handle,
            } => GattsEvent::ServiceDeleted {
                status,
                service_handle,
            },
            gatt::server::GattsEvent::ServiceStarted {
                status,
                service_handle,
            } => GattsEvent::ServiceStarted {
                status,
                service_handle,
            },
            gatt::server::GattsEvent::ServiceStopped {
                status,
                service_handle,
            } => GattsEvent::ServiceStopped {
                status,
                service_handle,
            },
            gatt::server::GattsEvent::PeerConnected {
                conn_id,
                link_role,
                addr,
                conn_params,
            } => GattsEvent::PeerConnected {
                conn_id,
                link_role,
                addr,
                conn_params,
            },
            gatt::server::GattsEvent::PeerDisconnected {
                conn_id,
                addr,
                reason,
            } => GattsEvent::PeerDisconnected {
                conn_id,
                addr,
                reason,
            },
            gatt::server::GattsEvent::Open { status } => GattsEvent::Open { status },
            gatt::server::GattsEvent::Close { status, conn_id } => {
                GattsEvent::Close { status, conn_id }
            }
            gatt::server::GattsEvent::Listen { conn_id, congested } => {
                GattsEvent::Listen { conn_id, congested }
            }
            gatt::server::GattsEvent::Congest { conn_id, congested } => {
                GattsEvent::Congest { conn_id, congested }
            }
            gatt::server::GattsEvent::ResponseComplete { status, handle } => {
                GattsEvent::ResponseComplete { status, handle }
            }
            gatt::server::GattsEvent::AttributeTableCreated {
                status,
                svc_uuid,
                svc_inst_id,
                handles,
            } => GattsEvent::AttributeTableCreated {
                status,
                svc_uuid,
                svc_inst_id,
                handles: handles.to_vec(),
            },
            gatt::server::GattsEvent::AttributeValueModified {
                srvc_handle,
                attr_handle,
                status,
            } => GattsEvent::AttributeValueModified {
                srvc_handle,
                attr_handle,
                status,
            },
            gatt::server::GattsEvent::ServiceChanged { status } => {
                GattsEvent::ServiceChanged { status }
            }
            _ => GattsEvent::Other,
        }
    }
}

pub struct Gatts<'d>(pub Arc<GattsInner<'d>>);

pub struct GattsInner<'d> {
    gatts: EspGatts<'d, svc::bt::Ble, ExtBtDriver<'d>>,
    apps: Arc<RwLock<HashMap<AppId, Arc<AppInner<'d>>>>>,
    gatts_events: Arc<RwLock<HashMap<Discriminant<GattsEvent>, mpsc::Sender<GattsEvent>>>>,
}

impl<'d> Gatts<'d> {
    pub fn new(bt: ExtBtDriver<'d>) -> anyhow::Result<Self> {
        let gatts = EspGatts::new(bt)?;
        let gatts_inner = GattsInner {
            gatts,
            apps: Arc::new(RwLock::new(HashMap::new())),
            gatts_events: Arc::new(RwLock::new(HashMap::new())),
        };
        gatts_inner.init_callback()?;

        let gatts = Self(Arc::new(gatts_inner));

        Ok(gatts)
    }

    pub fn register_app(&self, app_id: AppId) -> anyhow::Result<App<'d>> {
        App::new(self.0.clone(), app_id)
    }
}

impl<'d> GattsInner<'d> {
    fn init_callback(&self) -> anyhow::Result<()> {
        let callback_inner_ref = Arc::downgrade(&self.gatts_events);
        self.gatts.subscribe(move |(interface, e)| {
            log::info!("Received event {:?}", e);

            let Some(callback_map) = callback_inner_ref.upgrade() else {
                log::error!("Failed to upgrade Gatts reference");
                return;
            };

            let Ok(callback_map) = callback_map.read() else {
                log::error!("Failed to acquire read lock on Gatts events map");
                return;
            };

            let event = GattsEvent::from(e);
            let Some(sender) = callback_map.get(&discriminant(&event)) else {
                log::error!("No callback found for event {:?}", event);
                return;
            };

            sender.send(event).unwrap_or_else(|err| {
                log::error!("Failed to send event: {:?}", err);
            });
        })?;

        Ok(())
    }
}
