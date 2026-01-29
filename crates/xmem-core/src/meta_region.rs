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
    /// Number of allocated buffers (for stats)
    allocated: AtomicU32,
    /// Free list head index (u32::MAX = empty)
    free_head: AtomicU32,
    /// Waiter count for backpressure
    waiters: AtomicU32,
    /// Reserved for future use
    _reserved: [u32; 1],
}

const MAGIC: u32 = 0x584D454D; // "XMEM"
const VERSION: u32 = 2;

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
        header.allocated = AtomicU32::new(0);
        header.free_head = AtomicU32::new(u32::MAX); // Empty free list
        header.waiters = AtomicU32::new(0);
        header._reserved = [0; 1];

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

    /// Get header reference
    fn header(&self) -> &MetaRegionHeader {
        unsafe { &*(self.shm.as_ptr() as *const MetaRegionHeader) }
    }

    /// Allocate a buffer slot, returns meta_index
    pub fn alloc(&self) -> Result<u32> {
        let header = self.header();

        // Try to get from free list first
        loop {
            let head = header.free_head.load(Ordering::Acquire);
            if head == u32::MAX {
                break; // Free list empty
            }

            let meta = self.get(head)?;
            let next = meta.next_free.load(Ordering::Acquire);

            if header.free_head
                .compare_exchange_weak(head, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                header.allocated.fetch_add(1, Ordering::SeqCst);
                return Ok(head);
            }
        }

        // Free list empty, allocate new slot
        let id = header.next_id.fetch_add(1, Ordering::SeqCst);

        if id >= self.capacity as u32 {
            header.next_id.fetch_sub(1, Ordering::SeqCst);
            return Err(Error::SharedMemory("metadata region full".to_string()));
        }

        header.allocated.fetch_add(1, Ordering::SeqCst);
        Ok(id)
    }

    /// Free a buffer slot, add to free list
    pub fn free(&self, index: u32) -> Result<()> {
        if index >= self.capacity as u32 {
            return Err(Error::BufferNotFound(index));
        }

        let header = self.header();
        let meta = self.get(index)?;

        // Add to free list head (lock-free CAS)
        loop {
            let old_head = header.free_head.load(Ordering::Acquire);
            meta.next_free.store(old_head, Ordering::Release);

            if header.free_head
                .compare_exchange_weak(old_head, index, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }

        header.allocated.fetch_sub(1, Ordering::SeqCst);
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    fn unique_name() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("/xmem_meta_test_{}", ts)
    }

    #[test]
    fn test_create_and_alloc() {
        let name = unique_name();
        let region = MetaRegion::create(&name, 10).unwrap();

        assert_eq!(region.capacity(), 10);

        // Allocate slots
        let idx0 = region.alloc().unwrap();
        let idx1 = region.alloc().unwrap();

        assert_eq!(idx0, 0);
        assert_eq!(idx1, 1);
    }

    #[test]
    fn test_get_meta() {
        let name = unique_name();
        let mut region = MetaRegion::create(&name, 10).unwrap();

        let idx = region.alloc().unwrap();
        let meta = region.get_mut(idx).unwrap();

        meta.id.store(42, Ordering::SeqCst);
        meta.size.store(1024, Ordering::SeqCst);
        meta.ref_count.store(1, Ordering::SeqCst);

        // Read back
        let meta = region.get(idx).unwrap();
        assert_eq!(meta.id.load(Ordering::SeqCst), 42);
        assert_eq!(meta.size.load(Ordering::SeqCst), 1024);
        assert_eq!(meta.ref_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_open_existing() {
        let name = unique_name();

        // Create and write
        let mut region = MetaRegion::create(&name, 10).unwrap();
        let idx = region.alloc().unwrap();
        let meta = region.get_mut(idx).unwrap();
        meta.id.store(123, Ordering::SeqCst);

        // Open and read (owner still alive)
        let region2 = MetaRegion::open(&name).unwrap();
        let meta = region2.get(0).unwrap();
        assert_eq!(meta.id.load(Ordering::SeqCst), 123);
    }

    #[test]
    fn test_capacity_limit() {
        let name = unique_name();
        let region = MetaRegion::create(&name, 2).unwrap();

        assert!(region.alloc().is_ok());
        assert!(region.alloc().is_ok());
        assert!(region.alloc().is_err()); // Full
    }
}
