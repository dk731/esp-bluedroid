use std::sync::{Arc, RwLock};

use crossbeam_channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};

pub trait Attribute: Send + Sync + 'static {
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>>;
    fn update_from_bytes(&mut self, bytes: &[u8]) -> anyhow::Result<()>;
}

pub trait SerializableAttribute: Serialize + for<'a> Deserialize<'a> {}
impl<T> Attribute for T
where
    T: Serialize + for<'a> Deserialize<'a> + Send + Sync + 'static,
{
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        // let bytes = bincode::serialize(self)?;
        // Ok(bytes)
        todo!()
    }

    fn update_from_bytes(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        // self.
        // *self = bincode::deserialize(bytes)?;
        // Ok(())
        todo!()
    }
}

#[derive(Clone)]
pub struct AttributeUpdate<T> {
    pub old: T,
    pub new: T,
}

pub struct AttributeInner {
    pub data: Arc<RwLock<dyn Attribute>>,
    updates_rx: Receiver<AttributeUpdate<Arc<dyn Attribute>>>,
    updates_tx: Sender<AttributeUpdate<Arc<dyn Attribute>>>,
}

impl AttributeInner {
    pub fn new<T>(value: T) -> Self
    where
        T: Serialize + for<'de> Deserialize<'de> + Clone + Sync + Send + 'static,
    {
        let (updates_tx, updates_rx) = crossbeam_channel::bounded(1);
        Self {
            data: Arc::new(RwLock::new(value)),
            updates_rx,
            updates_tx,
        }
    }
}
