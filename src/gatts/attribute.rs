use std::sync::{Arc, RwLock};

use crossbeam_channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};

pub trait Attribute: Send + Sync + 'static {
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>>;
    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self>
    where
        Self: Sized;
}

pub trait SerializableAttribute: Serialize + for<'a> Deserialize<'a> {}
impl<T> Attribute for T
where
    T: Serialize + for<'a> Deserialize<'a> + Send + Sync + 'static,
{
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        bincode::serde::encode_to_vec(self, bincode::config::standard()).map_err(|err| {
            anyhow::anyhow!(
                "Failed to serialize characteristic value to bytes: {:?}",
                err
            )
        })
    }

    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let (new_value, _): (T, usize) =
            bincode::serde::decode_from_slice(bytes, bincode::config::standard()).map_err(
                |err| {
                    anyhow::anyhow!(
                        "Failed to deserialize bytes to characteristic value: {:?}",
                        err
                    )
                },
            )?;

        Ok(new_value)
    }
}

#[derive(Clone)]
pub struct AttributeUpdate<T> {
    pub old: T,
    pub new: T,
}

pub struct AttributeInner<T: Attribute> {
    value: RwLock<Arc<T>>,

    updates_rx: Receiver<AttributeUpdate<Arc<dyn Attribute>>>,
    updates_tx: Sender<AttributeUpdate<Arc<dyn Attribute>>>,
}

impl<T: Attribute> AttributeInner<T> {
    pub fn new(value: T) -> Self
    where
        T: Serialize + for<'de> Deserialize<'de> + Clone + Sync + Send + 'static,
    {
        let (updates_tx, updates_rx) = crossbeam_channel::bounded(1);
        Self {
            value: RwLock::new(Arc::new(value)),
            updates_rx,
            updates_tx,
        }
    }

    pub fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        self.value
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read attribute"))?
            .get_bytes()
    }

    pub fn update_from_bytes(&self, bytes: &[u8]) -> anyhow::Result<()> {
        let mut value = self
            .value
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write attribute"))?;
        let old_value = value.clone();

        *value = Arc::new(T::from_bytes(bytes)?);
        let new_value = value.clone();

        self.updates_tx
            .send(AttributeUpdate {
                old: old_value,
                new: new_value,
            })
            .map_err(|_| anyhow::anyhow!("Failed to send attribute update"))?;

        Ok(())
    }
}
