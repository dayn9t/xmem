//! Buffer metadata structure

use std::sync::atomic::{AtomicI32, AtomicU8, AtomicU32, AtomicU64};

/// Maximum number of dimensions
pub const MAX_NDIM: usize = 8;

/// CUDA IPC handle size (预留，即使 CPU-only 也保留以保证跨进程兼容性)
pub const CUDA_IPC_HANDLE_SIZE: usize = 64;

/// Buffer metadata stored in shared memory
///
/// 所有字段使用原子类型，支持跨进程并发访问
#[repr(C)]
pub struct BufferMeta {
    /// Unique buffer ID
    pub id: AtomicU32,
    /// Reference count (atomic)
    pub ref_count: AtomicI32,
    /// Storage type: 0=cpu, 1=cuda
    pub storage_type: AtomicU8,
    /// GPU device ID (for CUDA)
    pub device_id: AtomicU8,
    /// Data type
    pub dtype: AtomicU8,
    /// Number of dimensions
    pub ndim: AtomicU8,
    /// Shape (up to 8 dimensions)
    pub shape: [AtomicU64; MAX_NDIM],
    /// Strides in bytes
    pub strides: [AtomicU64; MAX_NDIM],
    /// Total size in bytes
    pub size: AtomicU64,
    /// Timestamp (milliseconds since epoch)
    pub timestamp: AtomicU64,
    /// Sequence number
    pub seq: AtomicU64,
    /// Content type string (null-terminated, 不需要原子操作)
    pub content_type: [u8; 32],
    /// Producer name (null-terminated, 不需要原子操作)
    pub producer: [u8; 32],
    /// CUDA IPC handle (预留，仅 storage_type == 1 时有效)
    pub cuda_ipc_handle: [u8; CUDA_IPC_HANDLE_SIZE],
    /// Reserved for future use
    pub reserved: [u8; 64],
}

impl BufferMeta {
    /// Size of BufferMeta in bytes
    pub const SIZE: usize = std::mem::size_of::<Self>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta_size() {
        // Ensure struct size is stable for cross-process compatibility
        assert!(BufferMeta::SIZE > 0);
        println!("BufferMeta size: {} bytes", BufferMeta::SIZE);
    }
}
