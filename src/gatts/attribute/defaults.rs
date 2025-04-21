use crate::gatts::attribute::Attribute;
use std::fmt::Debug;

/// A wrapper for u8 values that implements the Attribute trait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct U8Attr(pub u8);

impl Attribute for U8Attr {
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(vec![self.0])
    }

    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        if bytes.len() != 1 {
            return Err(anyhow::anyhow!(
                "Invalid length for U8Attr: expected 1 byte, got {}",
                bytes.len()
            ));
        }
        Ok(U8Attr(bytes[0]))
    }
}

/// A wrapper for u16 values that implements the Attribute trait.
/// Uses little-endian byte order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct U16Attr(pub u16);

impl Attribute for U16Attr {
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(self.0.to_le_bytes().to_vec())
    }

    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        if bytes.len() != 2 {
            return Err(anyhow::anyhow!(
                "Invalid length for U16Attr: expected 2 bytes, got {}",
                bytes.len()
            ));
        }
        let value = u16::from_le_bytes([bytes[0], bytes[1]]);
        Ok(U16Attr(value))
    }
}

/// A wrapper for u32 values that implements the Attribute trait.
/// Uses little-endian byte order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct U32Attr(pub u32);

impl Attribute for U32Attr {
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(self.0.to_le_bytes().to_vec())
    }

    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        if bytes.len() != 4 {
            return Err(anyhow::anyhow!(
                "Invalid length for U32Attr: expected 4 bytes, got {}",
                bytes.len()
            ));
        }
        let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        Ok(U32Attr(value))
    }
}

/// A wrapper for i8 values that implements the Attribute trait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct I8Attr(pub i8);

impl Attribute for I8Attr {
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(vec![self.0 as u8])
    }

    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        if bytes.len() != 1 {
            return Err(anyhow::anyhow!(
                "Invalid length for I8Attr: expected 1 byte, got {}",
                bytes.len()
            ));
        }
        Ok(I8Attr(bytes[0] as i8))
    }
}

/// A wrapper for i16 values that implements the Attribute trait.
/// Uses little-endian byte order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct I16Attr(pub i16);

impl Attribute for I16Attr {
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(self.0.to_le_bytes().to_vec())
    }

    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        if bytes.len() != 2 {
            return Err(anyhow::anyhow!(
                "Invalid length for I16Attr: expected 2 bytes, got {}",
                bytes.len()
            ));
        }
        let value = i16::from_le_bytes([bytes[0], bytes[1]]);
        Ok(I16Attr(value))
    }
}

/// A wrapper for i32 values that implements the Attribute trait.
/// Uses little-endian byte order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct I32Attr(pub i32);

impl Attribute for I32Attr {
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(self.0.to_le_bytes().to_vec())
    }

    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        if bytes.len() != 4 {
            return Err(anyhow::anyhow!(
                "Invalid length for I32Attr: expected 4 bytes, got {}",
                bytes.len()
            ));
        }
        let value = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        Ok(I32Attr(value))
    }
}

/// A wrapper for boolean values that implements the Attribute trait.
/// Uses a single byte (0 for false, 1 for true).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoolAttr(pub bool);

impl Attribute for BoolAttr {
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(vec![if self.0 { 1 } else { 0 }])
    }

    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        if bytes.len() != 1 {
            return Err(anyhow::anyhow!(
                "Invalid length for BoolAttr: expected 1 byte, got {}",
                bytes.len()
            ));
        }
        Ok(BoolAttr(bytes[0] != 0))
    }
}

/// A wrapper for f32 values that implements the Attribute trait.
/// Uses little-endian byte order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct F32Attr(pub f32);

impl Attribute for F32Attr {
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(self.0.to_le_bytes().to_vec())
    }

    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        if bytes.len() != 4 {
            return Err(anyhow::anyhow!(
                "Invalid length for F32Attr: expected 4 bytes, got {}",
                bytes.len()
            ));
        }
        let value = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        Ok(F32Attr(value))
    }
}

/// A wrapper for string values that implements the Attribute trait.
/// Stores UTF-8 encoded string data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringAttr(pub String);

impl Attribute for StringAttr {
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(self.0.as_bytes().to_vec())
    }

    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let string = String::from_utf8(bytes.to_vec())
            .map_err(|e| anyhow::anyhow!("Invalid UTF-8 string data: {}", e))?;
        Ok(StringAttr(string))
    }
}

/// A wrapper for byte array values that implements the Attribute trait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytesAttr(pub Vec<u8>);

impl Attribute for BytesAttr {
    fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(self.0.clone())
    }

    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(BytesAttr(bytes.to_vec()))
    }
}
