//! Shared metadata region management

use crate::meta::BufferMeta;
use crate::shm::SharedMemory;
use crate::{Error, Result};
use std::sync::atomic::{AtomicU32, Ordering};

/// Header for metadata region
#[repr(C)]
struct MetaRegionHeader {
    /// Magic number for validation
    magic: u32,
    /// Version number
    version: u32,
    /// Maximum number of buffers
    capacity: u32,
    /// Next buffer ID to allocate
    next_id: AtomicU32,
}

const MAGIC: u32 = 0x584D454D; // "XMEM"
const VERSION: u32 = 1;

/// Shared metadata region
pub struct MetaRegion {
    shm: SharedMemory,
    capacity: usize,
}

impl MetaRegion {
    /// Calculate required size for given capacity
    fn calc_size(capacity: usize) -> usize {
        std::mem::size_of::<MetaRegionHeader>() + capacity * BufferMeta::SIZE
    }

    /// Create a new metadata region
    pub fn create(name: &str, capacity: usize) -> Result<Self> {
        let size = Self::calc_size(capacity);
        let mut shm = SharedMemory::create(name, size)?;

        // Initialize header
        let header = unsafe { &mut *(shm.as_mut_ptr() as *mut MetaRegionHeader) };
        header.magic = MAGIC;
        header.version = VERSION;
        header.capacity = capacity as u32;
        header.next_id = AtomicU32::new(0);

        Ok(Self { shm, capacity })
    }

    /// Open an existing metadata region
    pub fn open(name: &str) -> Result<Self> {
        let shm = SharedMemory::open(name)?;

        // Validate header
        let header = unsafe { &*(shm.as_ptr() as *const MetaRegionHeader) };
        if header.magic != MAGIC {
            return Err(Error::SharedMemory("invalid magic number".to_string()));
        }
        if header.version != VERSION {
            return Err(Error::SharedMemory(format!(
                "version mismatch: expected {}, got {}",
                VERSION, header.version
            )));
        }

        let capacity = header.capacity as usize;
        Ok(Self { shm, capacity })
    }

    /// Get capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Allocate a new buffer slot, returns meta_index
    pub fn alloc(&self) -> Result<u32> {
        let header = unsafe { &*(self.shm.as_ptr() as *const MetaRegionHeader) };
        let id = header.next_id.fetch_add(1, Ordering::SeqCst);

        if id >= self.capacity as u32 {
            header.next_id.fetch_sub(1, Ordering::SeqCst);
            return Err(Error::SharedMemory("metadata region full".to_string()));
        }

        Ok(id)
    }

    /// Get metadata by index
    pub fn get(&self, index: u32) -> Result<&BufferMeta> {
        if index >= self.capacity as u32 {
            return Err(Error::BufferNotFound(index));
        }

        let offset = std::mem::size_of::<MetaRegionHeader>() + (index as usize) * BufferMeta::SIZE;
        let ptr = unsafe { self.shm.as_ptr().add(offset) as *const BufferMeta };
        Ok(unsafe { &*ptr })
    }

    /// Get mutable metadata by index
    pub fn get_mut(&mut self, index: u32) -> Result<&mut BufferMeta> {
        if index >= self.capacity as u32 {
            return Err(Error::BufferNotFound(index));
        }

        let offset = std::mem::size_of::<MetaRegionHeader>() + (index as usize) * BufferMeta::SIZE;
        let ptr = unsafe { self.shm.as_mut_ptr().add(offset) as *mut BufferMeta };
        Ok(unsafe { &mut *ptr })
    }
}
