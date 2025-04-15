pub trait Attribute: Send + Sync {
    fn as_bytes(&self) -> anyhow::Result<Vec<u8>>;
    fn update_from_bytes(&self, data: &[u8]) -> anyhow::Result<()>;
}
