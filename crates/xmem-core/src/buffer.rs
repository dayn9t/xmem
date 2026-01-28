//! Buffer handle and storage

use crate::shm::SharedMemory;
use crate::storage::StorageType;

/// Buffer data storage (Phase 3: CPU only, Phase 4 添加 CUDA)
pub enum BufferData {
    Cpu(SharedMemory),
}

impl BufferData {
    /// Get storage type
    pub fn storage_type(&self) -> StorageType {
        match self {
            BufferData::Cpu(_) => StorageType::Cpu,
        }
    }

    /// Get size in bytes
    pub fn size(&self) -> usize {
        match self {
            BufferData::Cpu(shm) => shm.size(),
        }
    }

    /// Get CPU pointer (only for CPU buffers)
    pub fn as_cpu_ptr(&self) -> Option<*const u8> {
        match self {
            BufferData::Cpu(shm) => Some(shm.as_ptr()),
        }
    }

    /// Get mutable CPU pointer (only for CPU buffers)
    pub fn as_cpu_mut_ptr(&mut self) -> Option<*mut u8> {
        match self {
            BufferData::Cpu(shm) => Some(shm.as_mut_ptr()),
        }
    }
}
