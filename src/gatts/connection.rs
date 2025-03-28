use std::{rc::Weak, sync::Arc};

use esp_idf_svc::bt::ble::gatt::server::ConnectionId;

pub struct Connection(pub Arc<ConnectionInner>);

pub struct ConnectionInner {
    id: ConnectionId,
    // link_role: LinkR,
}
