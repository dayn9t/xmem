//! Data type definitions

/// Supported data types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DType {
    UInt8 = 0,
    Int8 = 1,
    UInt16 = 2,
    Int16 = 3,
    UInt32 = 4,
    Int32 = 5,
    UInt64 = 6,
    Int64 = 7,
    Float16 = 8,
    Float32 = 9,
    Float64 = 10,
}

impl DType {
    /// Size in bytes
    pub const fn size(&self) -> usize {
        match self {
            DType::UInt8 | DType::Int8 => 1,
            DType::UInt16 | DType::Int16 | DType::Float16 => 2,
            DType::UInt32 | DType::Int32 | DType::Float32 => 4,
            DType::UInt64 | DType::Int64 | DType::Float64 => 8,
        }
    }

    /// Convert from u8
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(DType::UInt8),
            1 => Some(DType::Int8),
            2 => Some(DType::UInt16),
            3 => Some(DType::Int16),
            4 => Some(DType::UInt32),
            5 => Some(DType::Int32),
            6 => Some(DType::UInt64),
            7 => Some(DType::Int64),
            8 => Some(DType::Float16),
            9 => Some(DType::Float32),
            10 => Some(DType::Float64),
            _ => None,
        }
    }
}
