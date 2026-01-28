//! RAII buffer guard

use crate::buffer::BufferData;
use crate::storage::AccessMode;
use crate::{Error, Result};
use std::sync::atomic::{AtomicI32, Ordering};

/// RAII guard for buffer access
pub struct BufferGuard {
    /// Buffer data
    data: Option<BufferData>,
    /// Metadata index
    meta_index: u32,
    /// Access mode
    mode: AccessMode,
    /// Reference to ref_count in shared memory
    ref_count: *const AtomicI32,
    /// Whether this guard owns the release responsibility
    should_release: bool,
}

// Safety: BufferGuard can be sent between threads
// The underlying shared memory is process-wide accessible
unsafe impl Send for BufferGuard {}

impl BufferGuard {
    /// Create a new buffer guard
    pub(crate) fn new(
        data: BufferData,
        meta_index: u32,
        mode: AccessMode,
        ref_count: *const AtomicI32,
    ) -> Self {
        Self {
            data: Some(data),
            meta_index,
            mode,
            ref_count,
            should_release: true,
        }
    }

    /// Get metadata index
    pub fn meta_index(&self) -> u32 {
        self.meta_index
    }

    /// Get access mode
    pub fn mode(&self) -> AccessMode {
        self.mode
    }

    /// Check if buffer is still valid
    pub fn is_valid(&self) -> bool {
        self.data.is_some()
    }

    /// Get CPU slice (read-only)
    pub fn as_cpu_slice(&self) -> Result<&[u8]> {
        let data = self.data.as_ref().ok_or(Error::AlreadyForgotten)?;
        let ptr = data.as_cpu_ptr().ok_or_else(|| Error::TypeMismatch {
            expected: "Cpu".to_string(),
            actual: "Cuda".to_string(),
        })?;
        Ok(unsafe { std::slice::from_raw_parts(ptr, data.size()) })
    }

    /// Get CPU slice (mutable, requires ReadWrite mode)
    pub fn as_cpu_slice_mut(&mut self) -> Result<&mut [u8]> {
        if self.mode == AccessMode::ReadOnly {
            return Err(Error::ReadOnly);
        }
        let data = self.data.as_mut().ok_or(Error::AlreadyForgotten)?;
        let size = data.size();
        let ptr = data.as_cpu_mut_ptr().ok_or_else(|| Error::TypeMismatch {
            expected: "Cpu".to_string(),
            actual: "Cuda".to_string(),
        })?;
        Ok(unsafe { std::slice::from_raw_parts_mut(ptr, size) })
    }

    /// Get CUDA device pointer (read-only)
    #[cfg(feature = "cuda")]
    pub fn as_cuda_ptr(&self) -> Result<u64> {
        let data = self.data.as_ref().ok_or(Error::AlreadyForgotten)?;
        data.as_cuda_ptr().ok_or_else(|| Error::TypeMismatch {
            expected: "Cuda".to_string(),
            actual: "Cpu".to_string(),
        })
    }

    /// Get CUDA device pointer (mutable, requires ReadWrite mode)
    #[cfg(feature = "cuda")]
    pub fn as_cuda_ptr_mut(&mut self) -> Result<u64> {
        if self.mode == AccessMode::ReadOnly {
            return Err(Error::ReadOnly);
        }
        self.as_cuda_ptr()
    }

    /// Forget this guard without releasing the buffer
    /// Used when transferring ownership to another process
    pub fn forget(mut self) {
        self.should_release = false;
        self.data = None;
    }

    /// Manually release and decrement ref count
    fn release(&self) {
        if !self.ref_count.is_null() {
            let ref_count = unsafe { &*self.ref_count };
            ref_count.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

impl Drop for BufferGuard {
    fn drop(&mut self) {
        if self.should_release && self.data.is_some() {
            self.release();
        }
    }
}
