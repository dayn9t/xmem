//! Buffer handle and storage

use crate::shm::SharedMemory;
use crate::storage::StorageType;

#[cfg(feature = "cuda")]
use crate::cuda::CudaBuffer;

/// Buffer data storage
pub enum BufferData {
    Cpu(SharedMemory),
    #[cfg(feature = "cuda")]
    Cuda(CudaBuffer),
}

impl BufferData {
    /// Get storage type
    pub fn storage_type(&self) -> StorageType {
        match self {
            BufferData::Cpu(_) => StorageType::Cpu,
            #[cfg(feature = "cuda")]
            BufferData::Cuda(_) => StorageType::Cuda,
        }
    }

    /// Get size in bytes
    pub fn size(&self) -> usize {
        match self {
            BufferData::Cpu(shm) => shm.size(),
            #[cfg(feature = "cuda")]
            BufferData::Cuda(buf) => buf.size(),
        }
    }

    /// Get CPU pointer (only for CPU buffers)
    pub fn as_cpu_ptr(&self) -> Option<*const u8> {
        match self {
            BufferData::Cpu(shm) => Some(shm.as_ptr()),
            #[cfg(feature = "cuda")]
            BufferData::Cuda(_) => None,
        }
    }

    /// Get mutable CPU pointer (only for CPU buffers)
    pub fn as_cpu_mut_ptr(&mut self) -> Option<*mut u8> {
        match self {
            BufferData::Cpu(shm) => Some(shm.as_mut_ptr()),
            #[cfg(feature = "cuda")]
            BufferData::Cuda(_) => None,
        }
    }

    /// Get CUDA device pointer (only for CUDA buffers)
    #[cfg(feature = "cuda")]
    pub fn as_cuda_ptr(&self) -> Option<u64> {
        match self {
            BufferData::Cpu(_) => None,
            BufferData::Cuda(buf) => Some(buf.device_ptr()),
        }
    }

    /// Get CUDA device ID (only for CUDA buffers)
    #[cfg(feature = "cuda")]
    pub fn cuda_device_id(&self) -> Option<i32> {
        match self {
            BufferData::Cpu(_) => None,
            BufferData::Cuda(buf) => Some(buf.device_id()),
        }
    }
}
