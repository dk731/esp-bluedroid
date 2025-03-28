use esp_idf_svc::bt::ble::gatt::{server::ConnectionId, GattConnParams};

pub struct ConnectionInner {
    pub id: ConnectionId,
    pub link_role: u8,
    pub mtu: u16,
    pub conn_params: GattConnParams,
}
