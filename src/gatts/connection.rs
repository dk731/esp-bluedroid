pub struct Service(pub Arc<ServiceInner>);

pub struct ServiceInner {
    pub app: Weak<AppInner>,
    pub id: GattServiceId,
    pub num_handles: u16,

    pub characteristics: Arc<RwLock<HashMap<Handle, Arc<dyn AnyCharacteristic>>>>,
    pub handle: RwLock<Option<Handle>>,
}
