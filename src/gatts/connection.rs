use esp_idf_svc::bt::{
    ble::gatt::{server::ConnectionId, GattConnParams},
    BdAddr,
};

#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Connected(ConnectionInner),
    Disconnected(ConnectionInner),
}

#[derive(Debug, Clone)]
pub struct ConnectionInner {
    pub id: ConnectionId,
    pub link_role: u8,
    pub mtu: Option<u16>,
    pub address: BdAddr,
    pub conn_params: GattConnParams,
}
