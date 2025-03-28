pub mod app;
pub mod characteristic;
pub mod connection;
pub mod descriptor;
pub mod event;
pub mod service;

use std::{
    collections::HashMap,
    mem::{discriminant, Discriminant},
    sync::{Arc, RwLock},
};

use app::{App, AppInner};
use characteristic::AnyCharacteristic;
use crossbeam_channel::bounded;
use esp_idf_svc::bt::{
    ble::gatt::{
        server::{AppId, ConnectionId, EspGatts, TransferId},
        GattConnParams, GattConnReason, GattInterface, GattResponse, GattStatus, Handle,
    },
    BdAddr,
};
use event::{GattsEvent, GattsEventMessage};

use crate::ble::ExtBtDriver;
use esp_idf_svc as svc;

struct PrepareWriteBuffer {
    value: Vec<u8>,
    characteristic_handle: Handle,
}

pub struct Gatts(pub Arc<GattsInner>);

pub struct GattsInner {
    gatts: EspGatts<'static, svc::bt::Ble, ExtBtDriver>,
    apps: Arc<RwLock<HashMap<GattInterface, Arc<AppInner>>>>,
    temporary_write_buffer: Arc<RwLock<HashMap<TransferId, PrepareWriteBuffer>>>,

    gatts_events: Arc<
        RwLock<HashMap<Discriminant<GattsEvent>, crossbeam_channel::Sender<GattsEventMessage>>>,
    >,
}

impl Gatts {
    pub fn new(bt: ExtBtDriver) -> anyhow::Result<Self> {
        let gatts = EspGatts::new(bt)?;
        let gatts_inner = GattsInner {
            gatts,
            apps: Arc::new(RwLock::new(HashMap::new())),
            gatts_events: Arc::new(RwLock::new(HashMap::new())),
            temporary_write_buffer: Arc::new(RwLock::new(HashMap::new())),
        };

        let gatts = Self(Arc::new(gatts_inner));

        gatts.init_callback()?;
        gatts.configure_global_events()?;

        Ok(gatts)
    }

    fn configure_global_events(&self) -> anyhow::Result<()> {
        let (tx, rx) = bounded(1);

        let mut gatt_events = self
            .0
            .gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events map"))?;

        gatt_events.insert(
            discriminant(&GattsEvent::Read {
                conn_id: 0,
                trans_id: 0,
                addr: BdAddr::from_bytes([0; 6]),
                handle: 0,
                offset: 0,
                is_long: false,
                need_rsp: false,
            }),
            tx.clone(),
        );
        gatt_events.insert(
            discriminant(&GattsEvent::Write {
                conn_id: 0,
                trans_id: 0,
                addr: BdAddr::from_bytes([0; 6]),
                handle: 0,
                offset: 0,
                need_rsp: false,
                is_prep: false,
                value: vec![],
            }),
            tx.clone(),
        );
        gatt_events.insert(
            discriminant(&GattsEvent::ExecWrite {
                conn_id: 0,
                trans_id: 0,
                addr: BdAddr::from_bytes([0; 6]),
                canceled: false,
            }),
            tx.clone(),
        );
        gatt_events.insert(
            discriminant(&GattsEvent::PeerConnected {
                conn_id: 0,
                link_role: 0,
                addr: BdAddr::from_bytes([0; 6]),
                conn_params: GattConnParams {
                    interval_ms: 0,
                    latency_ms: 0,
                    timeout_ms: 0,
                },
            }),
            tx.clone(),
        );
        gatt_events.insert(
            discriminant(&GattsEvent::PeerDisconnected {
                conn_id: 0,
                addr: BdAddr::from_bytes([0; 6]),
                reason: GattConnReason::Unknown,
            }),
            tx.clone(),
        );
        gatt_events.insert(
            discriminant(&GattsEvent::Mtu { conn_id: 0, mtu: 0 }),
            tx.clone(),
        );

        let gatts = Arc::downgrade(&self.0);
        std::thread::Builder::new()
            .stack_size(8 * 1024)
            .spawn(move || {
                for event in rx.iter() {
                    let Some(gatts) = gatts.upgrade() else {
                        log::warn!("Failed to upgrade Gatts, exiting write events thread");
                        return;
                    };

                    if let Err(err) = gatts.handle_gatts_global_event(event) {
                        log::error!("Failed to handle write event: {:?}", err);
                    }
                }
            })?;

        Ok(())
    }

    fn init_callback(&self) -> anyhow::Result<()> {
        let callback_inner_ref = Arc::downgrade(&self.0.gatts_events);
        self.0.gatts.subscribe(move |(interface, e)| {
            log::info!("Received event {:?}", (interface, &e));

            let Some(callback_map) = callback_inner_ref.upgrade() else {
                log::error!("Failed to upgrade Gatts events map");
                return;
            };

            let Ok(callback_map) = callback_map.read() else {
                log::error!("Failed to acquire read lock on Gatts events map");
                return;
            };

            let event = GattsEvent::from(e);
            let Some(sender) = callback_map.get(&discriminant(&event)) else {
                log::warn!("No callback found for event {:?}", event);
                return;
            };

            sender
                .send(GattsEventMessage(interface, event))
                .unwrap_or_else(|err| {
                    log::error!("Failed to send event: {:?}", err);
                });
        })?;

        Ok(())
    }

