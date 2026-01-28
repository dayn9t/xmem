//! Storage type definitions

/// Storage location type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StorageType {
    Cpu = 0,
    #[cfg(feature = "cuda")]
    Cuda = 1,
}

impl StorageType {
    /// Convert from u8
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(StorageType::Cpu),
            #[cfg(feature = "cuda")]
            1 => Some(StorageType::Cuda),
            _ => None,
        }
    }
}

/// Access mode for buffer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    ReadOnly,
    ReadWrite,
}
