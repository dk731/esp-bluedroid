use std::sync::{Arc, RwLock};

use crossbeam_channel::{Receiver, Sender};
use esp_idf_svc::bt::ble::gatt::Handle;
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

pub trait AnyAttribute: Send + Sync + 'static {
    fn update_from_bytes(&self, bytes: &[u8]) -> anyhow::Result<()>;
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>>;
}

#[derive(Clone)]
pub struct AttributeUpdate<T> {
    pub old: T,
    pub new: T,
}

pub struct AttributeInner<T: Attribute> {
    value: RwLock<Arc<T>>,
    pub handle: RwLock<Option<Handle>>,

    pub updates_rx: Receiver<AttributeUpdate<Arc<dyn Attribute>>>,
    updates_tx: Sender<AttributeUpdate<Arc<dyn Attribute>>>,
}

impl<T: Attribute> AttributeInner<T> {
    pub fn new(value: T) -> Self {
        let (updates_tx, updates_rx) = crossbeam_channel::bounded(1);
        Self {
            handle: RwLock::new(None),
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

    pub fn update(&self, new_value: T) -> anyhow::Result<()> {
        let mut value = self
            .value
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write attribute"))?;
        let old_value = value.clone();

        *value = Arc::new(new_value);
        let new_value = value.clone();

        self.updates_tx
            .send(AttributeUpdate {
                old: old_value,
                new: new_value,
            })
            .map_err(|_| anyhow::anyhow!("Failed to send attribute update"))?;

        Ok(())
    }

    pub fn get_value(&self) -> anyhow::Result<Arc<T>> {
        Ok(self
            .value
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read attribute"))?
            .clone())
    }

    pub fn set_handle(&self, handle: Handle) -> anyhow::Result<()> {
        *self
            .handle
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to write handle"))? = Some(handle);

        Ok(())
    }

    pub fn handle(&self) -> anyhow::Result<Handle> {
        self.handle
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to read attribute handle"))?
            .ok_or_else(|| anyhow::anyhow!("Attribute handle is not set"))
    }
}

impl<T: Attribute> AnyAttribute for AttributeInner<T> {
    fn update_from_bytes(&self, bytes: &[u8]) -> anyhow::Result<()> {
        let new_value = T::from_bytes(bytes)?;
        self.update(new_value)
    }

    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        self.get_bytes()
    }
}
