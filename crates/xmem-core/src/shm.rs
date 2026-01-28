//! POSIX shared memory wrapper

use crate::{Error, Result};
use shared_memory::{Shmem, ShmemConf};

/// Shared memory region wrapper
pub struct SharedMemory {
    inner: Shmem,
    name: String,
    size: usize,
    owner: bool,
}

impl SharedMemory {
    /// Create a new shared memory region
    pub fn create(name: &str, size: usize) -> Result<Self> {
        let shmem = ShmemConf::new()
            .size(size)
            .os_id(name)
            .create()
            .map_err(|e| Error::SharedMemory(e.to_string()))?;

        Ok(Self {
            inner: shmem,
            name: name.to_string(),
            size,
            owner: true,
        })
    }

    /// Open an existing shared memory region
    pub fn open(name: &str) -> Result<Self> {
        let shmem = ShmemConf::new()
            .os_id(name)
            .open()
            .map_err(|e| Error::SharedMemory(e.to_string()))?;

        let size = shmem.len();

        Ok(Self {
            inner: shmem,
            name: name.to_string(),
            size,
            owner: false,
        })
    }

    /// Get the name of the shared memory region
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the size of the shared memory region
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get a raw pointer to the shared memory
    pub fn as_ptr(&self) -> *const u8 {
        self.inner.as_ptr()
    }

    /// Get a mutable raw pointer to the shared memory
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.inner.as_ptr() as *mut u8
    }

    /// Get a slice view of the shared memory
    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.as_ptr(), self.size) }
    }

    /// Get a mutable slice view of the shared memory
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.as_mut_ptr(), self.size) }
    }
}