    pub fn register_app(&self, app_id: AppId) -> anyhow::Result<App> {
        App::new(self.0.clone(), app_id)
    }
}

impl GattsInner {
    fn send_response(
        &self,
        characteristic_handle: Handle,
        gatts_if: GattInterface,
        conn_id: ConnectionId,
        trans_id: TransferId,
        status: GattStatus,
        response: Option<&GattResponse>,
    ) -> anyhow::Result<()> {
        let (tx, rx) = bounded(1);
        let callback_key = discriminant(&GattsEvent::ResponseComplete {
            status: GattStatus::Busy,
            handle: 0,
        });

        self.gatts_events
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write Gatts events"))?
            .insert(callback_key.clone(), tx.clone());

        self.gatts
            .send_response(gatts_if, conn_id, trans_id, status, response)?;

        match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(GattsEventMessage(_, GattsEvent::ResponseComplete { status, handle })) => {
                if characteristic_handle != handle {
                    return Err(anyhow::anyhow!(
                        "Received unexpected GATT characteristic handle: {:?}",
                        characteristic_handle
                    ));
                }

                if status != GattStatus::Ok {
                    return Err(anyhow::anyhow!("Failed to stop service: {:?}", status));
                }

                Ok(())
            }
            Ok(_) => Err(anyhow::anyhow!("Received unexpected GATT")),
            Err(_) => Err(anyhow::anyhow!("Timed out waiting for GATT")),
        }
    }

    fn get_characteristic_lock(
        &self,
        interface: GattInterface,
        handle: Handle,
    ) -> anyhow::Result<Arc<dyn AnyCharacteristic>> {
        let app = self
            .apps
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to acquire read lock on Gatts apps"))?
            .get(&interface)
            .ok_or(anyhow::anyhow!(
                "No found app with given gatts interface: {:?}",
                interface
            ))?
            .clone();

        let services = &app
            .services
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to acquire read lock on Gatts services"))?;

        let characteristic = {
            let mut result = None;
            for service in services.values() {
                let characteristic = service
                    .characteristics
                    .read()
                    .map_err(|_| {
                        anyhow::anyhow!("Failed to acquire read lock on Gatts characteristics")
                    })?
                    .get(&handle)
                    .cloned();

                if let Some(c) = characteristic {
                    result = Some(c);
                    break;
                }
            }
            result
        }
        .ok_or(anyhow::anyhow!(
            "Not found characteristic with given handle: {:?}",
            handle
        ))?;

        Ok(characteristic)
    }

    fn handle_gatts_global_event(&self, event: GattsEventMessage) -> anyhow::Result<()> {
        match event {
            GattsEventMessage(
                interface,
                GattsEvent::Read {
                    conn_id,
                    trans_id,
                    handle,
                    offset,
                    need_rsp,
                    ..
                },
            ) => {
                if !need_rsp {
                    log::warn!("Read event without response, ignoring");
                    return Ok(());
                }

                let response = (|| {
                    let characteristic = self.get_characteristic_lock(interface, handle)?;
                    let bytes = characteristic.as_bytes()?;

                      let mut response = GattResponse::new();
                    response.attr_handle(handle).auth_req(0).offset(offset).value(&bytes)?;

                    log::info!(
                        "Sending read response with handle: {:?}, bytes: {:?}",
                        handle,
                        bytes
                    );

                     Ok(response)
                })()
                .map_err(|err: anyhow::Error| {
                    match self.send_response(handle,interface, conn_id, trans_id, GattStatus::Error, None) {
                        Ok(_) => anyhow::anyhow!("Failed to prepare characteristics bytes: {:?}", err),
                        Err(send_err) => {
                            anyhow::anyhow!("Failed to prepare characteristics bytes ({:?}) and send error response ({:?})", err, send_err)
                        }
                    }
                })?;

                self.send_response(
                    handle,
                    interface,
                    conn_id,
                    trans_id,
                    GattStatus::Ok,
                    Some(&response),
                )?;

                Ok(())
            }
            GattsEventMessage(
                interface,
                GattsEvent::Write {
                    conn_id,
                    trans_id,
                    handle,
                    offset,
                    need_rsp,
                    is_prep,
                    value,
                    ..
                },
            ) => {
                let result: anyhow::Result<()> = (|| {
                    let mut temp_storage = self.temporary_write_buffer.write().map_err(|_| {
                        anyhow::anyhow!("Failed to acquire write lock on temporary write buffer")
                    })?;
                    let temp_buffer = temp_storage.entry(trans_id).or_insert(PrepareWriteBuffer {
                        value: Vec::new(),
                        characteristic_handle: handle,
                    });

                    if temp_buffer.value.len() < offset as usize + value.len() {
                        temp_buffer.value.resize(offset as usize + value.len(), 0);
                    }
                    temp_buffer.value[offset as usize..offset as usize + value.len()]
                        .copy_from_slice(&value);

                    if !is_prep {
                        log::info!("Updating characteristic with handle: {:?}", handle);
                        let characteristic = self.get_characteristic_lock(interface, handle)?;

                        log::info!("Updating characteristic bytes: {:?}", temp_buffer.value);
                        characteristic.update_from_bytes(&temp_buffer.value)?;
                    }

                    Ok(())
                })();

                if !need_rsp {
                    log::warn!("Write event without response, ignoring");
                    return result;
                }

                self.send_response(
                    handle,
                    interface,
                    conn_id,
                    trans_id,
                    if result.is_ok() {
                        GattStatus::Ok
                    } else {
                        GattStatus::Error
                    },
                    Some(
                        GattResponse::new()
                            .attr_handle(handle)
                            .auth_req(0)
                            .offset(offset)
                            .value(&value)?,
                    ),
                )?;

                result
            }
            GattsEventMessage(
                interface,
                GattsEvent::ExecWrite {
                    conn_id,
                    trans_id,
                    canceled,
                    ..
                },
            ) => {
                let mut handle = None;
                let result = (|| {
                    let temp_storage = self.temporary_write_buffer.write().map_err(|_| {
                        anyhow::anyhow!("Failed to acquire write lock on temporary write buffer")
                    })?;
                    let temp_buffer = temp_storage.get(&trans_id).ok_or(anyhow::anyhow!(
                        "Not found temporary write buffer with given transfer id: {:?}",
                        trans_id
                    ))?;
                    handle.replace(temp_buffer.characteristic_handle);

                    if !canceled {
                        let characteristic = self.get_characteristic_lock(
                            interface,
                            temp_buffer.characteristic_handle,
                        )?;

                        characteristic.update_from_bytes(&temp_buffer.value)?;
                    }

                    Ok(())
                })();

                if let Some(handle) = handle {
                    self.send_response(
                        handle,
                        interface,
                        conn_id,
                        trans_id,
                        if result.is_ok() {
                            GattStatus::Ok
                        } else {
                            GattStatus::Error
                        },
                        None,
                    )?;
                }

                result
            }
            GattsEventMessage(
                interface,
                GattsEvent::PeerConnected {
                    conn_id,
                    link_role,
                    addr,
                    conn_params,
                },
            ) => {
                let app = self
                    .apps
                    .read()
                    .map_err(|_| anyhow::anyhow!("Failed to acquire read lock on Gatts apps"))?
                    .get(&interface)
                    .ok_or(anyhow::anyhow!(
                        "No found app with given gatts interface: {:?}",
                        interface
                    ))?
                    .clone();

                app.connections
                    .write()
                    .map_err(|_| {
                        anyhow::anyhow!("Failed to acquire write lock on Gatts connections")
                    })?
                    .insert(
                        conn_id,
                        connection::ConnectionInner {
                            id: conn_id,
                            link_role,
                            mtu: None,
                            conn_params,
                            address: addr,
                        },
                    );

                Ok(())
            }
            GattsEventMessage(interface, GattsEvent::PeerDisconnected { conn_id, .. }) => {
                let app = self
                    .apps
                    .read()
                    .map_err(|_| anyhow::anyhow!("Failed to acquire read lock on Gatts apps"))?
                    .get(&interface)
                    .ok_or(anyhow::anyhow!(
                        "No found app with given gatts interface: {:?}",
                        interface
                    ))?
                    .clone();

                app.connections
                    .write()
                    .map_err(|_| {
                        anyhow::anyhow!("Failed to acquire write lock on Gatts connections")
                    })?
                    .remove(&conn_id)
                    .ok_or(anyhow::anyhow!(
                        "No found connection with given connection id: {:?}",
                        conn_id
                    ))?;

                Ok(())
            }
            GattsEventMessage(interface, GattsEvent::Mtu { conn_id, mtu }) => {
                let app = self
                    .apps
                    .read()
                    .map_err(|_| anyhow::anyhow!("Failed to acquire read lock on Gatts apps"))?
                    .get(&interface)
                    .ok_or(anyhow::anyhow!(
                        "No found app with given gatts interface: {:?}",
                        interface
                    ))?
                    .clone();

                app.connections
                    .write()
                    .map_err(|_| {
                        anyhow::anyhow!("Failed to acquire write lock on Gatts connections")
                    })?
                    .get_mut(&conn_id)
                    .ok_or(anyhow::anyhow!(
                        "No found connection with given connection id: {:?}",
                        conn_id
                    ))?
                    .mtu
                    .replace(mtu);

                Ok(())
            }
            _ => Err(anyhow::anyhow!("Unexpected GATT event: {:?}", event)),
        }
    }
}
