use std::sync::Arc;

use esp_idf_svc::bt::BtUuid;

use super::attribute::{Attribute, AttributeInner};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescritporId(BtUuid);

impl std::hash::Hash for DescritporId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_bytes().hash(state);
    }
}

pub trait DescriptorAttribute: Send + Sync + 'static {
    fn update_from_bytes(&self, bytes: &[u8]) -> anyhow::Result<()>;
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>>;
}

#[derive(Clone)]
pub struct Descriptor<T: Attribute>(pub Arc<DescriptorInner<T>>);

pub struct DescriptorInner<T: Attribute> {
    // pub service: RwLock<Weak<ServiceInner>>,
    // pub config: CharacteristicConfig,
    // descriptors: HashMap<DescritporId, Arc<dyn AnyAttribute>>,
    pub attribute: AttributeInner<T>,
}

impl<T: Attribute> DescriptorAttribute for DescriptorInner<T> {
    fn update_from_bytes(&self, bytes: &[u8]) -> anyhow::Result<()> {
        self.attribute.update(Arc::new(T::from_bytes(bytes)?))
    }

    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        self.attribute.get_bytes()
    }
}
